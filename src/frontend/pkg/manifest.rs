use serde::Deserialize;
use std::path::Path;
use tokio::{
  fs::{OpenOptions, rename},
  io::AsyncReadExt,
};

use crate::{
  common::semver::Semver,
  frontend::FrontendPkgErr,
  project_paths::{get_frontend_dir, get_frontend_temp_dir, get_project_home_dir},
};

pub const PKG_MANIFEST_NAME: &str = "pkg_manifest.toml";

#[derive(Deserialize, PartialEq)]
pub struct VersionInfo {
  pub version: Semver,
  pub commit: String,
}

#[derive(Deserialize)]
pub struct Manifest {
  pub version_info: VersionInfo,
}

pub async fn parse_temp_package_manifest() -> Result<Manifest, FrontendPkgErr> {
  let mut path = get_frontend_temp_dir();
  path.push(PKG_MANIFEST_NAME);
  parse_package_manifest(path).await
}

pub async fn parse_project_package_manifest() -> Result<Manifest, FrontendPkgErr> {
  let mut path = get_project_home_dir().map_err(FrontendPkgErr::HomeDirInaccessible)?;
  path.push(PKG_MANIFEST_NAME);
  parse_package_manifest(path).await
}

pub async fn move_manifest_to_project_home() -> Result<(), FrontendPkgErr> {
  let frontend_dir = get_frontend_dir().map_err(FrontendPkgErr::HomeDirInaccessible)?;
  let manifest_file_path = {
    let mut path = frontend_dir.clone();
    path.push(PKG_MANIFEST_NAME);
    path
  };
  let new_manifest_file_path = {
    let mut path = get_project_home_dir().map_err(FrontendPkgErr::HomeDirInaccessible)?;
    path.push(PKG_MANIFEST_NAME);
    path
  };
  rename(manifest_file_path, new_manifest_file_path)
    .await
    .map_err(FrontendPkgErr::HomeDirInaccessible)
}

async fn parse_package_manifest<T>(path: T) -> Result<Manifest, FrontendPkgErr>
where
  T: AsRef<Path>,
{
  let mut package_file = OpenOptions::new()
    .create(false)
    .truncate(false)
    .read(true)
    .write(false)
    .open(&path)
    .await
    .map_err(FrontendPkgErr::PkgUnpackErr)?;

  let mut toml_content = String::new();
  package_file
    .read_to_string(&mut toml_content)
    .await
    .map_err(|err| FrontendPkgErr::ManifestInvalid(err.to_string()))?;
  let manifest: Manifest = toml::from_str(toml_content.as_ref())
    .map_err(|err| FrontendPkgErr::ManifestInvalid(err.to_string()))?;

  Ok(manifest)
}
