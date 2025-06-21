use std::{
  fs::OpenOptions,
  io::{BufReader, BufWriter, Seek, copy},
  path::Path,
};

use flate2::bufread::GzDecoder;
use tar::Archive;

use crate::home_dir::get_project_home_dir;

pub fn extract_frontend_pkg<T>(name: T) -> Result<(), std::io::Error>
where
  T: AsRef<Path>,
{
  let mut src_path = get_project_home_dir()?;
  src_path.push(&name);
  let src_file_open_handle = OpenOptions::new()
    .create(false)
    .read(true)
    .write(false)
    .open(&src_path)?;

  let mut temp_inflated_path = get_project_home_dir()?;
  temp_inflated_path.push("inflated.tar");
  let mut temp_inflated_file_open_handle = OpenOptions::new()
    .create(true)
    .truncate(true)
    .read(true)
    .write(true)
    .open(&temp_inflated_path)?;

  let src_pkg_reader = BufReader::with_capacity(64, src_file_open_handle);
  let mut decoder = GzDecoder::new(src_pkg_reader);
  let mut inflated_writer = BufWriter::with_capacity(64, &temp_inflated_file_open_handle);
  copy(&mut decoder, &mut inflated_writer)?;
  drop(inflated_writer);

  temp_inflated_file_open_handle.seek(std::io::SeekFrom::Start(0))?;
  let mut tar_archive = Archive::new(temp_inflated_file_open_handle);
  tar_archive.unpack(get_project_home_dir()?)?;

  Ok(())
}
