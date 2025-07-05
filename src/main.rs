use clap::Parser;
use log::{debug, error};
use std::{error::Error, path::PathBuf, time::SystemTime};

use crate::{
  frontend::{
    check_frontend_pkg, newer_remote_release_available,
    releases::fetch_remote_frontend_package_release,
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
    match newer_remote_release_available().await {
      Ok(release) => {
        path_pkg = match fetch_remote_frontend_package_release(&release).await {
          Ok(path_pkg) => Some(path_pkg),
          Err(err) => {
            error!("fetch of remote frontend package failed: {err}");
            None
          }
        }
      }
      Err(err) => {
        error!("check for the latest remote package failed: {err}");
      }
    };
  }

  let result = tokio::task::spawn_blocking(|| check_frontend_pkg(path_pkg)).await;
  let check_frontend_result = match result {
    Ok(res) => res,
    Err(e) => return Err(format!("issue with joining on blocking task {e}")),
  };

  match check_frontend_result {
    Ok(_) => Ok(()),
    Err(err) => Err(format!("frontend init failed: {err}")),
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
