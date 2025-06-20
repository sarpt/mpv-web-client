use std::env;
use std::error::Error;
use std::net::{Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};

use futures::StreamExt;
use http_body_util::combinators::BoxBody;
use http_body_util::{BodyExt, Empty, StreamBody};
use hyper::body::{Bytes, Frame};
use hyper::header::HeaderValue;
use hyper::service::service_fn;
use hyper::{Request, Response, StatusCode};
use hyper_util::rt::{TokioExecutor, TokioIo};
use hyper_util::server::conn::auto;
use route_recognizer::Router;
use tokio::fs::{File, OpenOptions};
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

        tokio::task::spawn(async {
            let io = TokioIo::new(stream);
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

    router.add("/*path", Routes::Frontend);
    router.add("/", Routes::Frontend);

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
        Routes::Frontend => serve_frontend(routes.params().find("path")).await,
    }
}

const INDEX_FILE_NAME: &str = "index.html";
async fn serve_frontend<'a, T>(
    name: Option<T>,
) -> Result<Response<BoxBody<Bytes, ServiceError>>, ServiceError>
where
    T: AsRef<Path> + From<&'a str> + Into<&'a str>,
{
    let src_name = match name {
        Some(name) => name,
        None => INDEX_FILE_NAME.into(),
    };
    let (src_file, src_path) = match get_frontend_file(&src_name).await {
        Ok((src_file, src_path)) => (src_file, src_path),
        Err(err) => {
            let name: &str = src_name.into();
            if name == INDEX_FILE_NAME {
                return Err(err);
            }

            match get_frontend_file(INDEX_FILE_NAME).await {
                Ok((src_file, src_path)) => (src_file, src_path),
                Err(err) => return Err(err),
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

const STREAM_CHUNK_SIZE: usize = 1024 * 1024 * 128;
async fn get_frontend_file<T>(name: T) -> Result<(File, PathBuf), ServiceError>
where
    T: AsRef<Path>,
{
    let mut src_path = get_project_home_dir()?;
    src_path.push(name);

    let src_file_open_result = OpenOptions::default()
        .create(false)
        .read(true)
        .write(false)
        .open(&src_path)
        .await;

    match src_file_open_result {
        Ok(src_file) => Ok((src_file, src_path)),
        Err(err) => Err(Box::new(err)),
    }
}

const HOME_SUBDIR: &str = ".mwc";
fn get_project_home_dir() -> Result<PathBuf, ServiceError> {
    let mut src_path = match env::home_dir() {
        Some(path) => path,
        None => {
            return Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "could not resolve home path",
            )));
        }
    };

    src_path.push(HOME_SUBDIR);
    Ok(src_path)
}

fn empty() -> BoxBody<Bytes, ServiceError> {
    Empty::<Bytes>::new()
        .map_err(|never| match never {})
        .boxed()
}
