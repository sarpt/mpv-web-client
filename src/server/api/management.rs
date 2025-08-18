use std::ops::Deref;

use hyper::Response;
use tokio::sync::Notify;

use crate::server::common::{ServiceResponse, empty_body};

pub async fn trigger_shutdown<T>(notifier: T) -> ServiceResponse
where
  T: Deref<Target = Notify>,
{
  notifier.notify_waiters();
  let response = Response::new(empty_body());
  Ok(response)
}
