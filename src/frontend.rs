use flate2::bufread::GzDecoder;
use log::{info, warn};
use std::{
  fmt::Display,
  fs::{create_dir_all, exists, remove_file},
  io::{BufReader, BufWriter, Seek, copy},
  path::{Path, PathBuf},
};
use tar::Archive;

use crate::{
  frontend::{
    pkg_manifest::{
      PKG_MANIFEST_NAME, move_manifest_to_project_home, parse_project_package_manifest,
      parse_temp_package_manifest, semver::Semver,
    },
    releases::{Release, ReleaseFetchErr, check_latest_remote_release},
  },
  project_paths::{get_frontend_dir, get_frontend_temp_dir, get_project_home_dir, get_temp_dir},
};

mod pkg_manifest;
pub mod releases;

pub const INDEX_FILE_NAME: &str = "index.html";
pub async fn install_package<T>(pkg_path: T) -> Result<(), FrontendPkgErr>
where
  T: AsRef<Path> + Send + Sync + 'static,
{
  let result = tokio::task::spawn_blocking(move || extract_frontend_pkg(pkg_path)).await;
  let extract_frontend_result = match result {
    Ok(res) => res,
    Err(e) => {
      return Err(FrontendPkgErr::PkgInstallFailed(format!(
        "issue with joining on blocking task for frontend extraction: {e}"
      )));
    }
  };

  extract_frontend_result?;
  check_new_pkg_manifest_against_local_one()?;
  move_frontend_pkg_to_home()?;
  move_manifest_to_project_home()?;
  Ok(())
}

pub fn check_frontend_pkg<T>(pkg_path: Option<T>) -> Result<(), FrontendPkgErr>
where
  T: AsRef<Path>,
{
  {
    let mut path = get_frontend_dir().map_err(FrontendPkgErr::HomeDirInaccessible)?;
    path.push(INDEX_FILE_NAME);
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

const STREAM_CHUNK_SIZE: usize = 1024 * 1024 * 64;
const TEMP_INFLATED_PKG_NAME: &str = "inflated.tar";
pub fn extract_frontend_pkg<T>(src_path: T) -> Result<(), FrontendPkgErr>
where
  T: AsRef<Path>,
{
  let src_file_open_handle = std::fs::OpenOptions::new()
    .create(false)
    .read(true)
    .write(false)
    .open(&src_path)
    .map_err(|err| FrontendPkgErr::PkgInvalid(err.to_string()))?;

  let temp_inflated_path = {
    let mut temp_path = get_temp_dir();
    temp_path.push(TEMP_INFLATED_PKG_NAME);
    temp_path
  };

  let mut temp_inflated_file_open_handle = std::fs::OpenOptions::new()
    .create(true)
    .truncate(true)
    .read(true)
    .write(true)
    .open(&temp_inflated_path)
    .map_err(FrontendPkgErr::PkgUnpackErr)?;

  let src_pkg_reader = BufReader::with_capacity(STREAM_CHUNK_SIZE, src_file_open_handle);
  let mut decoder = GzDecoder::new(src_pkg_reader);
  let mut inflated_writer =
    BufWriter::with_capacity(STREAM_CHUNK_SIZE, &temp_inflated_file_open_handle);
  copy(&mut decoder, &mut inflated_writer)
    .map_err(|err| FrontendPkgErr::PkgInvalid(err.to_string()))?;
  drop(inflated_writer);

  temp_inflated_file_open_handle
    .seek(std::io::SeekFrom::Start(0))
    .map_err(FrontendPkgErr::HomeDirInaccessible)?;

  let unpack_temp_dir = get_frontend_temp_dir();
  let mut tar_archive = Archive::new(temp_inflated_file_open_handle);
  tar_archive
    .unpack(&unpack_temp_dir)
    .map_err(|err| FrontendPkgErr::PkgInvalid(err.to_string()))?;
  remove_file(temp_inflated_path).map_err(FrontendPkgErr::HomeDirInaccessible)?;

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

pub enum RemoteReleaseCheckResult {
  UpToDate(Semver),
  NewerRemoteAvailable(Release),
  RemoteNecessary(Release),
}
pub async fn check_for_newer_remote_release() -> Result<RemoteReleaseCheckResult, FrontendPkgErr> {
  let release = check_latest_remote_release()
    .await
    .map_err(FrontendPkgErr::RemoteReleaseCheckFailure)?;

  info!(
    "the latest remote frontend version is \"{}\"",
    release.tag_name
  );
  let (local_version, remote_version) = check_release_against_local_one(&release)?;
  match local_version {
    Some(local) => {
      if local >= remote_version {
        Ok(RemoteReleaseCheckResult::UpToDate(local))
      } else {
        info!(
          "local frontend version \"{local}\" is outdated, the newer remote version is \"{remote_version}\""
        );
        Ok(RemoteReleaseCheckResult::NewerRemoteAvailable(release))
      }
    }
    None => {
      warn!("could not infer local frontend package version");
      Ok(RemoteReleaseCheckResult::RemoteNecessary(release))
    }
  }
}

fn move_frontend_pkg_to_home() -> Result<(), FrontendPkgErr> {
  let frontend_temp_dir = get_frontend_temp_dir();
  let project_dir = get_project_home_dir().map_err(FrontendPkgErr::HomeDirInaccessible)?;
  for entry_result in walkdir::WalkDir::new(frontend_temp_dir) {
    let entry = match entry_result {
      Ok(e) => e,
      Err(err) => return Err(FrontendPkgErr::PkgUnpackErr(err.into())),
    };

    let mut tgt_path = project_dir.clone();
    let stripped_path = entry.path().strip_prefix(get_temp_dir()).unwrap();
    tgt_path.push(stripped_path);
    if entry.file_type().is_dir() {
      create_dir_all(tgt_path).map_err(FrontendPkgErr::PkgUnpackErr)?;
    } else if entry.file_type().is_file() {
      std::fs::copy(entry.path(), tgt_path).map_err(FrontendPkgErr::HomeDirInaccessible)?;
    }
  }

  Ok(())
}

fn check_release_against_local_one(
  release: &Release,
) -> Result<(Option<Semver>, Semver), FrontendPkgErr> {
  let release_semver = Semver::try_from(&release.name).map_err(FrontendPkgErr::ManifestInvalid)?;
  let project_manifest = match parse_project_package_manifest() {
    Ok(m) => m,
    Err(err) => {
      warn!("could not parse existing frontend package manifest: {err}");
      return Ok((None, release_semver));
    }
  };
  Ok((Some(project_manifest.version_info.version), release_semver))
}

fn check_new_pkg_manifest_against_local_one() -> Result<(), FrontendPkgErr> {
  let temp_manifest = parse_temp_package_manifest()?;
  let project_manifest = match parse_project_package_manifest() {
    Ok(m) => m,
    Err(err) => {
      warn!("could not parse existing frontend package manifest: {err}");
      return Ok(());
    }
  };

  if temp_manifest.version_info.version < project_manifest.version_info.version {
    return Err(FrontendPkgErr::PkgOutdated(
      temp_manifest.version_info.version.into(),
      project_manifest.version_info.version.into(),
    ));
  }
  Ok(())
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
