use flate2::bufread::GzDecoder;
use std::{
  fs::{create_dir_all, exists, remove_file, rename},
  io::{BufReader, BufWriter, Seek, copy},
  path::{Path, PathBuf},
};
use tar::Archive;

use crate::project_paths::{FRONTEND_DIR, get_frontend_dir, get_project_home_dir, get_temp_dir};

pub const INDEX_FILE_NAME: &str = "index.html";
const PKG_MANIFEST_NAME: &str = "pkg_manifest.toml";
pub enum FrontendPkgErr {
  IndexNotFound(Option<std::io::Error>),
  PkgNotProvided,
  PkgUnpackErr(std::io::Error),
  PkgInvalid(Option<std::io::Error>),
  HomeDirInaccessible(std::io::Error),
}

pub fn check_frontend_pkg<T>(pkg_path: Option<T>) -> Result<(), FrontendPkgErr>
where
  T: AsRef<Path>,
{
  if let Some(path) = &pkg_path {
    extract_frontend_pkg(path)?;
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
    let manifest_exists = exists(path).map_err(|err| FrontendPkgErr::PkgInvalid(Some(err)))?;
    if !manifest_exists {
      if pkg_path.is_none() {
        return Err(FrontendPkgErr::PkgNotProvided);
      } else {
        return Err(FrontendPkgErr::PkgInvalid(None));
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
    .map_err(|err| FrontendPkgErr::PkgInvalid(Some(err)))?;

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
  copy(&mut decoder, &mut inflated_writer).map_err(|err| FrontendPkgErr::PkgInvalid(Some(err)))?;
  drop(inflated_writer);

  temp_inflated_file_open_handle
    .seek(std::io::SeekFrom::Start(0))
    .map_err(FrontendPkgErr::HomeDirInaccessible)?;
  let unpack_temp_dir = {
    let mut dir = get_temp_dir();
    dir.push(FRONTEND_DIR);
    dir
  };

  let mut tar_archive = Archive::new(temp_inflated_file_open_handle);
  tar_archive
    .unpack(&unpack_temp_dir)
    .map_err(|err| FrontendPkgErr::PkgInvalid(Some(err)))?;

  remove_file(temp_inflated_path).map_err(FrontendPkgErr::HomeDirInaccessible)?;

  let project_dir = get_project_home_dir().map_err(FrontendPkgErr::HomeDirInaccessible)?;
  let temp_project_dir = get_temp_dir();
  for entry_result in walkdir::WalkDir::new(unpack_temp_dir) {
    let entry = match entry_result {
      Ok(e) => e,
      Err(err) => return Err(FrontendPkgErr::PkgUnpackErr(err.into())),
    };

    let mut tgt_path = project_dir.clone();
    let stripped_path = entry.path().strip_prefix(&temp_project_dir).unwrap();
    tgt_path.push(stripped_path);
    if entry.file_type().is_dir() {
      create_dir_all(tgt_path).map_err(FrontendPkgErr::PkgUnpackErr)?;
    } else if entry.file_type().is_file() {
      std::fs::copy(entry.path(), &tgt_path).map_err(FrontendPkgErr::HomeDirInaccessible)?;
    }
  }

  let frontend_dir = get_frontend_dir().map_err(FrontendPkgErr::HomeDirInaccessible)?;
  let manifest_file_path = {
    let mut path = frontend_dir.clone();
    path.push(PKG_MANIFEST_NAME);
    path
  };
  let new_manifest_file_path = {
    let mut path = get_project_home_dir().map_err(FrontendPkgErr::HomeDirInaccessible)?;
    path.push(PKG_MANIFEST_NAME);
    path
  };
  rename(manifest_file_path, new_manifest_file_path)
    .map_err(FrontendPkgErr::HomeDirInaccessible)?;

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
