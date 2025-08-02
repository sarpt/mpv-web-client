use std::{
  fs::create_dir_all,
  path::{Path, PathBuf},
};

use log::{debug, info, warn};
use tokio::fs::{remove_dir_all, rename};

use crate::{
  common::semver::Semver,
  frontend::{
    FrontendPkgErr,
    pkg::{
      extraction::extract_frontend_pkg,
      manifest::{Manifest, PKG_MANIFEST_NAME, parse_package_manifest},
    },
  },
  project_paths::{get_frontend_dir, get_frontend_temp_dir, get_project_home_dir},
};

#[derive(Clone)]
pub struct Package {
  pub manifest: Manifest,
}

pub struct PackagesRepository {
  installed: Option<Package>,
  temp: Option<Package>,
}

impl PackagesRepository {
  pub fn new() -> Self {
    PackagesRepository {
      installed: None,
      temp: None,
    }
  }

  pub async fn init(&mut self) {
    if let Err(err) = self.check_installed().await {
      debug!("initial installed package check unsuccessful: {err}");
    };
  }

  pub fn get_installed(&self) -> Result<&Package, FrontendPkgErr> {
    match self.installed {
      Some(ref pkg) => Ok(pkg),
      None => Err(FrontendPkgErr::PackageUnavailable(
        "there is no package installed".to_owned(),
      )),
    }
  }

  pub fn get_temp(&self) -> Result<&Package, FrontendPkgErr> {
    match self.temp {
      Some(ref pkg) => Ok(pkg),
      None => Err(FrontendPkgErr::PackageUnavailable(
        "there is no temporary package".to_owned(),
      )),
    }
  }

  async fn check_installed(&mut self) -> Result<Package, FrontendPkgErr> {
    let mut path = get_project_home_dir().map_err(FrontendPkgErr::HomeDirInaccessible)?;
    path.push(PKG_MANIFEST_NAME);
    match parse_package_manifest(path).await {
      Ok(m) => {
        let package = Package { manifest: m };
        self.installed = Some(package.clone());
        Ok(package)
      }
      Err(err) => Err(err),
    }
  }

  async fn check_temp(&mut self) -> Result<Package, FrontendPkgErr> {
    let mut path = get_frontend_temp_dir();
    path.push(PKG_MANIFEST_NAME);
    match parse_package_manifest(path).await {
      Ok(m) => {
        let package = Package { manifest: m };
        self.temp = Some(package.clone());
        Ok(package)
      }
      Err(err) => Err(err),
    }
  }

  pub async fn install_package<T>(
    &mut self,
    pkg_path: T,
    force_outdated: bool,
  ) -> Result<(), FrontendPkgErr>
  where
    T: AsRef<Path> + Send + Sync + 'static,
  {
    tokio::task::spawn_blocking(|| extract_frontend_pkg(pkg_path))
      .await
      .map_err(|e| {
        FrontendPkgErr::PkgInstallFailed(format!(
          "issue with joining on blocking task for frontend extraction: {e}"
        ))
      })??;
    let temp_version = self.check_temp().await?.manifest.version_info.version;

    match self.check_temp_pkg_manifest_against_installed_one().await {
      Ok(()) => {}
      Err(err) => {
        match &err {
          FrontendPkgErr::PkgOutdated(provided_version, served_version) => {
            if force_outdated {
              info!("forcing outdated version \"{provided_version}\" over \"{served_version}\"");
            } else {
              return Err(err);
            }
          }
          _ => {
            return Err(err);
          }
        };
      }
    };

    tokio::task::spawn_blocking(move || copy_frontend_pkg_to_home(&temp_version))
      .await
      .map_err(|e| {
        FrontendPkgErr::PkgInstallFailed(format!(
          "issue with joining on blocking task for frontend move: {e}"
        ))
      })??;

    let frontend_temp_dir = get_frontend_temp_dir();
    if let Err(e) = remove_dir_all(&frontend_temp_dir).await {
      warn!(
        "could not remove the temporary frontend directory at path {}: reason: {e}",
        frontend_temp_dir.to_string_lossy()
      );
    };
    self.temp = None;

    move_manifest_to_project_home(&temp_version).await?;
    self.check_installed().await?;

    Ok(())
  }

  pub async fn get_installed_file<T>(
    &self,
    name: T,
  ) -> Result<(tokio::fs::File, PathBuf), FrontendPkgErr>
  where
    T: AsRef<Path>,
  {
    let mut src_path = get_frontend_dir().map_err(FrontendPkgErr::HomeDirInaccessible)?;
    let version = self
      .get_installed()?
      .manifest
      .version_info
      .version
      .to_string();
    src_path.push(version);
    src_path.push(name);

    let src_file_open_result = tokio::fs::OpenOptions::default()
      .create(false)
      .read(true)
      .write(false)
      .open(&src_path)
      .await;

    match src_file_open_result {
      Ok(src_file) => Ok((src_file, src_path)),
      Err(err) => Err(FrontendPkgErr::HomeDirInaccessible(err)),
    }
  }

  async fn check_temp_pkg_manifest_against_installed_one(&mut self) -> Result<(), FrontendPkgErr> {
    let temp_version = self.get_temp()?.manifest.version_info.version;
    let local_version = match self.get_installed() {
      Ok(pkg) => pkg.manifest.version_info.version,
      Err(err) => {
        warn!("could not parse existing frontend package manifest: {err}");
        return Ok(());
      }
    };

    if temp_version < local_version {
      return Err(FrontendPkgErr::PkgOutdated(
        temp_version.into(),
        local_version.into(),
      ));
    }
    Ok(())
  }
}

fn copy_frontend_pkg_to_home(version: &Semver) -> Result<(), FrontendPkgErr> {
  let frontend_temp_dir = get_frontend_temp_dir();
  let mut install_frontend_dir = get_frontend_dir().map_err(FrontendPkgErr::HomeDirInaccessible)?;
  install_frontend_dir.push(version.to_string());

  for entry_result in walkdir::WalkDir::new(&frontend_temp_dir) {
    let entry = match entry_result {
      Ok(e) => e,
      Err(err) => return Err(FrontendPkgErr::PkgUnpackErr(err.into())),
    };

    let mut tgt_path = install_frontend_dir.clone();
    let stripped_path = entry.path().strip_prefix(&frontend_temp_dir).unwrap();
    tgt_path.push(stripped_path);
    if entry.file_type().is_dir() {
      create_dir_all(tgt_path).map_err(FrontendPkgErr::PkgUnpackErr)?;
    } else if entry.file_type().is_file() {
      std::fs::copy(entry.path(), tgt_path).map_err(FrontendPkgErr::HomeDirInaccessible)?;
    }
  }

  Ok(())
}

async fn move_manifest_to_project_home(version: &Semver) -> Result<(), FrontendPkgErr> {
  let mut frontend_dir = get_frontend_dir().map_err(FrontendPkgErr::HomeDirInaccessible)?; // this should also use version
  frontend_dir.push(version.to_string());
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
