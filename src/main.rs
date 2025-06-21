use std::error::Error;

use crate::{pkgs::extract_frontend_pkg, server::serve};

mod home_dir;
mod pkgs;
mod server;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn Error>> {
  let result = tokio::task::spawn_blocking(|| extract_frontend_pkg("010.tar.gz")).await;
  match result {
    Ok(res) => {
      if let Err(e) = res {
        return Err(Box::new(e).into());
      }
    }
    Err(e) => return Err(Box::new(e).into()),
  }

  serve().await
}
