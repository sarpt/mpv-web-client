use clap::Parser;
use std::{error::Error, path::PathBuf};

use crate::{frontend::check_frontend_pkg, server::serve};

mod frontend;
mod home_dir;
mod server;

#[derive(Parser, Debug)]
#[command(version = "0.1.0", about = "client for mpv-web-api and mpv-web-front server", long_about = None)]
struct Args {
  #[arg(short, long)]
  pkg: Option<PathBuf>,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn Error>> {
  let args = Args::parse();

  verify_frontend(&args.pkg).await?;
  serve().await
}

async fn verify_frontend(pkg_path: &Option<PathBuf>) -> Result<(), Box<dyn Error>> {
  let path_pkg = pkg_path.to_owned();
  let result = tokio::task::spawn_blocking(|| check_frontend_pkg(path_pkg)).await;
  match result {
    Ok(res) => {
      if let Err(err) = res {
        match err {
          frontend::FrontendCheckErr::PkgInvalid(error) => {
            return Err((*Box::new(format!("provided pkg file is invalid: {:?}", error))).into());
          }
          frontend::FrontendCheckErr::IndexNotFound(error) => {
            return Err(
              (*Box::new(format!(
                "frontend cannot be served due to lack of entrypoint file: {:?}",
                error
              )))
              .into(),
            );
          }
          frontend::FrontendCheckErr::HomeDirInaccessible(error) => {
            return Err(
              (*Box::new(format!(
                "the program could not read it's home directory: {}",
                error
              )))
              .into(),
            );
          }
          frontend::FrontendCheckErr::PkgNotProvided => {
            return Err(
              (*Box::new(
                "frontend package has not been provided and there is no cached frontend package",
              ))
              .into(),
            );
          }
        }
      }
    }
    Err(e) => return Err(Box::new(e).into()),
  }

  Ok(())
}
