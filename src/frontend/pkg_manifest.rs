use serde::Deserialize;
use std::{cmp::Ordering, fs::rename, io::Read, path::Path};

use crate::{
  frontend::FrontendPkgErr,
  project_paths::{get_frontend_dir, get_frontend_temp_dir, get_project_home_dir},
};

pub const PKG_MANIFEST_NAME: &str = "pkg_manifest.toml";

#[derive(Deserialize, PartialEq)]
pub struct VersionInfo {
  pub version: String,
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

const VERSION_SEMVER_SEPARATOR: &str = ".";
// TODO: can't be PartialOrd/Ord since parsing of Semver may fail and None will not handle that correctly.
// Create a Semver type that can be deserialized by serde and evaluated at parsing time?
pub fn compare_package_manifests(
  a_pkg: &Manifest,
  b_pkg: &Manifest,
) -> Result<Ordering, FrontendPkgErr> {
  if a_pkg.version_info == b_pkg.version_info {
    return Ok(Ordering::Equal);
  };

  let split_a_semver = a_pkg
    .version_info
    .version
    .split(VERSION_SEMVER_SEPARATOR)
    .map(|chunk| {
      chunk.parse::<usize>().map_err(|_| {
        FrontendPkgErr::ManifestInvalid("could not parse version as semver".to_owned())
      })
    });
  let mut split_b_semver = b_pkg
    .version_info
    .version
    .split(VERSION_SEMVER_SEPARATOR)
    .map(|chunk| {
      chunk.parse::<usize>().map_err(|_| {
        FrontendPkgErr::ManifestInvalid("could not parse version as semver".to_owned())
      })
    });
  for (idx, a_semver_chunk_result) in split_a_semver.enumerate() {
    let a_semver_chunk = a_semver_chunk_result?;
    match split_b_semver.nth(idx) {
      Some(b_semver_chunk_result) => {
        let b_semver_chunk = b_semver_chunk_result?;
        if a_semver_chunk == b_semver_chunk {
          continue;
        }

        if a_semver_chunk > b_semver_chunk {
          return Ok(Ordering::Greater);
        } else {
          return Ok(Ordering::Less);
        };
      }
      None => {
        return Err(FrontendPkgErr::ManifestInvalid(
          "could not compare semvers since they don't have equal format".to_owned(),
        ));
      }
    }
  }

  Ok(Ordering::Equal)
}
