use http_body_util::combinators::BoxBody;
use hyper::body::Bytes;
use hyper::header::HeaderValue;
use hyper::{Response, StatusCode};

use crate::frontend::install_package;
use crate::frontend::releases::{
  Release, check_latest_remote_release, fetch_remote_frontend_package_release,
};
use crate::server::common::{ServiceError, empty_body, full_body};

pub async fn check_latest_frontend_release()
-> Result<Response<BoxBody<Bytes, ServiceError>>, ServiceError> {
  let response = match check_latest_remote_release().await {
    Ok(latest_release) => {
      let release_text = serde_json::to_string(&latest_release).map_err(Box::new)?;
      let body = full_body(release_text);
      let mut response = Response::new(body);
      response.headers_mut().append(
        "Content-Type",
        HeaderValue::from_str(mime_guess::mime::APPLICATION_JSON.as_ref()).unwrap(),
      );
      response
    }
    Err(err) => {
      let body = full_body(format!("could not fetch latest release: {err}"));
      let mut response = Response::new(body);
      *response.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
      response
    }
  };

  Ok(response)
}

pub async fn update_frontend_package(
  release: Release,
) -> Result<Response<BoxBody<Bytes, ServiceError>>, ServiceError> {
  let path = match fetch_remote_frontend_package_release(&release).await {
    Ok(path) => path,
    Err(err) => {
      let body = full_body(format!("could not fetch the release: {err}"));
      let mut response = Response::new(body);
      *response.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
      return Ok(response);
    }
  };

  const FORCE_OUTDATED: bool = true; // TODO: this should be provided from frontend. atm always force outdated pkg
  match install_package(path, FORCE_OUTDATED).await {
    Ok(()) => {
      let response = Response::new(empty_body());
      Ok(response)
    }
    Err(err) => {
      let body = full_body(format!("could not install the release: {err}"));
      let mut response = Response::new(body);
      *response.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
      Ok(response)
    }
  }
}
