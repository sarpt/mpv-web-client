use serde::Deserialize;
use std::{fs::rename, io::Read, path::Path};

use crate::{
  frontend::FrontendPkgErr,
  project_paths::{get_frontend_dir, get_frontend_temp_dir, get_project_home_dir},
};

pub const PKG_MANIFEST_NAME: &str = "pkg_manifest.toml";

#[derive(Deserialize)]
pub struct Info {
  version: String,
  commit: String,
}

#[derive(Deserialize)]
pub struct Manifest {
  info: Info,
}

pub fn parse_temp_package_manifest() -> Result<Manifest, FrontendPkgErr> {
  let frontend_temp_dir = get_frontend_temp_dir();
  let temp_manifest_file_path = {
    let mut path = frontend_temp_dir.clone();
    path.push(PKG_MANIFEST_NAME);
    path
  };
  parse_package_manifest(temp_manifest_file_path)
}

pub fn parse_project_package_manifest() -> Result<Manifest, FrontendPkgErr> {
  let frontend_dir = get_frontend_dir().map_err(FrontendPkgErr::HomeDirInaccessible)?;
  let manifest_file_path = {
    let mut path = frontend_dir.clone();
    path.push(PKG_MANIFEST_NAME);
    path
  };
  parse_package_manifest(manifest_file_path)
}

pub fn parse_package_manifest<T>(path: T) -> Result<Manifest, FrontendPkgErr>
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
    .map_err(|_| FrontendPkgErr::PkgInvalid(None))?;
  let manifest: Manifest =
    toml::from_str(toml_content.as_ref()).map_err(|_| FrontendPkgErr::PkgInvalid(None))?;

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
