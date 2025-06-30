use flate2::bufread::GzDecoder;
use log::warn;
use serde::Deserialize;
use std::{
  cmp::Ordering,
  fs::{create_dir_all, exists, remove_file},
  io::{BufReader, BufWriter, Seek, copy},
  path::{Path, PathBuf},
};
use tar::Archive;
use tokio::io::AsyncWriteExt;

use crate::{
  frontend::pkg_manifest::{
    PKG_MANIFEST_NAME, compare_package_manifests, move_manifest_to_project_home,
    parse_project_package_manifest, parse_temp_package_manifest,
  },
  project_paths::{get_frontend_dir, get_frontend_temp_dir, get_project_home_dir, get_temp_dir},
};

mod pkg_manifest;

pub const INDEX_FILE_NAME: &str = "index.html";

#[derive(Debug)]
pub enum FrontendPkgErr {
  IndexNotFound(Option<std::io::Error>),
  PkgNotProvided,
  PkgUnpackErr(std::io::Error),
  PkgInvalid(String),
  PkgOutdated(String, String),
  ManifestInvalid(String),
  HomeDirInaccessible(std::io::Error),
}

pub fn check_frontend_pkg<T>(pkg_path: Option<T>) -> Result<(), FrontendPkgErr>
where
  T: AsRef<Path>,
{
  if let Some(path) = &pkg_path {
    extract_frontend_pkg(path)?;
    check_new_pkg_manifest_against_existing_one()?;
    move_frontend_pkg_to_home()?;
    move_manifest_to_project_home()?;
  }

  {
    let mut path = get_frontend_dir().map_err(FrontendPkgErr::HomeDirInaccessible)?;
    path.push(INDEX_FILE_NAME);
    let index_exists = exists(path).map_err(|err| FrontendPkgErr::IndexNotFound(Some(err)))?;
    if !index_exists {
      if pkg_path.is_none() {
        return Err(FrontendPkgErr::PkgNotProvided);
      } else {
        return Err(FrontendPkgErr::IndexNotFound(None));
      }
    }
  };

  {
    let mut path = get_project_home_dir().map_err(FrontendPkgErr::HomeDirInaccessible)?;
    path.push(PKG_MANIFEST_NAME);
    let manifest_exists =
      exists(path).map_err(|err| FrontendPkgErr::PkgInvalid(err.to_string()))?;
    if !manifest_exists {
      if pkg_path.is_none() {
        return Err(FrontendPkgErr::PkgNotProvided);
      } else {
        return Err(FrontendPkgErr::PkgInvalid(
          "manifest file does not exist in project home directory".to_owned(),
        ));
      }
    }
  };

  Ok(())
}

const STREAM_CHUNK_SIZE: usize = 1024 * 1024 * 64;
const TEMP_INFLATED_PKG_NAME: &str = "inflated.tar";
pub fn extract_frontend_pkg<T>(src_path: T) -> Result<(), FrontendPkgErr>
where
  T: AsRef<Path>,
{
  let src_file_open_handle = std::fs::OpenOptions::new()
    .create(false)
    .read(true)
    .write(false)
    .open(&src_path)
    .map_err(|err| FrontendPkgErr::PkgInvalid(err.to_string()))?;

  let temp_inflated_path = {
    let mut temp_path = get_temp_dir();
    temp_path.push(TEMP_INFLATED_PKG_NAME);
    temp_path
  };

  let mut temp_inflated_file_open_handle = std::fs::OpenOptions::new()
    .create(true)
    .truncate(true)
    .read(true)
    .write(true)
    .open(&temp_inflated_path)
    .map_err(FrontendPkgErr::PkgUnpackErr)?;

  let src_pkg_reader = BufReader::with_capacity(STREAM_CHUNK_SIZE, src_file_open_handle);
  let mut decoder = GzDecoder::new(src_pkg_reader);
  let mut inflated_writer =
    BufWriter::with_capacity(STREAM_CHUNK_SIZE, &temp_inflated_file_open_handle);
  copy(&mut decoder, &mut inflated_writer)
    .map_err(|err| FrontendPkgErr::PkgInvalid(err.to_string()))?;
  drop(inflated_writer);

  temp_inflated_file_open_handle
    .seek(std::io::SeekFrom::Start(0))
    .map_err(FrontendPkgErr::HomeDirInaccessible)?;

  let unpack_temp_dir = get_frontend_temp_dir();
  let mut tar_archive = Archive::new(temp_inflated_file_open_handle);
  tar_archive
    .unpack(&unpack_temp_dir)
    .map_err(|err| FrontendPkgErr::PkgInvalid(err.to_string()))?;
  remove_file(temp_inflated_path).map_err(FrontendPkgErr::HomeDirInaccessible)?;

  Ok(())
}

pub async fn get_frontend_file<T>(name: T) -> Result<(tokio::fs::File, PathBuf), std::io::Error>
where
  T: AsRef<Path>,
{
  let mut src_path = get_frontend_dir()?;
  src_path.push(name);

  let src_file_open_result = tokio::fs::OpenOptions::default()
    .create(false)
    .read(true)
    .write(false)
    .open(&src_path)
    .await;

  match src_file_open_result {
    Ok(src_file) => Ok((src_file, src_path)),
    Err(err) => Err(err),
  }
}

fn move_frontend_pkg_to_home() -> Result<(), FrontendPkgErr> {
  let frontend_temp_dir = get_frontend_temp_dir();
  let project_dir = get_project_home_dir().map_err(FrontendPkgErr::HomeDirInaccessible)?;
  for entry_result in walkdir::WalkDir::new(frontend_temp_dir) {
    let entry = match entry_result {
      Ok(e) => e,
      Err(err) => return Err(FrontendPkgErr::PkgUnpackErr(err.into())),
    };

    let mut tgt_path = project_dir.clone();
    let stripped_path = entry.path().strip_prefix(get_temp_dir()).unwrap();
    tgt_path.push(stripped_path);
    if entry.file_type().is_dir() {
      create_dir_all(tgt_path).map_err(FrontendPkgErr::PkgUnpackErr)?;
    } else if entry.file_type().is_file() {
      std::fs::copy(entry.path(), tgt_path).map_err(FrontendPkgErr::HomeDirInaccessible)?;
    }
  }

  Ok(())
}

fn check_new_pkg_manifest_against_existing_one() -> Result<(), FrontendPkgErr> {
  let temp_manifest = parse_temp_package_manifest()?;
  let project_manifest = match parse_project_package_manifest() {
    Ok(m) => m,
    Err(err) => {
      warn!("could not parse existing frontend package manifest: {err:?}");
      return Ok(());
    }
  };
  let compare_res = compare_package_manifests(&temp_manifest, &project_manifest)?;

  match compare_res {
    Ordering::Less => Err(FrontendPkgErr::PkgOutdated(
      temp_manifest.version_info.version,
      project_manifest.version_info.version,
    )),
    Ordering::Equal | Ordering::Greater => Ok(()),
  }
}

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

  let tgt_file_open_result = tokio::fs::OpenOptions::default()
    .create(true)
    .read(false)
    .write(true)
    .open(&target_path)
    .await
    .map_err(|err| err.to_string())?;

  let mut tgt_file_wrtier = tokio::io::BufWriter::new(tgt_file_open_result);

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
