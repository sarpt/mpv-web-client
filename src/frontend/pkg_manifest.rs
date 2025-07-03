use serde::{Deserialize, Deserializer};
use std::{fs::rename, io::Read, path::Path};

use crate::{
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

#[derive(PartialEq, PartialOrd)]
pub struct Semver {
  major: usize,
  minor: usize,
  patch: usize,
}

const VERSION_SEMVER_SEPARATOR: &str = ".";
impl<'de> Deserialize<'de> for Semver {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: Deserializer<'de>,
  {
    let version = String::deserialize(deserializer)?;
    Semver::from_string(&version).map_err(serde::de::Error::custom)
  }
}

impl Semver {
  fn from_string(source: &String) -> Result<Self, String> {
    let mut split_version = source.split(VERSION_SEMVER_SEPARATOR).map(|chunk| {
      chunk
        .parse::<usize>()
        .map_err(|err| format!("could not parse source string of \"{source}\" as semver: {err}"))
    });
    let major: usize = split_version.nth(0).unwrap_or(Ok(0))?;
    let minor: usize = split_version.nth(1).unwrap_or(Ok(0))?;
    let patch: usize = split_version.nth(2).unwrap_or(Ok(0))?;
    Ok(Semver {
      major,
      minor,
      patch,
    })
  }
}

impl TryFrom<&String> for Semver {
  type Error = String;

  fn try_from(value: &String) -> Result<Self, Self::Error> {
    Semver::from_string(value)
  }
}

impl From<Semver> for String {
  fn from(val: Semver) -> Self {
    [val.major, val.minor, val.patch]
      .map(|chunk| chunk.to_string())
      .join(VERSION_SEMVER_SEPARATOR)
  }
}
