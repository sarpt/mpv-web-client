use std::{
  env::{self},
  fs::create_dir_all,
  path::PathBuf,
};

const PROJECT_SUBDIR: &str = ".mwc";
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

  src_path.push(PROJECT_SUBDIR);
  Ok(src_path)
}

const FRONTEND_DIR: &str = "frontend";
pub fn get_frontend_dir() -> Result<PathBuf, std::io::Error> {
  let mut home_dir = get_project_home_dir()?;
  home_dir.push(FRONTEND_DIR);

  Ok(home_dir)
}

pub fn get_temp_dir() -> PathBuf {
  let mut path = env::temp_dir();
  path.push(PROJECT_SUBDIR);

  path
}

pub fn ensure_project_dirs() -> Result<(), std::io::Error> {
  let temp_dir = get_temp_dir();
  create_dir_all(temp_dir)?;

  let frontend_dir = get_frontend_dir()?;
  create_dir_all(frontend_dir)
}

pub fn get_frontend_temp_dir() -> PathBuf {
  let mut dir = get_temp_dir();
  dir.push(FRONTEND_DIR);
  dir
}
