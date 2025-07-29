use std::{
  fs::create_dir_all,
  path::{Path, PathBuf},
};

use log::{info, warn};
use tokio::fs::{remove_dir_all, rename};

use crate::{
  frontend::{
    FrontendPkgErr,
    pkg::{
      extraction::extract_frontend_pkg,
      manifest::{Manifest, PKG_MANIFEST_NAME, parse_package_manifest},
    },
  },
  project_paths::{get_frontend_dir, get_frontend_temp_dir, get_project_home_dir, get_temp_dir},
};

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

  pub fn get_installed(&self) -> Result<&Package, FrontendPkgErr> {
    match self.installed {
      Some(ref pkg) => Ok(pkg),
      None => Err(FrontendPkgErr::ManifestInvalid(
        "could not retrieve installed manifest".to_owned(),
      )), // TODO: this error type does not make sense
    }
  }

  pub fn get_temp(&self) -> Result<&Package, FrontendPkgErr> {
    match self.temp {
      Some(ref pkg) => Ok(pkg),
      None => Err(FrontendPkgErr::ManifestInvalid(
        "could not retrieve temp manifest".to_owned(),
      )), // TODO: this error type does not make sense
    }
  }

  async fn check_installed(&mut self) -> Result<(), FrontendPkgErr> {
    let mut path = get_project_home_dir().map_err(FrontendPkgErr::HomeDirInaccessible)?;
    path.push(PKG_MANIFEST_NAME);
    match parse_package_manifest(path).await {
      Ok(m) => {
        let package = Package { manifest: m };
        self.installed = Some(package);
        Ok(())
      }
      Err(err) => Err(err),
    }
  }

  async fn check_temp(&mut self) -> Result<(), FrontendPkgErr> {
    let mut path = get_frontend_temp_dir();
    path.push(PKG_MANIFEST_NAME);
    match parse_package_manifest(path).await {
      Ok(m) => {
        let package = Package { manifest: m };
        self.installed = Some(package);
        Ok(())
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
    self.check_temp().await?;

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

    tokio::task::spawn_blocking(copy_frontend_pkg_to_home)
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

    move_manifest_to_project_home().await?;
    self.check_installed().await?;

    Ok(())
  }

  pub async fn get_installed_file<T>(
    &self,
    name: T,
  ) -> Result<(tokio::fs::File, PathBuf), std::io::Error>
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

fn copy_frontend_pkg_to_home() -> Result<(), FrontendPkgErr> {
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

async fn move_manifest_to_project_home() -> Result<(), FrontendPkgErr> {
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
