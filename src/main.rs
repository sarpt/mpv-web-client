use clap::Parser;
use log::{error, info, warn};
use std::{error::Error, path::PathBuf, sync::Arc, time::SystemTime};
use tokio::sync::Mutex;

use crate::{
  frontend::{init_frontend, pkg::repository::PackagesRepository},
  project_paths::ensure_project_dirs,
  server::serve,
};

mod common;
mod frontend;
mod project_paths;
mod server;

const DEFAULT_IDLE_SHUTDOWN_TIMEOUT: u8 = 60;
const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Parser, Debug)]
#[command(version = VERSION, about = "client for mpv-web-api and mpv-web-front server", long_about = None)]
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

  #[arg(
    action,
    default_value_t = DEFAULT_IDLE_SHUTDOWN_TIMEOUT.into(),
    long,
    required = false,
    help = "Time in seconds after which server will shutdown when idle. Any incoming request to server will reset this interval. Does not apply when --enable-idle-shutdown-timeout is not set."
  )]
  idle_shutdown_timeout: u32,

  #[arg(
    action,
    long,
    required = false,
    help = "Enables server idle timeout mechanism which shuts server down when the server does not receive any requests in specified timeout interval."
  )]
  enable_idle_shutdown_timeout: bool,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn Error>> {
  let args = Args::parse();

  init_logging()?;
  info!("version {VERSION}");

  ensure_project_dirs()?;
  let mut packages_repository = PackagesRepository::new();
  init_frontend(
    args.pkg,
    args.update,
    args.force_outdated,
    &mut packages_repository,
  )
  .await
  .map_err(|err_msg| *Box::new(err_msg))?;
  let idle_shutdown_interval = if args.enable_idle_shutdown_timeout {
    warn!(
      "server will shut down after being idle for {} seconds!",
      args.idle_shutdown_timeout
    );
    Some(args.idle_shutdown_timeout)
  } else {
    None
  };

  let server_dependencies = server::Dependencies {
    packages_repository: Arc::new(Mutex::new(packages_repository)),
  };
  if let Err(err) = serve(idle_shutdown_interval, server_dependencies).await {
    error!("error encountered while serving: {err}");
    return Err(err);
  }

  Ok(())
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
