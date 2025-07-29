use serde::Deserialize;
use std::path::Path;
use tokio::{fs::OpenOptions, io::AsyncReadExt};

use crate::{common::semver::Semver, frontend::FrontendPkgErr};

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

pub async fn parse_package_manifest<T>(path: T) -> Result<Manifest, FrontendPkgErr>
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
