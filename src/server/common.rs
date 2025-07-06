use std::error::Error;

use http_body_util::{BodyExt, Empty, Full, combinators::BoxBody};
use hyper::body::Bytes;

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
