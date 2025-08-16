use std::error::Error;

use http_body_util::{BodyExt, Empty, Full, combinators::BoxBody};
use hyper::{Response, body::Bytes, header::HeaderValue};

pub type ServiceError = Box<dyn Error + Send + Sync>;

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
