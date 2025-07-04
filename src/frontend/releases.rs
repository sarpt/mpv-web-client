use std::{fmt::Display, path::PathBuf};

use reqwest::{Client, IntoUrl, Request};
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
  pub size: usize,
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
pub async fn check_latest_remote_release() -> Result<Release, ReleaseFetchErr> {
  let client = Client::new();
  let request = get_request(&client, LATEST_RELEASES_URL)?;

  let response_text = client
    .execute(request)
    .await
    .map_err(ReleaseFetchErr::RemoteFetchFailed)?
    .text()
    .await
    .map_err(|err| {
      ReleaseFetchErr::ResponseParseFailure(format!("could not retrieve text response : {err}"))
    })?;

  let response: Release = serde_json::from_str(&response_text).map_err(|err| {
    ReleaseFetchErr::ResponseParseFailure(format!("response has invalid JSON: {err}"))
  })?;
  Ok(response)
}

pub async fn fetch_remote_frontend_package_release(
  release: &Release,
) -> Result<PathBuf, ReleaseFetchErr> {
  let asset = match release
    .assets
    .iter()
    .find(|asset| is_asset_a_frontend_package(asset))
  {
    Some(url) => url,
    None => {
      return Err(ReleaseFetchErr::NoPkgAssets);
    }
  };

  let client = Client::new();
  let request = get_request(&client, &asset.browser_download_url)?;
  let mut response = client
    .execute(request)
    .await
    .map_err(ReleaseFetchErr::RemoteFetchFailed)?;

  let mut target_path = get_temp_dir();
  target_path.push(&release.name);

  let tgt_file_open_result = OpenOptions::default()
    .create(true)
    .read(false)
    .write(true)
    .open(&target_path)
    .await
    .map_err(ReleaseFetchErr::WriteToDiskFailed)?;

  let mut tgt_file_wrtier = BufWriter::new(tgt_file_open_result);

  let mut total_written: usize = 0;
  while let Some(chunk) = response
    .chunk()
    .await
    .map_err(ReleaseFetchErr::RemoteFetchFailed)?
  {
    let written = tgt_file_wrtier
      .write(&chunk)
      .await
      .map_err(ReleaseFetchErr::WriteToDiskFailed)?;
    total_written += written;
  }

  tgt_file_wrtier
    .shutdown()
    .await
    .map_err(ReleaseFetchErr::WriteToDiskFailed)?;

  if total_written != asset.size {
    return Err(ReleaseFetchErr::SizeMismatch(total_written, asset.size));
  }

  Ok(target_path)
}

fn is_asset_a_frontend_package(asset: &Asset) -> bool {
  asset.content_type == "application/gzip"
}

fn get_request<T>(client: &Client, url: T) -> Result<Request, ReleaseFetchErr>
where
  T: IntoUrl + Copy + Display,
{
  client
    .get(url)
    .header(
      "User-Agent",
      format!("mpv-web-client/{}", env!("CARGO_PKG_VERSION")),
    )
    .header("Accept", "application/vnd.github+json")
    .header("GitHub-Api-Version", "2022-11-28")
    .build()
    .map_err(ReleaseFetchErr::RemoteFetchFailed)
}

pub enum ReleaseFetchErr {
  NoPkgAssets,
  SizeMismatch(usize, usize),
  WriteToDiskFailed(std::io::Error),
  RemoteFetchFailed(reqwest::Error),
  ResponseParseFailure(String),
}

impl Display for ReleaseFetchErr {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      ReleaseFetchErr::NoPkgAssets => write!(f, "release doesn't have any frontend package assets"),
      ReleaseFetchErr::WriteToDiskFailed(err) => write!(f, "could not write file to disk: {err}"),
      ReleaseFetchErr::RemoteFetchFailed(err) => write!(f, "could not fetch package file: {err}"),
      ReleaseFetchErr::ResponseParseFailure(msg) => write!(f, "{msg}"),
      ReleaseFetchErr::SizeMismatch(written, declared) => write!(
        f,
        "expected package size of {declared} bytes but only {written} bytes written"
      ),
    }
  }
}
