use std::{fmt::Display, path::PathBuf};

use reqwest::{Client, IntoUrl, Request};
use serde::{Deserialize, Serialize};
use tokio::{
  fs::OpenOptions,
  io::{AsyncWriteExt, BufWriter},
};

use crate::{frontend::pkg::manifest::semver::Semver, project_paths::get_temp_dir};

#[derive(Deserialize)]
struct Asset {
  pub browser_download_url: String,
  pub content_type: String,
  pub size: usize,
}

#[derive(Deserialize)]
struct RemoteRelease {
  pub tag_name: String,
  pub name: String,
  pub body: String,
  pub assets: Vec<Asset>,
}

impl RemoteRelease {
  fn is_asset_a_frontend_package(asset: &Asset) -> bool {
    asset.content_type == "application/gzip"
  }
}

impl TryFrom<RemoteRelease> for Release {
  type Error = String;

  fn try_from(val: RemoteRelease) -> Result<Release, std::string::String> {
    let download = val
      .assets
      .iter()
      .find(|asset| RemoteRelease::is_asset_a_frontend_package(asset))
      .map(|asset| ReleaseDownloadInfo {
        url: asset.browser_download_url.to_owned(),
        size: asset.size,
      });

    Ok(Release {
      name: val.name,
      description: val.body,
      version: val
        .tag_name
        .try_into()
        .map_err(|err| format!("can't parse tag_name as a version: {err}"))?,
      download,
    })
  }
}

#[derive(Deserialize, Serialize)]
pub struct ReleaseDownloadInfo {
  pub url: String,
  pub size: usize,
}

#[derive(Deserialize, Serialize)]
pub struct Release {
  pub name: String,
  pub version: Semver,
  pub description: String,
  pub download: Option<ReleaseDownloadInfo>,
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

  let response: RemoteRelease = serde_json::from_str(&response_text).map_err(|err| {
    ReleaseFetchErr::ResponseParseFailure(format!("response has invalid JSON: {err}"))
  })?;
  let release = response
    .try_into()
    .map_err(ReleaseFetchErr::ResponseParseFailure)?;
  Ok(release)
}

pub async fn fetch_remote_frontend_package_release(
  release: &Release,
) -> Result<PathBuf, ReleaseFetchErr> {
  let download = match &release.download {
    Some(download) => download,
    None => {
      return Err(ReleaseFetchErr::NoPkgAssets);
    }
  };

  let client = Client::new();
  let request = get_request(&client, &download.url)?;
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

  if total_written != download.size {
    return Err(ReleaseFetchErr::SizeMismatch(total_written, download.size));
  }

  Ok(target_path)
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
