use std::env;
use std::error::Error;
use std::fmt::Display;
use std::net::{Ipv4Addr, SocketAddr};
use std::path::Path;

use futures::StreamExt;
use http_body_util::combinators::BoxBody;
use http_body_util::{BodyExt, Empty, StreamBody};
use hyper::body::{Bytes, Frame};
use hyper::service::service_fn;
use hyper::{Request, Response, StatusCode};
use hyper_util::rt::{TokioExecutor, TokioIo};
use hyper_util::server::conn::auto;
use route_recognizer::Router;
use tokio::fs::OpenOptions;
use tokio::io::BufReader;
use tokio::net::TcpListener;
use tokio_util::io::ReaderStream;

const DEFAULT_PORT: u16 = 3000;
const DEFAULT_IPADDR: [u8; 4] = [127, 0, 0, 1];

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn Error>> {
    let addr = SocketAddr::from((Ipv4Addr::from(DEFAULT_IPADDR), DEFAULT_PORT));
    let listener = TcpListener::bind(addr).await?;

    loop {
        let (stream, _) = listener.accept().await?;

        let io = TokioIo::new(stream);
        tokio::task::spawn(async move {
            let runner = auto::Builder::new(TokioExecutor::new());
            _ = runner.serve_connection(io, service_fn(router)).await;
        });
    }
}

enum Routes {
    Frontend,
}

type ServiceError = Box<dyn std::error::Error + Send + Sync>;
async fn router(
    req: Request<hyper::body::Incoming>,
) -> Result<Response<BoxBody<Bytes, ServiceError>>, ServiceError> {
    let mut router = Router::new();

    router.add("/frontend/*name", Routes::Frontend);
    router.add("/frontend", Routes::Frontend);
    router.add("/frontend/", Routes::Frontend);

    let match_result = router.recognize(req.uri().path());

    let routes = match match_result {
        Ok(m) => m,
        Err(_) => {
            let mut not_found = Response::new(empty());
            *not_found.status_mut() = StatusCode::NOT_FOUND;
            return Ok(not_found);
        }
    };

    match routes.handler() {
        Routes::Frontend => {
            serve_frontend(routes.params().find("name").unwrap_or("index.html")).await
        }
    }
}

const STREAM_CHUNK_SIZE: usize = 1024 * 1024 * 128;
const HOME_SUBDIR: &str = ".mwc";
async fn serve_frontend<T>(name: T) -> Result<Response<BoxBody<Bytes, ServiceError>>, ServiceError>
where
    T: AsRef<str> + Display + AsRef<Path>,
{
    let mut tgt_path = match env::home_dir() {
        Some(path) => path,
        None => {
            return Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "could not resolve home path",
            )));
        }
    };

    tgt_path.push(HOME_SUBDIR);
    tgt_path.push(name);

    let src_file_open_result = OpenOptions::default()
        .create(false)
        .read(true)
        .write(false)
        .open(tgt_path)
        .await;

    let src_file = match src_file_open_result {
        Ok(src_file) => src_file,
        Err(err) => return Err(Box::new(err)),
    };

    let reader = BufReader::with_capacity(STREAM_CHUNK_SIZE, src_file);
    let reader_stream = ReaderStream::new(reader).map(|chunk| match chunk {
        Ok(bytes) => Ok(Frame::data(bytes)),
        Err(err) => Err(Box::new(err).into()),
    });
    Ok(Response::new(BoxBody::new(StreamBody::new(reader_stream))))
}

fn empty() -> BoxBody<Bytes, ServiceError> {
    Empty::<Bytes>::new()
        .map_err(|never| match never {})
        .boxed()
}
