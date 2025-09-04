use log::{error, info, warn};
use std::{fmt::Display, path::PathBuf};

use crate::{
  common::semver::Semver,
  frontend::{
    pkg::repository::PackagesRepository,
    releases::{
      Release, ReleaseFetchErr, Version, fetch_remote_frontend_package_release, get_remote_release,
    },
  },
};

pub mod pkg;
pub mod releases;

pub const DEFAULT_ENTRYPOINT_FILE_NAME: &str = "index.html";

pub async fn init_frontend(
  pkg: Option<PathBuf>,
  update: bool,
  force_outdated: bool,
  pkgs_repository: &mut PackagesRepository,
) -> Result<(), String> {
  pkgs_repository.init().await;

  let mut pkg_path = pkg;
  if pkg_path.is_none()
    && let Some(new_release) = remote_frontend_release_available(update, pkgs_repository).await
  {
    info!(
      "fetching new frontend package version \"{}\"",
      new_release.name
    );
    pkg_path = fetch_new_frontend_release(&new_release).await;
  }

  if let Some(path) = pkg_path {
    pkgs_repository
      .install_package(path, force_outdated)
      .await
      .map_err(|err| format!("frontend package install failed: {err}"))?;
  }

  match check_frontend_pkg(pkgs_repository).await {
    Ok(_) => Ok(()),
    Err(err) => Err(format!("frontend init failed: {err}")),
  }
}

async fn fetch_new_frontend_release(new_release: &Release) -> Option<PathBuf> {
  match fetch_remote_frontend_package_release(new_release).await {
    Ok(path_pkg) => Some(path_pkg),
    Err(err) => {
      error!("fetch of remote frontend package failed: {err}");
      None
    }
  }
}

async fn remote_frontend_release_available(
  allow_updates: bool,
  pkgs_repository: &PackagesRepository,
) -> Option<Release> {
  match check_for_newer_remote_release(pkgs_repository).await {
    Ok(result) => match result {
      RemoteReleaseCheckResult::UpToDate(local) => {
        info!("local frontend version \"{local}\" is up to date");
        None
      }
      RemoteReleaseCheckResult::NewerRemoteAvailable(new_release) => {
        if allow_updates {
          Some(new_release)
        } else {
          info!(
            "newer frontend release \"{}\" is available - run the program with \"--update\" argument to install it",
            new_release.name
          );
          None
        }
      }
      RemoteReleaseCheckResult::RemoteNecessary(release) => Some(release),
    },
    Err(err) => {
      error!("check for the latest remote package failed: {err}");
      None
    }
  }
}

pub async fn check_frontend_pkg(pkgs_repo: &PackagesRepository) -> Result<(), FrontendPkgErr> {
  let frontend_entrypoint = match pkgs_repo.get_installed() {
    Ok(pkg) => &pkg.manifest.version_info.entrypoint,
    Err(err) => {
      return Err(FrontendPkgErr::PkgInvalid(format!(
        "cannot read manifest file in project home directory: {err}"
      )));
    }
  };

  let frontend_entrypoint_path = frontend_entrypoint
    .as_deref()
    .unwrap_or(DEFAULT_ENTRYPOINT_FILE_NAME);
  match pkgs_repo.get_installed_file(frontend_entrypoint_path).await {
    Ok(_) => Ok(()),
    Err(err) => Err(FrontendPkgErr::EntrypointNotFound(format!(
      "entrypoint file {frontend_entrypoint_path} does not exist in project home directory: {err}"
    ))),
  }
}

enum RemoteReleaseCheckResult {
  UpToDate(Semver),
  NewerRemoteAvailable(Release),
  RemoteNecessary(Release),
}
async fn check_for_newer_remote_release(
  pkgs_repo: &PackagesRepository,
) -> Result<RemoteReleaseCheckResult, FrontendPkgErr> {
  let release = get_remote_release(Version::Latest)
    .await
    .map_err(FrontendPkgErr::RemoteReleaseCheckFailure)?;

  info!(
    "the latest remote frontend version is \"{}\"",
    release.version
  );
  let remote_version = release.version;
  let local_version = match pkgs_repo.get_installed() {
    Ok(installed) => installed.manifest.version_info.version,
    Err(_) => {
      warn!("could not infer local frontend package version");
      return Ok(RemoteReleaseCheckResult::RemoteNecessary(release));
    }
  };

  if local_version >= remote_version {
    Ok(RemoteReleaseCheckResult::UpToDate(local_version))
  } else {
    info!(
      "local frontend version \"{local_version}\" is outdated, the newer remote version is \"{remote_version}\""
    );
    Ok(RemoteReleaseCheckResult::NewerRemoteAvailable(release))
  }
}

pub enum FrontendPkgErr {
  EntrypointNotFound(String),
  PkgInstallFailed(String),
  PkgUnpackErr(String),
  PkgInvalid(String),
  PkgOutdated(String, String),
  ManifestInvalid(String),
  PackageUnavailable(String),
  HomeDirInaccessible(std::io::Error),
  RemoteReleaseCheckFailure(ReleaseFetchErr),
}

impl Display for FrontendPkgErr {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      FrontendPkgErr::PkgInvalid(err) => write!(f, "provided pkg file is invalid: {err}"),
      FrontendPkgErr::PkgInstallFailed(err) => write!(f, "package install failed: {err}"),
      FrontendPkgErr::EntrypointNotFound(error) => write!(
        f,
        "frontend cannot be served due to lack of an entrypoint file: {error}"
      ),
      FrontendPkgErr::HomeDirInaccessible(error) => {
        write!(f, "the program could not read it's home directory: {error}")
      }
      FrontendPkgErr::PkgUnpackErr(error) => {
        write!(f, "frontend package could not be unpacked: {error}")
      }
      FrontendPkgErr::PkgOutdated(tmp_version, home_version) => write!(
        f,
        "provided frontend package has outdated version \"{tmp_version}\" compared to currently installed version \"{home_version}\""
      ),
      FrontendPkgErr::ManifestInvalid(msg) => {
        write!(f, "frontend package manifest is in incorrect format: {msg}")
      }
      FrontendPkgErr::PackageUnavailable(msg) => {
        write!(f, "frontend package is not available: {msg}")
      }
      FrontendPkgErr::RemoteReleaseCheckFailure(err) => {
        write!(f, "check for the latest version failed: {err}")
      }
    }
  }
}
