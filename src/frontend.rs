use log::{info, warn};
use std::{
  fmt::Display,
  fs::exists,
  path::{Path, PathBuf},
};

use crate::{
  common::semver::Semver,
  frontend::{
    pkg::{manifest::PKG_MANIFEST_NAME, repository::PackagesRepository},
    releases::{Release, ReleaseFetchErr, Version, get_remote_release},
  },
  project_paths::{get_frontend_dir, get_project_home_dir},
};

pub mod pkg;
pub mod releases;

pub fn check_frontend_pkg<T>(pkg_path: Option<T>) -> Result<(), FrontendPkgErr>
where
  T: AsRef<Path>,
{
  {
    let path = get_frontend_index_path().map_err(FrontendPkgErr::HomeDirInaccessible)?;
    let index_exists = exists(path).map_err(|err| FrontendPkgErr::IndexNotFound(Some(err)))?;
    if !index_exists {
      if pkg_path.is_none() {
        return Err(FrontendPkgErr::PkgNotProvided);
      } else {
        return Err(FrontendPkgErr::IndexNotFound(None));
      }
    }
  };

  {
    let mut path = get_project_home_dir().map_err(FrontendPkgErr::HomeDirInaccessible)?;
    path.push(PKG_MANIFEST_NAME);
    let manifest_exists =
      exists(path).map_err(|err| FrontendPkgErr::PkgInvalid(err.to_string()))?;
    if !manifest_exists {
      if pkg_path.is_none() {
        return Err(FrontendPkgErr::PkgNotProvided);
      } else {
        return Err(FrontendPkgErr::PkgInvalid(
          "manifest file does not exist in project home directory".to_owned(),
        ));
      }
    }
  };

  Ok(())
}

pub async fn get_frontend_file<T>(name: T) -> Result<(tokio::fs::File, PathBuf), std::io::Error>
where
  T: AsRef<Path>,
{
  let mut src_path = get_frontend_dir()?;
  src_path.push(name);

  let src_file_open_result = tokio::fs::OpenOptions::default()
    .create(false)
    .read(true)
    .write(false)
    .open(&src_path)
    .await;

  match src_file_open_result {
    Ok(src_file) => Ok((src_file, src_path)),
    Err(err) => Err(err),
  }
}

pub const INDEX_FILE_NAME: &str = "index.html";
pub fn get_frontend_index_path() -> Result<PathBuf, std::io::Error> {
  let mut path = get_frontend_dir()?;
  path.push(INDEX_FILE_NAME);
  Ok(path)
}

pub enum RemoteReleaseCheckResult {
  UpToDate(Semver),
  NewerRemoteAvailable(Release),
  RemoteNecessary(Release),
}
pub async fn check_for_newer_remote_release(
  pkgs_repo: &mut PackagesRepository,
) -> Result<RemoteReleaseCheckResult, FrontendPkgErr> {
  let release = get_remote_release(Version::Latest)
    .await
    .map_err(FrontendPkgErr::RemoteReleaseCheckFailure)?;

  info!(
    "the latest remote frontend version is \"{}\"",
    release.version
  );
  let remote_version = release.version;
  let local_version = match pkgs_repo.get_installed().await {
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
  IndexNotFound(Option<std::io::Error>),
  PkgInstallFailed(String),
  PkgNotProvided,
  PkgUnpackErr(std::io::Error),
  PkgInvalid(String),
  PkgOutdated(String, String),
  ManifestInvalid(String),
  HomeDirInaccessible(std::io::Error),
  RemoteReleaseCheckFailure(ReleaseFetchErr),
}

impl Display for FrontendPkgErr {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      FrontendPkgErr::PkgInvalid(err) => write!(f, "provided pkg file is invalid: {err}"),
      FrontendPkgErr::PkgInstallFailed(err) => write!(f, "package install failed: {err}"),
      FrontendPkgErr::IndexNotFound(error) => write!(
        f,
        "frontend cannot be served due to lack of entrypoint file: {error:?}"
      ),
      FrontendPkgErr::HomeDirInaccessible(error) => {
        write!(f, "the program could not read it's home directory: {error}")
      }
      FrontendPkgErr::PkgNotProvided => write!(
        f,
        "frontend package has not been provided and there is no cached frontend package",
      ),
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
      FrontendPkgErr::RemoteReleaseCheckFailure(err) => {
        write!(f, "check for the latest version failed: {err}")
      }
    }
  }
}
