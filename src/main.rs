use clap::Parser;
use log::{debug, error, info};
use std::{error::Error, path::PathBuf, time::SystemTime};

use crate::{
  frontend::{
    RemoteReleaseCheckResult, check_for_newer_remote_release, check_frontend_pkg, install_package,
    releases::{Release, fetch_remote_frontend_package_release},
  },
  project_paths::ensure_project_dirs,
  server::serve,
};

mod common;
mod frontend;
mod project_paths;
mod server;

#[derive(Parser, Debug)]
#[command(version = "0.1.0", about = "client for mpv-web-api and mpv-web-front server", long_about = None)]
struct Args {
  #[arg(
    long,
    required = false,
    help = "Path to a .tar.gz frontend package. Overrides --update."
  )]
  pkg: Option<PathBuf>,

  #[arg(
    action,
    short = 'u',
    long,
    required = false,
    help = "Update to newer frontend package, if any exists on remote repository. Does not apply when --pkg provided."
  )]
  update: bool,

  #[arg(
    action,
    short = 'f',
    long,
    required = false,
    help = "Force installation of provided outdated frontend package with --pkg, even if the newer package is already being served."
  )]
  force_outdated: bool,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn Error>> {
  let args = Args::parse();

  init_logging()?;
  debug!("version {}", env!("CARGO_PKG_VERSION"));

  ensure_project_dirs()?;
  init_frontend(&args)
    .await
    .map_err(|err_msg| *Box::new(err_msg))?;
  serve().await
}

async fn init_frontend(args: &Args) -> Result<(), String> {
  let mut pkg_path = args.pkg.to_owned();
  if pkg_path.is_none() {
    if let Some(new_release) = remote_frontend_release_available(args.update).await {
      info!(
        "fetching new frontend package version \"{}\"",
        new_release.name
      );
      pkg_path = fetch_new_frontend_release(&new_release).await;
    }
  }

  if let Some(ref path) = pkg_path {
    install_package(path.to_owned(), args.force_outdated)
      .await
      .map_err(|err| format!("frontend package install failed: {err}"))?;
  }

  match check_frontend_pkg(pkg_path) {
    Ok(_) => Ok(()),
    Err(err) => Err(format!("frontend init failed: {err}")),
  }
}

async fn remote_frontend_release_available(allow_updates: bool) -> Option<Release> {
  match check_for_newer_remote_release().await {
    Ok(result) => match result {
      RemoteReleaseCheckResult::UpToDate(local) => {
        info!("local frontend version \"{local}\" is up to date");
        None
      }
      RemoteReleaseCheckResult::NewerRemoteAvailable(new_release) => {
        if allow_updates {
          Some(new_release)
        } else {
          info!(
            "newer frontend release \"{}\" is available - run the program with \"--update\" argument to install it",
            new_release.name
          );
          None
        }
      }
      RemoteReleaseCheckResult::RemoteNecessary(release) => Some(release),
    },
    Err(err) => {
      error!("check for the latest remote package failed: {err}");
      None
    }
  }
}

async fn fetch_new_frontend_release(new_release: &Release) -> Option<PathBuf> {
  match fetch_remote_frontend_package_release(new_release).await {
    Ok(path_pkg) => Some(path_pkg),
    Err(err) => {
      error!("fetch of remote frontend package failed: {err}");
      None
    }
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
