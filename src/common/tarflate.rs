use std::{
  fs::OpenOptions,
  io::BufWriter,
  path::{Path, PathBuf},
};
use tar::Builder;

pub fn compress_files<T>(out: &T, src_paths: &[T]) -> Result<(), String>
where
  T: AsRef<Path>,
{
  let mut temp_tar_file = PathBuf::from(out.as_ref());
  temp_tar_file.set_extension("tar.temp");

  let target_file = OpenOptions::new()
    .create(true)
    .truncate(true)
    .read(false)
    .write(true)
    .open(&temp_tar_file)
    .map_err(|err| format!("could not open file for stdout writing: {err}",))?;
  let writer = BufWriter::new(target_file);
  let mut ar = Builder::new(writer);

  for src_path in src_paths {
    let archive_path = PathBuf::from(src_path.as_ref().file_name().ok_or(format!(
      "provided file name {} can't be archived",
      src_path.as_ref().to_string_lossy()
    ))?);
    ar.append_path_with_name(src_path, &archive_path)
      .map_err(|err| {
        format!(
          "could not put {} into archive: {err}",
          &archive_path.to_string_lossy()
        )
      })?;
  }

  ar.finish()
    .map_err(|err| format!("could not finish creating a temporary tar archive: {err}"))?;

  Ok(())
}
