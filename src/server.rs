use std::error::Error;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use std::time::Duration;

use http_body_util::combinators::BoxBody;
use hyper::body::Bytes;
use hyper::service::service_fn;
use hyper::{Request, Response, StatusCode};
use hyper_util::rt::{TokioExecutor, TokioIo};
use hyper_util::server::conn::auto;
use hyper_util::server::graceful;
use log::{debug, info};
use tokio::net::TcpListener;
use tokio::select;
use tokio::sync::{Mutex, Notify};
use tokio::time::sleep;

use crate::api::ApiServersService;
use crate::frontend::pkg::repository::PackagesRepository;
use crate::server::api::{
  check_latest_frontend_release, trigger_shutdown, update_frontend_package,
};
use crate::server::common::{ServiceError, empty_body, full_body};
use crate::server::frontend::serve_frontend;
use crate::server::router::get_route;

mod api;
mod common;
mod frontend;
mod router;

const GRACEFUL_SHUTDOWN_TIMEOUT_SEC: u8 = 30;
#[derive(Clone)]
pub struct Dependencies {
  pub packages_repository: Arc<Mutex<PackagesRepository>>,
  pub api_service: Arc<Mutex<ApiServersService>>,
}

pub async fn serve(
  listener: TcpListener,
  idle_shutdown_timeout: Option<u32>,
  dependencies: Dependencies,
) -> Result<(), Box<dyn Error>> {
  let graceful = graceful::GracefulShutdown::new();
  let main_service_shutdown_notifier = Arc::new(Notify::new());

  loop {
    let shutdown_notifier = main_service_shutdown_notifier.clone();

    select! {
      Ok((stream, incoming_addr)) = listener.accept() => {
        debug!("accepted connection from {incoming_addr}");

        let deps = dependencies.clone();
        tokio::task::spawn(async move {
          let io = TokioIo::new(stream);
          let runner = auto::Builder::new(TokioExecutor::new());
          _ = runner.serve_connection(io, service_fn(|req| { service(req, shutdown_notifier.clone(), deps.clone()) })).await;
        });
      }
      _ = wait_for_shutdown_condition(shutdown_notifier.clone(), idle_shutdown_timeout) => {
        drop(listener);
        break;
      }
    }
  }

  select! {
    _ = graceful.shutdown() => {
      info!("server shut down gracefully")
    },
    _ = sleep(Duration::from_secs(GRACEFUL_SHUTDOWN_TIMEOUT_SEC.into())) => {
      return Err(*Box::new(format!("could not finish graceful shutdown in {GRACEFUL_SHUTDOWN_TIMEOUT_SEC} seconds").into()));
    }
  }

  Ok(())
}

async fn wait_for_shutdown_condition<T>(
  service_shutdown_notify: T,
  idle_shutdown_timeout: Option<u32>,
) where
  T: Deref<Target = Notify>,
{
  select! {
    _ = service_shutdown_notify.notified() => {
      info!("triggering shutdown due to shutdown request")
    }
    _ = tokio::signal::ctrl_c() => {
      info!("triggering shutdown due to SIGINT signal")
    }
    _ = sleep(Duration::from_secs(idle_shutdown_timeout.unwrap_or_default().into())), if idle_shutdown_timeout.is_some() => {
      info!("triggering shutdown since no request has been received for {} seconds", idle_shutdown_timeout.unwrap_or_default())
    }
  }
}

async fn service<T>(
  req: Request<hyper::body::Incoming>,
  shutdown_notifier: T,
  dependencies: Dependencies,
) -> Result<Response<BoxBody<Bytes, ServiceError>>, ServiceError>
where
  T: Deref<Target = Notify>,
{
  let route = get_route(req).await;
  match route {
    Ok(r) => match r {
      router::Routes::Frontend(name, encodings) => {
        serve_frontend(
          name.as_deref(),
          encodings,
          dependencies.packages_repository.lock().await.deref_mut(),
        )
        .await
      }
      router::Routes::Api(api_route) => match api_route {
        router::ApiRoutes::FrontendLatest => {
          check_latest_frontend_release(dependencies.packages_repository.lock().await.deref()).await
        }
        router::ApiRoutes::FrontendUpdate(release) => {
          update_frontend_package(
            release,
            dependencies.packages_repository.lock().await.deref_mut(),
          )
          .await
        }
        router::ApiRoutes::Shutdown => trigger_shutdown(shutdown_notifier).await,
      },
    },
    Err(err) => {
      let mut response = Response::new(empty_body());
      match err {
        router::RoutingErr::Unmatched => {
          *response.status_mut() = StatusCode::NOT_FOUND;
        }
        router::RoutingErr::InvalidMethod => {
          *response.status_mut() = StatusCode::METHOD_NOT_ALLOWED;
        }
        router::RoutingErr::InvalidRequest(e) => {
          *response.status_mut() = StatusCode::BAD_REQUEST;
          *response.body_mut() = full_body(format!("request invalid: {e}"));
        }
      };
      Ok(response)
    }
  }
}
