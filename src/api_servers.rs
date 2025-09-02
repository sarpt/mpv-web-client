use std::{
  collections::{HashMap, hash_map::Iter},
  mem::take,
  path::PathBuf,
  process::Stdio,
  time::Duration,
};

use futures::future::{join, join_all};
use log::{debug, error, info, warn};
use nix::{
  sys::signal::{self, Signal},
  unistd::Pid,
};
use tokio::{
  fs::{File, OpenOptions, remove_file},
  io::{BufReader, BufWriter},
  process::{Child, Command},
  select, spawn,
  task::JoinHandle,
  time::sleep,
};

use crate::common::tarflate::compress_files;

pub struct ApiServerInstance {
  pub local: bool,
  pub address: String,
  handle: Child,
}

pub struct ApiServersService {
  instances: HashMap<String, ApiServerInstance>,
  logs_dir: PathBuf,
  logs_join_handles: Vec<JoinHandle<()>>,
}

const LOCAL_SERVER_IP_ADDR: &str = "127.0.0.1";
const LOCAL_SERVER_BIN_NAME: &str = "mpv-web-api";
const ADDR_ARG: &str = "--addr";
const DIR_ARG: &str = "--dir";
const WATCH_DIR_ARG: &str = "--watch-dir";

pub struct ServerArguments<'a> {
  pub port: u16,
  pub dir: &'a [String],
  pub watch_dir: bool,
}

impl ApiServersService {
  pub fn new(logs_dir: PathBuf) -> Self {
    ApiServersService {
      instances: HashMap::new(),
      logs_dir,
      logs_join_handles: Vec::new(),
    }
  }

  pub async fn spawn<'a>(
    &mut self,
    name: String,
    server_args: &ServerArguments<'a>,
  ) -> Result<(), String> {
    let mut cmd = Command::new(LOCAL_SERVER_BIN_NAME);

    let address = format!("{}:{}", LOCAL_SERVER_IP_ADDR, server_args.port);
    cmd.args([ADDR_ARG, &address]);

    for dir in server_args.dir {
      cmd.args([DIR_ARG, dir]);
    }

    if server_args.watch_dir {
      cmd.arg(WATCH_DIR_ARG);
    }

    let mut handle = cmd
      .stdout(Stdio::piped())
      .stderr(Stdio::piped())
      .spawn()
      .map_err(|err| format!("could not spawn an api instance on address {address}: {err}"))?;

    let mut stdout = handle.stdout.take().unwrap();
    let mut stderr = handle.stderr.take().unwrap();

    let (stdout_name, stderr_name) = Self::get_output_stream_filenames(&name);
    let mut stdout_file_writer = self.get_stream_file_writer(&stdout_name).await?;
    let mut stderr_file_writer = self.get_stream_file_writer(&stderr_name).await?;
    let join_handle = spawn(async move {
      let stdout_fut = tokio::io::copy(&mut stdout, &mut stdout_file_writer);
      let stderr_fut = tokio::io::copy(&mut stderr, &mut stderr_file_writer);

      _ = join(stdout_fut, stderr_fut).await;
    });
    self.logs_join_handles.push(join_handle);

    let instance = ApiServerInstance {
      local: true,
      address,
      handle,
    };
    self.instances.insert(name, instance);

    Ok(())
  }

  pub async fn get_logs_readers(
    &self,
    name: &str,
  ) -> Result<(BufReader<File>, BufReader<File>), String> {
    if !self.instances.contains_key(name) {
      return Err(format!("No api server instance with name {name} exists"));
    }

    let (stdout_filename, stderr_filename) = Self::get_output_stream_filenames(name);
    Ok((
      self.get_stream_file_reader(&stdout_filename).await?,
      self.get_stream_file_reader(&stderr_filename).await?,
    ))
  }

  pub fn server_instances(&'_ self) -> Iter<'_, String, ApiServerInstance> {
    self.instances.iter()
  }

  pub async fn shutdown(&mut self, shutdown_timeout: u32) {
    select! {
      _ = join_all(take(&mut self.logs_join_handles)) => {
        debug!("finished writing all streams from api servers")
      },
      _ = sleep(Duration::from_secs(shutdown_timeout.into())) => {
        warn!("forcing shutdown due to timeout on waiting for all streams of {shutdown_timeout} seconds")
      }
    }
  }

  pub async fn stop(&mut self, name: String) -> Result<(), String> {
    let mut instance = self.instances.remove(&name).ok_or(format!(
      "could not find api server instance with name {}",
      &name
    ))?;
    let id = instance
      .handle
      .id()
      .ok_or(format!("instance with name {} has already finished", &name))?;

    signal::kill(Pid::from_raw(id as i32), Signal::SIGTERM).unwrap();
    let result = instance
      .handle
      .wait()
      .await
      .map_err(|err| format!("could not await on instance closure: {err}"))?;
    info!(
      "instance pid: {id}; name: {} closed with result: {result}",
      &name
    );
    let archive_result = self.archive_logs(&name).await;
    if let Err(archive_err) = archive_result {
      error!("could not archive logs for {}: {archive_err}", &name);
      return Err(archive_err);
    }

    Ok(())
  }

  async fn archive_logs(&self, name: &str) -> Result<(), String> {
    let (stdout, stderr) = Self::get_output_stream_filenames(name);
    let mut stdout_path = PathBuf::from(&self.logs_dir.clone());
    stdout_path.push(stdout);
    let mut stderr_path = PathBuf::from(&self.logs_dir.clone());
    stderr_path.push(stderr);
    let mut archive_path = self.logs_dir.clone();
    archive_path.push(format!("{}_logs_archive.tar.gz", &name));

    let paths_to_compress = [stdout_path.clone(), stderr_path.clone()];
    spawn(async move { compress_files(&archive_path, &paths_to_compress) })
      .await
      .map_err(|err| format!("could not join spawned compression task: {err}"))?
      .map_err(|reason| format!("could not compress archive: {reason}"))?;

    remove_file(&stdout_path)
      .await
      .map_err(|err| format!("could not remove stdout output: {err}"))?;
    remove_file(&stderr_path)
      .await
      .map_err(|err| format!("could not remove stderr output: {err}"))?;

    Ok(())
  }

  fn get_output_stream_filenames(name: &str) -> (String, String) {
    (
      format!("mwa_{}_stdout", name),
      format!("mwa_{}_stderr", name),
    )
  }

  async fn get_stream_file_writer(&self, filename: &str) -> Result<BufWriter<File>, String> {
    let mut path = self.logs_dir.clone();
    path.push(filename);

    let target_file = OpenOptions::default()
      .create(true)
      .read(false)
      .write(true)
      .open(&path)
      .await
      .map_err(|err| {
        format!(
          "could not open file for stdout writing {}: {err}",
          &path.to_string_lossy()
        )
      })?;

    Ok(BufWriter::new(target_file))
  }

  async fn get_stream_file_reader(&self, filename: &str) -> Result<BufReader<File>, String> {
    let mut path = self.logs_dir.clone();
    path.push(filename);

    let target_file = OpenOptions::default()
      .create(false)
      .read(true)
      .write(false)
      .open(&path)
      .await
      .map_err(|err| {
        format!(
          "could not open file for stdout writing {}: {err}",
          &path.to_string_lossy()
        )
      })?;

    Ok(BufReader::new(target_file))
  }
}
