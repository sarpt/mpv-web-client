use std::{fs::create_dir_all, path::Path};

use log::{info, warn};

use crate::{
  frontend::{
    FrontendPkgErr,
    pkg::{
      extraction::extract_frontend_pkg,
      manifest::{
        Manifest, move_manifest_to_project_home, parse_project_package_manifest,
        parse_temp_package_manifest,
      },
    },
  },
  project_paths::{get_frontend_temp_dir, get_project_home_dir, get_temp_dir},
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

  pub async fn get_installed(&mut self) -> Result<&Package, FrontendPkgErr> {
    match self.installed {
      Some(ref pkg) => Ok(pkg),
      None => match parse_project_package_manifest().await {
        Ok(m) => {
          let package = Package { manifest: m };
          self.installed = Some(package);
          Ok(self.installed.as_ref().unwrap())
        }
        Err(err) => Err(err),
      },
    }
  }

  pub async fn get_temp(&mut self) -> Result<&Package, FrontendPkgErr> {
    match self.temp {
      Some(ref pkg) => Ok(pkg),
      None => match parse_temp_package_manifest().await {
        Ok(m) => {
          let package = Package { manifest: m };
          self.installed = Some(package);
          Ok(self.installed.as_ref().unwrap())
        }
        Err(err) => Err(err),
      },
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

    self.temp = None;

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

    tokio::task::spawn_blocking(move_frontend_pkg_to_home)
      .await
      .map_err(|e| {
        FrontendPkgErr::PkgInstallFailed(format!(
          "issue with joining on blocking task for frontend move: {e}"
        ))
      })??;
    move_manifest_to_project_home().await?;

    self.installed = None;

    Ok(())
  }

  async fn check_temp_pkg_manifest_against_installed_one(&mut self) -> Result<(), FrontendPkgErr> {
    let temp_version = self.get_temp().await?.manifest.version_info.version;
    let local_version = match self.get_installed().await {
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
