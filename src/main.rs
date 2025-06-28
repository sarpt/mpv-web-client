use clap::Parser;
use std::{error::Error, path::PathBuf};

use crate::{frontend::check_frontend_pkg, project_paths::ensure_project_dirs, server::serve};

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

  ensure_project_dirs()?;
  verify_frontend(&args.pkg)
    .await
    .map_err(|err_msg| *Box::new(err_msg))?;
  serve().await
}

async fn verify_frontend(pkg_path: &Option<PathBuf>) -> Result<(), String> {
  let path_pkg = pkg_path.to_owned();
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
      Err(format!("provided pkg file is invalid: {error:?}"))
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
  }
}
