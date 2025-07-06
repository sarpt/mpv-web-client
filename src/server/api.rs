use http_body_util::BodyExt;
use http_body_util::combinators::BoxBody;
use hyper::body::Bytes;
use hyper::header::HeaderValue;
use hyper::{Response, StatusCode};

use crate::frontend::releases::check_latest_remote_release;
use crate::server::common::ServiceError;

pub async fn check_latest_frontend_release()
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
