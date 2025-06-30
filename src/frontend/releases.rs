use std::path::PathBuf;

use serde::Deserialize;
use tokio::{
  fs::OpenOptions,
  io::{AsyncWriteExt, BufWriter},
};

use crate::project_paths::get_temp_dir;

#[derive(Deserialize)]
pub struct Asset {
  pub browser_download_url: String,
  pub content_type: String,
}

#[derive(Deserialize)]
pub struct Release {
  pub tag_name: String,
  pub name: String,
  pub body: String,
  pub assets: Vec<Asset>,
}

const LATEST_RELEASES_URL: &str =
  "https://api.github.com/repos/sarpt/mpv-web-front/releases/latest";
pub async fn check_latest_remote_release() -> Result<Release, String> {
  let client = reqwest::Client::new();
  let request = client
    .get(LATEST_RELEASES_URL)
    .header(
      "User-Agent",
      format!("mpv-web-client/{}", env!("CARGO_PKG_VERSION")),
    )
    .header("Accept", "application/vnd.github+json")
    .header("GitHub-Api-Version", "2022-11-28")
    .build()
    .map_err(|err| err.to_string())?;

  let response_text = client
    .execute(request)
    .await
    .map_err(|err| err.to_string())?
    .text()
    .await
    .map_err(|err| err.to_string())?;

  let response: Release = serde_json::from_str(&response_text).map_err(|err| err.to_string())?;
  Ok(response)
}

pub async fn fetch_remote_frontend_package_release(release: &Release) -> Result<PathBuf, String> {
  let client = reqwest::Client::new();
  let mut target_path = get_temp_dir();
  target_path.push(&release.name);

  let package_url = match release.assets.iter().find_map(|asset| {
    if is_asset_a_frontend_package(asset) {
      Some(&asset.browser_download_url)
    } else {
      None
    }
  }) {
    Some(url) => url,
    None => return Err("release doesn't have any frontend package assets".to_owned()),
  };

  let request = client
    .get(package_url)
    .header(
      "User-Agent",
      format!("mpv-web-client/{}", env!("CARGO_PKG_VERSION")),
    )
    .header("Accept", "application/vnd.github+json")
    .header("GitHub-Api-Version", "2022-11-28")
    .build()
    .map_err(|err| err.to_string())?;

  let tgt_file_open_result = OpenOptions::default()
    .create(true)
    .read(false)
    .write(true)
    .open(&target_path)
    .await
    .map_err(|err| err.to_string())?;

  let mut tgt_file_wrtier = BufWriter::new(tgt_file_open_result);

  let mut response = client
    .execute(request)
    .await
    .map_err(|err| err.to_string())?;

  while let Some(chunk) = response.chunk().await.map_err(|err| err.to_string())? {
    let _ = tgt_file_wrtier
      .write(&chunk)
      .await
      .map_err(|err| err.to_string())?;
  }

  let _ = tgt_file_wrtier
    .shutdown()
    .await
    .map_err(|err| err.to_string());

  Ok(target_path)
}

fn is_asset_a_frontend_package(asset: &Asset) -> bool {
  asset.content_type == "application/gzip"
}
