use std::collections::VecDeque;
use std::path::{Path, PathBuf};

use futures::StreamExt;
use http_body_util::StreamBody;
use http_body_util::combinators::BoxBody;
use hyper::Response;
use hyper::body::{Bytes, Frame};
use hyper::header::HeaderValue;
use log::debug;
use mime_guess::Mime;
use tokio::fs::File;
use tokio::io::BufReader;
use tokio_util::io::ReaderStream;

use crate::frontend::DEFAULT_ENTRYPOINT_FILE_NAME;
use crate::frontend::pkg::repository::PackagesRepository;
use crate::server::common::ServiceError;

const STREAM_CHUNK_SIZE: usize = 1024 * 1024 * 64;
pub async fn serve_frontend(
  name: Option<&str>,
  encodings: Vec<String>,
  pkgs_repo: &PackagesRepository,
) -> Result<Response<BoxBody<Bytes, ServiceError>>, ServiceError> {
  let file_to_serve = match decide_file_to_serve(name, &encodings, pkgs_repo).await {
    Some(served_file_info) => served_file_info,
    None => {
      return Err(*Box::<ServiceError>::new(
        "unable to serve any of the expected files for request"
          .to_owned()
          .into(),
      ));
    }
  };

  debug!("serving path \"{}\"", file_to_serve.path.to_string_lossy());
  let reader = BufReader::with_capacity(STREAM_CHUNK_SIZE, file_to_serve.file);
  let reader_stream = ReaderStream::new(reader).map(|chunk| match chunk {
    Ok(bytes) => Ok(Frame::data(bytes)),
    Err(err) => Err(Box::new(err).into()),
  });

  let mut response = Response::new(BoxBody::new(StreamBody::new(reader_stream)));
  response.headers_mut().append(
    "Content-Type",
    HeaderValue::from_str(file_to_serve.meta.mime.as_ref()).unwrap(),
  );

  if let Some(encoding) = file_to_serve.meta.encoding {
    response
      .headers_mut()
      .append("Content-Encoding", HeaderValue::from_str(encoding).unwrap());
  }

  Ok(response)
}

struct ServedFileMeta {
  mime: Mime,
  file_name: String,
  encoding: Option<&'static str>,
}

struct ServedFile {
  file: File,
  path: PathBuf,
  meta: ServedFileMeta,
}

async fn decide_file_to_serve(
  name: Option<&str>,
  encodings: &[String],
  pkgs_repo: &PackagesRepository,
) -> Option<ServedFile> {
  let mut file_candidates: VecDeque<ServedFileMeta> = VecDeque::new();
  // fallback to entrypoint on unmatched paths, with additional fallback to default index name
  // required for BrowserRouter in mpv-web-frontend
  let entrypoint_fallback_name = match pkgs_repo.get_installed() {
    Ok(pkg) => pkg
      .manifest
      .version_info
      .entrypoint
      .as_deref()
      .unwrap_or(DEFAULT_ENTRYPOINT_FILE_NAME),
    Err(_) => DEFAULT_ENTRYPOINT_FILE_NAME,
  };
  let (entrypoint_mime_type, entrypoint_encoding) =
    file_mime_and_encoding(entrypoint_fallback_name);
  file_candidates.push_back(ServedFileMeta {
    file_name: entrypoint_fallback_name.to_owned(),
    mime: entrypoint_mime_type.clone(),
    encoding: entrypoint_encoding,
  });
  if entrypoint_encoding.is_none() && should_file_be_encoded(&entrypoint_mime_type)
    && let Some((ext, encoding)) = decide_encoding_extension(encodings) {
      file_candidates.push_front(ServedFileMeta {
        file_name: format!("{entrypoint_fallback_name}.{ext}"),
        mime: entrypoint_mime_type,
        encoding: Some(encoding),
      });
    }

  if let Some(name) = name {
    let (file_mime_type, file_encoding) = file_mime_and_encoding(name);
    file_candidates.push_front(ServedFileMeta {
      mime: file_mime_type.clone(),
      file_name: name.to_owned(),
      encoding: file_encoding,
    });
    if file_encoding.is_none() && should_file_be_encoded(&file_mime_type)
      && let Some((ext, encoding)) = decide_encoding_extension(encodings) {
        file_candidates.push_front(ServedFileMeta {
          mime: file_mime_type,
          file_name: format!("{name}.{ext}"),
          encoding: Some(encoding),
        });
      }
  };

  let mut src_file_opt: Option<ServedFile> = None;
  for file_candidate in file_candidates {
    let src_file_name = &file_candidate.file_name;
    match pkgs_repo.get_installed_file(src_file_name).await {
      Ok((file, path)) => {
        src_file_opt = Some(ServedFile {
          file,
          path,
          meta: file_candidate,
        });
        break;
      }
      Err(err) => {
        debug!("could not serve a file \"{src_file_name}\", reason: {err}");
      }
    };
  }

  src_file_opt
}

const ENCODABLE_MIMES: [Mime; 6] = [
  mime_guess::mime::APPLICATION_JAVASCRIPT,
  mime_guess::mime::APPLICATION_JAVASCRIPT_UTF_8,
  mime_guess::mime::APPLICATION_OCTET_STREAM,
  mime_guess::mime::TEXT_JAVASCRIPT,
  mime_guess::mime::TEXT_HTML,
  mime_guess::mime::TEXT_HTML_UTF_8,
];
fn should_file_be_encoded(mime_type: &Mime) -> bool {
  ENCODABLE_MIMES
    .iter()
    .any(|encodable| mime_type == encodable)
}

const GZIP_EXT: &str = "gz";
const GZIP_ENCODING: &str = "gzip";
const ANY_ENCODING: &str = "*";
fn decide_encoding_extension(encodings: &[String]) -> Option<(&'static str, &'static str)> {
  let should_serve_gzip = encodings
    .iter()
    .any(|en| en == GZIP_ENCODING || en == ANY_ENCODING);
  if should_serve_gzip {
    Some((GZIP_EXT, GZIP_ENCODING))
  } else {
    None
  }
}

fn file_mime_and_encoding<T>(name: T) -> (Mime, Option<&'static str>)
where
  T: AsRef<Path>,
{
  let encoding = encoding_for_name(&name);
  let file_type = match encoding {
    Some(_) => {
      let name_without_ext = name.as_ref().file_stem();
      match name_without_ext {
        Some(name) => mime_guess::from_path(name),
        None => mime_guess::from_path(name),
      }
    }
    None => mime_guess::from_path(name),
  };
  let mime_type = file_type
    .first()
    .unwrap_or(mime_guess::mime::APPLICATION_OCTET_STREAM);

  (mime_type, encoding)
}

fn encoding_for_name<T>(name: T) -> Option<&'static str>
where
  T: AsRef<Path>,
{
  let extension = name.as_ref().extension()?;
  if extension == GZIP_EXT {
    Some(GZIP_ENCODING)
  } else {
    None
  }
}
