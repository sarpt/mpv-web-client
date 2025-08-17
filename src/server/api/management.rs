use std::ops::Deref;

use http_body_util::combinators::BoxBody;
use hyper::{Response, body::Bytes};
use tokio::sync::Notify;

use crate::server::common::{ServiceError, empty_body};

pub async fn trigger_shutdown<T>(
  notifier: T,
) -> Result<Response<BoxBody<Bytes, ServiceError>>, ServiceError>
where
  T: Deref<Target = Notify>,
{
  notifier.notify_waiters();
  let response = Response::new(empty_body());
  Ok(response)
}
