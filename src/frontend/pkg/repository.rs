use crate::frontend::{
  FrontendPkgErr,
  pkg::manifest::{Manifest, parse_project_package_manifest, parse_temp_package_manifest},
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
}
