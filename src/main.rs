use clap::Parser;
use log::{debug, error, info};
use std::{error::Error, path::PathBuf, time::SystemTime};

use crate::{
  frontend::{
    check_frontend_pkg,
    releases::{check_latest_remote_release, fetch_remote_frontend_package_release},
  },
  project_paths::ensure_project_dirs,
  server::serve,
};

mod frontend;
mod project_paths;
mod server;

#[derive(Parser, Debug)]
#[command(version = "0.1.0", about = "client for mpv-web-api and mpv-web-front server", long_about = None)]
struct Args {
  #[arg(
    short,
    long,
    required = false,
    help = "Path to a .tar.gz frontend package"
  )]
  pkg: Option<PathBuf>,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn Error>> {
  let args = Args::parse();

  init_logging()?;
  debug!("version {}", env!("CARGO_PKG_VERSION"));

  ensure_project_dirs()?;
  init_frontend(&args.pkg)
    .await
    .map_err(|err_msg| *Box::new(err_msg))?;
  serve().await
}

async fn init_frontend(pkg_path: &Option<PathBuf>) -> Result<(), String> {
  let mut path_pkg = pkg_path.to_owned();

  if path_pkg.is_none() {
    match check_latest_remote_release().await {
      Ok(release) => {
        info!("the latest version is \"{}\"", release.tag_name);
        path_pkg = match fetch_remote_frontend_package_release(&release).await {
          Ok(path_pkg) => Some(path_pkg),
          Err(err) => {
            error!("fetch of remote frontend package failed: {err}");
            None
          }
        }
      }
      Err(err) => {
        error!("check for the latest version failed: {err}");
      }
    };
  }

  let result = tokio::task::spawn_blocking(|| check_frontend_pkg(path_pkg)).await;
  let check_frontend_result = match result {
    Ok(res) => res,
    Err(e) => return Err(format!("issue with joining on blocking task {e}")),
  };

  let frontend_check_err = match check_frontend_result {
    Ok(_) => return Ok(()),
    Err(e) => e,
  };

  match frontend_check_err {
    frontend::FrontendPkgErr::PkgInvalid(error) => {
      Err(format!("provided pkg file is invalid: {error}"))
    }
    frontend::FrontendPkgErr::IndexNotFound(error) => Err(format!(
      "frontend cannot be served due to lack of entrypoint file: {error:?}"
    )),
    frontend::FrontendPkgErr::HomeDirInaccessible(error) => Err(format!(
      "the program could not read it's home directory: {error}"
    )),
    frontend::FrontendPkgErr::PkgNotProvided => Err(
      "frontend package has not been provided and there is no cached frontend package".to_owned(),
    ),
    frontend::FrontendPkgErr::PkgUnpackErr(error) => {
      Err(format!("frontend package could not be unpacked: {error}"))
    }
    frontend::FrontendPkgErr::PkgOutdated(tmp_version, home_version) => Err(format!(
      "provided frontend package has outdated version \"{tmp_version}\" compared to currently installed version \"{home_version}\""
    )),
    frontend::FrontendPkgErr::ManifestInvalid(msg) => Err(format!(
      "frontend package manifest is in incorrect format: {msg}"
    )),
  }
}

fn init_logging() -> Result<(), fern::InitError> {
  fern::Dispatch::new()
    .format(|out, message, record| {
      out.finish(format_args!(
        "{} {} {} # {}",
        humantime::format_rfc3339_seconds(SystemTime::now()),
        record.level(),
        record.target(),
        message
      ))
    })
    .level(log::LevelFilter::Debug)
    .chain(std::io::stdout())
    .apply()?;
  Ok(())
}
