use flate2::bufread::GzDecoder;
use std::{
  fs::{remove_file, rename},
  io::{BufReader, BufWriter, Seek, copy},
  path::{Path, PathBuf},
};
use tar::Archive;

use crate::home_dir::get_project_home_dir;

const STREAM_CHUNK_SIZE: usize = 1024 * 1024 * 64;
const TEMP_INFLATED_PKG_NAME: &str = "inflated.tar";
const PKG_MANIFEST_NAME: &str = "pkg_manifest.toml";

pub fn extract_frontend_pkg<T>(name: T) -> Result<(), std::io::Error>
where
  T: AsRef<Path>,
{
  let mut src_path = get_project_home_dir()?;
  src_path.push(&name);
  let src_file_open_handle = std::fs::OpenOptions::new()
    .create(false)
    .read(true)
    .write(false)
    .open(&src_path)?;

  let mut temp_inflated_path = get_project_home_dir()?;
  temp_inflated_path.push(TEMP_INFLATED_PKG_NAME);
  let mut temp_inflated_file_open_handle = std::fs::OpenOptions::new()
    .create(true)
    .truncate(true)
    .read(true)
    .write(true)
    .open(&temp_inflated_path)?;

  let src_pkg_reader = BufReader::with_capacity(STREAM_CHUNK_SIZE, src_file_open_handle);
  let mut decoder = GzDecoder::new(src_pkg_reader);
  let mut inflated_writer =
    BufWriter::with_capacity(STREAM_CHUNK_SIZE, &temp_inflated_file_open_handle);
  copy(&mut decoder, &mut inflated_writer)?;
  drop(inflated_writer);

  temp_inflated_file_open_handle.seek(std::io::SeekFrom::Start(0))?;
  let frontend_dir = get_frontend_dir()?;
  let mut tar_archive = Archive::new(temp_inflated_file_open_handle);
  tar_archive.unpack(frontend_dir)?;

  remove_file(temp_inflated_path)?;

  let manifest_file_path = {
    let mut path = get_frontend_dir()?;
    path.push(PKG_MANIFEST_NAME);
    path
  };
  let new_manifest_file_path = {
    let mut path = get_project_home_dir()?;
    path.push(PKG_MANIFEST_NAME);
    path
  };
  rename(manifest_file_path, new_manifest_file_path)?;

  Ok(())
}

pub async fn get_frontend_file<T>(name: T) -> Result<(tokio::fs::File, PathBuf), std::io::Error>
where
  T: AsRef<Path>,
{
  let mut src_path = get_project_home_dir()?;
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

const FRONTEND_DIR: &str = "frontend";
fn get_frontend_dir() -> Result<PathBuf, std::io::Error> {
  let mut home_dir = get_project_home_dir()?;
  home_dir.push(FRONTEND_DIR);

  Ok(home_dir)
}
