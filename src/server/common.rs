use std::error::Error;

use http_body_util::{BodyExt, Empty, Full, combinators::BoxBody};
use hyper::{Response, StatusCode, body::Bytes, header::HeaderValue};

use crate::server::api::ApiErr;

pub type ServiceError = Box<dyn Error + Send + Sync>;
pub type ServiceResponse = Result<Response<BoxBody<Bytes, ServiceError>>, ServiceError>;

pub fn empty_body() -> BoxBody<Bytes, ServiceError> {
  Empty::<Bytes>::new()
    .map_err(|never| match never {})
    .boxed()
}

pub fn full_body<T>(msg: T) -> BoxBody<Bytes, ServiceError>
where
  T: Into<Bytes>,
{
  Full::<Bytes>::new(msg.into())
    .map_err(|e| match e {})
    .boxed()
}

pub fn json_response<T>(msg: T) -> Response<BoxBody<Bytes, ServiceError>>
where
  T: Into<Bytes>,
{
  let body = full_body(msg);
  let mut response = Response::new(body);

  response.headers_mut().append(
    "Content-Type",
    HeaderValue::from_str(mime_guess::mime::APPLICATION_JSON.as_ref()).unwrap(),
  );

  response
}

pub fn error_json_response<T>(msg: T) -> ServiceResponse
where
  T: AsRef<str>,
{
  let body = serde_json::to_string(&ApiErr {
    err_msg: msg.as_ref(),
  })?;
  let mut response = json_response(body);
  *response.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
  Ok(response)
}
