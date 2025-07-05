use serde::Deserialize;
use std::{fs::rename, io::Read, path::Path};

use crate::{
  frontend::{FrontendPkgErr, pkg::manifest::semver::Semver},
  project_paths::{get_frontend_dir, get_frontend_temp_dir, get_project_home_dir},
};

pub const PKG_MANIFEST_NAME: &str = "pkg_manifest.toml";

pub mod semver;

#[derive(Deserialize, PartialEq)]
pub struct VersionInfo {
  pub version: Semver,
  pub commit: String,
}

#[derive(Deserialize)]
pub struct Manifest {
  pub version_info: VersionInfo,
}

pub fn parse_temp_package_manifest() -> Result<Manifest, FrontendPkgErr> {
  let mut path = get_frontend_temp_dir();
  path.push(PKG_MANIFEST_NAME);
  parse_package_manifest(path)
}

pub fn parse_project_package_manifest() -> Result<Manifest, FrontendPkgErr> {
  let mut path = get_project_home_dir().map_err(FrontendPkgErr::HomeDirInaccessible)?;
  path.push(PKG_MANIFEST_NAME);
  parse_package_manifest(path)
}

fn parse_package_manifest<T>(path: T) -> Result<Manifest, FrontendPkgErr>
where
  T: AsRef<Path>,
{
  let mut package_file = std::fs::OpenOptions::new()
    .create(false)
    .truncate(false)
    .read(true)
    .write(false)
    .open(&path)
    .map_err(FrontendPkgErr::PkgUnpackErr)?;

  let mut toml_content = String::new();
  package_file
    .read_to_string(&mut toml_content)
    .map_err(|err| FrontendPkgErr::PkgInvalid(err.to_string()))?;
  let manifest: Manifest = toml::from_str(toml_content.as_ref())
    .map_err(|err| FrontendPkgErr::PkgInvalid(err.to_string()))?;

  Ok(manifest)
}

pub fn move_manifest_to_project_home() -> Result<(), FrontendPkgErr> {
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
  rename(manifest_file_path, new_manifest_file_path).map_err(FrontendPkgErr::HomeDirInaccessible)
}
