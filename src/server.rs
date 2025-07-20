use std::error::Error;
use std::net::{Ipv4Addr, SocketAddr};
use std::ops::Deref;
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
use tokio::sync::Notify;
use tokio::time::sleep;

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

const DEFAULT_PORT: u16 = 3000;
const DEFAULT_IPADDR: [u8; 4] = [127, 0, 0, 1];
const GRACEFUL_SHUTDOWN_TIMEOUT_SEC: u64 = 30;
const IDLE_SHUTDOWN_TIMEOUT: u64 = 60;

pub async fn serve() -> Result<(), Box<dyn Error>> {
  let addr = SocketAddr::from((Ipv4Addr::from(DEFAULT_IPADDR), DEFAULT_PORT));
  let listener = TcpListener::bind(addr).await?;
  let graceful = graceful::GracefulShutdown::new();
  let main_service_shutdown_notifier = Arc::new(Notify::new());

  info!("accepting connections at {addr}");
  loop {
    let shutdown_notifier = main_service_shutdown_notifier.clone();

    select! {
      Ok((stream, incoming_addr)) = listener.accept() => {
        debug!("accepted connection from {incoming_addr}");

        tokio::task::spawn(async move {
          let io = TokioIo::new(stream);
          let runner = auto::Builder::new(TokioExecutor::new());
          _ = runner.serve_connection(io, service_fn(|req| { service(req, shutdown_notifier.clone()) })).await;
        });
      }
      _ = wait_for_shutdown_condition(shutdown_notifier.clone()) => {
        drop(listener);
        break;
      }
    }
  }

  select! {
    _ = graceful.shutdown() => {
      info!("server shut down gracefully")
    },
    _ = sleep(Duration::from_secs(GRACEFUL_SHUTDOWN_TIMEOUT_SEC)) => {
      return Err(*Box::new(format!("could not finish graceful shutdown in {GRACEFUL_SHUTDOWN_TIMEOUT_SEC} seconds").into()));
    }
  }

  Ok(())
}

async fn wait_for_shutdown_condition<T>(service_shutdown_notify: T)
where
  T: Deref<Target = Notify>,
{
  select! {
    _ = service_shutdown_notify.notified() => {
      info!("triggering shutdown due to shutdown request")
    }
    _ = tokio::signal::ctrl_c() => {
      info!("triggering shutdown due to SIGINT signal")
    }
    _ = sleep(Duration::from_secs(IDLE_SHUTDOWN_TIMEOUT)) => {
      info!("triggering shutdown since no request has been received for {IDLE_SHUTDOWN_TIMEOUT} seconds")
    }
  }
}

async fn service<T>(
  req: Request<hyper::body::Incoming>,
  shutdown_notifier: T,
) -> Result<Response<BoxBody<Bytes, ServiceError>>, ServiceError>
where
  T: Deref<Target = Notify>,
{
  let route = get_route(req).await;

  match route {
    Ok(r) => match r {
      router::Routes::Frontend(name) => serve_frontend(name.as_deref()).await,
      router::Routes::Api(api_route) => match api_route {
        router::ApiRoutes::FrontendLatest => check_latest_frontend_release().await,
        router::ApiRoutes::FrontendUpdate(release) => update_frontend_package(release).await,
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
