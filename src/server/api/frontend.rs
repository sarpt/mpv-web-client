use hyper::{Response, StatusCode};
use serde::{Deserialize, Serialize};

use crate::{
  common::semver::Semver,
  frontend::{
    pkg::repository::PackagesRepository,
    releases::{Release, Version, fetch_remote_frontend_package_release, get_remote_release},
  },
  server::{
    api::ApiErr,
    common::{ServiceResponse, empty_body, json_response},
  },
};

#[derive(Serialize)]
pub struct CheckLatestResponseBody {
  latest_release: Release,
  local_version: Option<Semver>,
  should_update: bool,
}

pub async fn check_latest_frontend_release(pkgs_repo: &PackagesRepository) -> ServiceResponse {
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

#[derive(Deserialize)]
pub struct FrontendUpdateRequest {
  version: Semver,
}

pub async fn update_frontend_package(
  req: FrontendUpdateRequest,
  pkgs_repo: &mut PackagesRepository,
) -> ServiceResponse {
  let release = match get_remote_release(Version::Semver(req.version)).await {
    Ok(release) => release,
    Err(err) => {
      let body = serde_json::to_string(&ApiErr {
        err_msg: format!(
          "could not fetch release info for version {}: {err}",
          req.version
        ),
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
        err_msg: format!("could not fetch the \"{}\" release: {err}", req.version),
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
        err_msg: format!("could not fetch the \"{}\" release: {err}", req.version),
      })?;
      let mut response = json_response(body);
      *response.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
      Ok(response)
    }
  }
}
