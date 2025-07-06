use std::error::Error;
use std::net::{Ipv4Addr, SocketAddr};

use http_body_util::combinators::BoxBody;
use http_body_util::{BodyExt, Empty};
use hyper::body::Bytes;
use hyper::service::service_fn;
use hyper::{Request, Response, StatusCode};
use hyper_util::rt::{TokioExecutor, TokioIo};
use hyper_util::server::conn::auto;
use tokio::net::TcpListener;

use crate::server::api::check_latest_frontend_release;
use crate::server::common::ServiceError;
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
  let route = get_route(&req);

  match route {
    Some(r) => match r {
      router::Routes::Frontend(name) => serve_frontend(name.as_deref()).await,
      router::Routes::Api(api_route) => match api_route {
        router::ApiRoutes::FrontendLatest => check_latest_frontend_release().await,
      },
    },
    None => {
      let mut not_found = Response::new(empty());
      *not_found.status_mut() = StatusCode::NOT_FOUND;
      Ok(not_found)
    }
  }
}

fn empty() -> BoxBody<Bytes, ServiceError> {
  Empty::<Bytes>::new()
    .map_err(|never| match never {})
    .boxed()
}
