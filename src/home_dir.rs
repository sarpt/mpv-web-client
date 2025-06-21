use std::{env, path::PathBuf};

const HOME_SUBDIR: &str = ".mwc";
pub fn get_project_home_dir() -> Result<PathBuf, std::io::Error> {
  let mut src_path = match env::home_dir() {
    Some(path) => path,
    None => {
      return Err(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        "could not resolve home path",
      ));
    }
  };

  src_path.push(HOME_SUBDIR);
  Ok(src_path)
}
