use std::error::Error;
use std::net::{Ipv4Addr, SocketAddr};

use http_body_util::combinators::BoxBody;
use hyper::body::Bytes;
use hyper::service::service_fn;
use hyper::{Request, Response, StatusCode};
use hyper_util::rt::{TokioExecutor, TokioIo};
use hyper_util::server::conn::auto;
use tokio::net::TcpListener;

use crate::server::api::{check_latest_frontend_release, update_frontend_package};
use crate::server::common::{ServiceError, empty_body, full_body};
use crate::server::frontend::serve_frontend;
use crate::server::router::get_route;

mod api;
mod common;
mod frontend;
mod router;

const DEFAULT_PORT: u16 = 3000;
const DEFAULT_IPADDR: [u8; 4] = [127, 0, 0, 1];

pub async fn serve() -> Result<(), Box<dyn Error>> {
  let addr = SocketAddr::from((Ipv4Addr::from(DEFAULT_IPADDR), DEFAULT_PORT));
  let listener = TcpListener::bind(addr).await?;

  loop {
    let (stream, _) = listener.accept().await?;

    tokio::task::spawn(async {
      let io = TokioIo::new(stream);
      let runner = auto::Builder::new(TokioExecutor::new());
      _ = runner.serve_connection(io, service_fn(service)).await;
    });
  }
}

async fn service(
  req: Request<hyper::body::Incoming>,
) -> Result<Response<BoxBody<Bytes, ServiceError>>, ServiceError> {
  let route = get_route(req).await;

  match route {
    Ok(r) => match r {
      router::Routes::Frontend(name) => serve_frontend(name.as_deref()).await,
      router::Routes::Api(api_route) => match api_route {
        router::ApiRoutes::FrontendLatest => check_latest_frontend_release().await,
        router::ApiRoutes::FrontendUpdate(release) => update_frontend_package(release).await,
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
