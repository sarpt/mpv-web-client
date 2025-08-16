use std::ops::Deref;

use http_body_util::combinators::BoxBody;
use hyper::body::Bytes;
use hyper::{Response, StatusCode};
use serde::Serialize;
use tokio::sync::Notify;

use crate::api::ApiServersService;
use crate::common::semver::Semver;
use crate::frontend::pkg::repository::PackagesRepository;
use crate::frontend::releases::{
  Release, Version, fetch_remote_frontend_package_release, get_remote_release,
};
use crate::server::common::{ServiceError, empty_body, json_response};

#[derive(Serialize)]
struct CheckLatestResponseBody {
  latest_release: Release,
  local_version: Option<Semver>,
  should_update: bool,
}

#[derive(Serialize)]
struct ApiErr {
  err_msg: String,
}

pub async fn check_latest_frontend_release(
  pkgs_repo: &PackagesRepository,
) -> Result<Response<BoxBody<Bytes, ServiceError>>, ServiceError> {
  let response = match get_remote_release(Version::Latest).await {
    Ok(latest_release) => {
      let local_version = pkgs_repo.get_installed().map_or(None, |installed| {
        Some(installed.manifest.version_info.version)
      });
      let response_body = CheckLatestResponseBody {
        should_update: local_version.is_none_or(|local| local < latest_release.version),
        latest_release,
        local_version,
      };
      let body = serde_json::to_string(&response_body).map_err(Box::new)?;
      json_response(body)
    }
    Err(err) => {
      let body = serde_json::to_string(&ApiErr {
        err_msg: format!("could not fetch latest release: {err}"),
      })?;
      let mut response = json_response(body);
      *response.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
      response
    }
  };

  Ok(response)
}

pub async fn update_frontend_package(
  version: Semver,
  pkgs_repo: &mut PackagesRepository,
) -> Result<Response<BoxBody<Bytes, ServiceError>>, ServiceError> {
  let release = match get_remote_release(Version::Semver(version)).await {
    Ok(release) => release,
    Err(err) => {
      let body = serde_json::to_string(&ApiErr {
        err_msg: format!("could not fetch release info for version {version}: {err}"),
      })?;
      let mut response = json_response(body);
      *response.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
      return Ok(response);
    }
  };

  let path = match fetch_remote_frontend_package_release(&release).await {
    Ok(path) => path,
    Err(err) => {
      let body = serde_json::to_string(&ApiErr {
        err_msg: format!("could not fetch the \"{version}\" release: {err}"),
      })?;
      let mut response = json_response(body);
      *response.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
      return Ok(response);
    }
  };

  const FORCE_OUTDATED: bool = true; // TODO: this should be provided from frontend. atm always force outdated pkg
  match pkgs_repo.install_package(path, FORCE_OUTDATED).await {
    Ok(()) => {
      let response = Response::new(empty_body());
      Ok(response)
    }
    Err(err) => {
      let body = serde_json::to_string(&ApiErr {
        err_msg: format!("could not fetch the \"{version}\" release: {err}"),
      })?;
      let mut response = json_response(body);
      *response.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
      Ok(response)
    }
  }
}

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

pub fn spawn_local_server(
  name: String,
  servers_service: &mut ApiServersService,
) -> Result<Response<BoxBody<Bytes, ServiceError>>, ServiceError> {
  match servers_service.spawn(name) {
    Ok(()) => {
      let response = Response::new(empty_body());
      Ok(response)
    }
    Err(err) => {
      let body = serde_json::to_string(&ApiErr {
        err_msg: format!("could not spawn a new api instance: {err}"),
      })?;
      let mut response = json_response(body);
      *response.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
      Ok(response)
    }
  }
}

pub fn get_all_instances(
  servers_service: &mut ApiServersService,
) -> Result<Response<BoxBody<Bytes, ServiceError>>, ServiceError> {
  let instances: Vec<ApiServerInstance> = servers_service
    .server_instances()
    .iter()
    .map(|inst| ApiServerInstance {
      local: inst.local,
      address: &inst.address,
      name: &inst.name,
    })
    .collect();
  let body = serde_json::to_string(&ApiInstancesResponse {
    instances: &instances,
  })?;
  let response = json_response(body);
  Ok(response)
}

#[derive(Serialize)]
struct ApiServerInstance<'a> {
  pub local: bool,
  pub address: &'a str,
  pub name: &'a str,
}

#[derive(Serialize)]
struct ApiInstancesResponse<'a> {
  instances: &'a [ApiServerInstance<'a>],
}
