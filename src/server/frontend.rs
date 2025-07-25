use futures::StreamExt;
use http_body_util::StreamBody;
use http_body_util::combinators::BoxBody;
use hyper::Response;
use hyper::body::{Bytes, Frame};
use hyper::header::HeaderValue;
use tokio::io::BufReader;
use tokio_util::io::ReaderStream;

use crate::frontend::{INDEX_FILE_NAME, get_frontend_file};
use crate::server::common::ServiceError;

const STREAM_CHUNK_SIZE: usize = 1024 * 1024 * 64;
pub async fn serve_frontend(
  name: Option<&str>,
  encodings: Vec<String>,
) -> Result<Response<BoxBody<Bytes, ServiceError>>, ServiceError> {
  let mut src_name = String::from(match name {
    Some(name) => name,
    None => INDEX_FILE_NAME,
  });

  if let Some(ext) = decide_encoding_extension(encodings) {
    src_name.push_str(ext);
  }

  let (src_file, src_path) = match get_frontend_file(&src_name).await {
    Ok((src_file, src_path)) => (src_file, src_path),
    Err(err) => {
      if src_name == INDEX_FILE_NAME {
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

const GZIP_EXT: &str = ".gz";
const GZIP_ENCODING: &str = "gzip";
const ANY_ENCODING: &str = "*";
fn decide_encoding_extension(encodings: Vec<String>) -> Option<&'static str> {
  let should_serve_gzip = encodings
    .iter()
    .any(|en| en == GZIP_ENCODING || en == ANY_ENCODING);
  if should_serve_gzip {
    Some(GZIP_EXT)
  } else {
    None
  }
}
