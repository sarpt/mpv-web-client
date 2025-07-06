use std::error::Error;
use std::net::{Ipv4Addr, SocketAddr};

use futures::StreamExt;
use http_body_util::combinators::BoxBody;
use http_body_util::{BodyExt, Empty, StreamBody};
use hyper::body::{Bytes, Frame};
use hyper::header::HeaderValue;
use hyper::service::service_fn;
use hyper::{Request, Response, StatusCode};
use hyper_util::rt::{TokioExecutor, TokioIo};
use hyper_util::server::conn::auto;
use tokio::io::BufReader;
use tokio::net::TcpListener;
use tokio_util::io::ReaderStream;

use crate::frontend::releases::check_latest_remote_release;
use crate::frontend::{INDEX_FILE_NAME, get_frontend_file};
use crate::server::common::ServiceError;
use crate::server::router::get_route;

mod common;
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

const STREAM_CHUNK_SIZE: usize = 1024 * 1024 * 64;
async fn serve_frontend(
  name: Option<&str>,
) -> Result<Response<BoxBody<Bytes, ServiceError>>, ServiceError> {
  let src_name = match name {
    Some(name) => name,
    None => INDEX_FILE_NAME,
  };
  let (src_file, src_path) = match get_frontend_file(&src_name).await {
    Ok((src_file, src_path)) => (src_file, src_path),
    Err(err) => {
      let name: &str = src_name;
      if name == INDEX_FILE_NAME {
        return Err(Box::new(err));
      }

      // fallback to index file on unmatched paths
      // required for BrowserRouter in mpv-web-frontend
      match get_frontend_file(INDEX_FILE_NAME).await {
        Ok((src_file, src_path)) => (src_file, src_path),
        Err(err) => return Err(Box::new(err)),
      }
    }
  };
  let reader = BufReader::with_capacity(STREAM_CHUNK_SIZE, src_file);
  let reader_stream = ReaderStream::new(reader).map(|chunk| match chunk {
    Ok(bytes) => Ok(Frame::data(bytes)),
    Err(err) => Err(Box::new(err).into()),
  });

  let media_type = mime_guess::from_path(&src_path);
  let mime_type = media_type
    .first()
    .unwrap_or(mime_guess::mime::APPLICATION_OCTET_STREAM);

  let mut response = Response::new(BoxBody::new(StreamBody::new(reader_stream)));
  response.headers_mut().append(
    "Content-Type",
    HeaderValue::from_str(mime_type.as_ref()).unwrap(),
  );
  Ok(response)
}

async fn check_latest_frontend_release()
-> Result<Response<BoxBody<Bytes, ServiceError>>, ServiceError> {
  let response = match check_latest_remote_release().await {
    Ok(latest_release) => {
      let release_text = serde_json::to_string(&latest_release).map_err(Box::new)?;
      let body = BoxBody::new(release_text).map_err(|e| match e {}).boxed();
      let mut response = Response::new(body);
      response.headers_mut().append(
        "Content-Type",
        HeaderValue::from_str(mime_guess::mime::APPLICATION_JSON.as_ref()).unwrap(),
      );
      response
    }
    Err(err) => {
      let body = BoxBody::new(format!("could not fetch latest release: {err}"))
        .map_err(|e| match e {})
        .boxed();
      let mut response = Response::new(body);
      *response.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
      response
    }
  };

  Ok(response)
}

fn empty() -> BoxBody<Bytes, ServiceError> {
  Empty::<Bytes>::new()
    .map_err(|never| match never {})
    .boxed()
}
