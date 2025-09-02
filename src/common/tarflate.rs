use flate2::{Compression, bufread::GzEncoder};
use std::{
  fs::{OpenOptions, remove_file},
  io::{BufReader, BufWriter, Seek, copy},
  path::{Path, PathBuf},
};
use tar::Builder;

pub fn compress_files<T>(out: &T, src_paths: &[T]) -> Result<(), String>
where
  T: AsRef<Path>,
{
  let mut temp_tar_file_path = PathBuf::from(out.as_ref());
  temp_tar_file_path.set_extension("temp");
  let mut temp_tar_file = OpenOptions::new()
    .create(true)
    .truncate(true)
    .read(true)
    .write(true)
    .open(&temp_tar_file_path)
    .map_err(|err| format!("could not open file for stdout writing: {err}",))?;
  let mut writer = BufWriter::new(&temp_tar_file);
  let mut archive_builder = Builder::new(&mut writer);

  for src_path in src_paths {
    let archive_path = PathBuf::from(src_path.as_ref().file_name().ok_or(format!(
      "provided file name {} can't be archived",
      src_path.as_ref().to_string_lossy()
    ))?);
    archive_builder
      .append_path_with_name(src_path, &archive_path)
      .map_err(|err| {
        format!(
          "could not put {} into archive: {err}",
          &archive_path.to_string_lossy()
        )
      })?;
  }
  archive_builder
    .finish()
    .map_err(|err| format!("could not finish creating a temporary tar archive: {err}"))?;
  drop(archive_builder);
  drop(writer);

  temp_tar_file
    .seek(std::io::SeekFrom::Start(0))
    .map_err(|err| format!("could not seek temporary tar file: {err}"))?;

  let reader = BufReader::new(&temp_tar_file);
  let mut archive_encoder = GzEncoder::new(reader, Compression::fast());
  let target_archive_path = PathBuf::from(out.as_ref());
  let target_archive_file = OpenOptions::new()
    .create(true)
    .truncate(true)
    .read(false)
    .write(true)
    .open(&target_archive_path)
    .map_err(|err| format!("could not open file for stdout writing: {err}",))?;
  let mut archive_writer = BufWriter::new(target_archive_file);

  copy(&mut archive_encoder, &mut archive_writer).map_err(|err| {
    format!(
      "could not create compressed archive file in {}: {err}",
      &target_archive_path.to_string_lossy()
    )
  })?;

  remove_file(temp_tar_file_path)
    .map_err(|err| format!("could not remove temp tar file: {err}"))?;

  Ok(())
}
