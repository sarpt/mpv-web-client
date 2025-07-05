use std::{
  fs::remove_file,
  io::{BufReader, BufWriter, Seek, copy},
  path::Path,
};

use flate2::bufread::GzDecoder;
use tar::Archive;

use crate::{
  frontend::FrontendPkgErr,
  project_paths::{get_frontend_temp_dir, get_temp_dir},
};

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
