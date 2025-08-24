use std::{
  collections::{HashMap, hash_map::Iter},
  mem::take,
  path::PathBuf,
  process::Stdio,
};

use futures::future::{join, join_all};
use log::info;
use nix::{
  sys::signal::{self, Signal},
  unistd::Pid,
};
use tokio::{
  fs::{File, OpenOptions},
  io::BufWriter,
  process::{Child, Command},
  spawn,
  task::JoinHandle,
};

pub struct ApiServerInstance {
  pub local: bool,
  pub address: String,
  handle: Child,
}

pub struct ApiServersService {
  instances: HashMap<String, ApiServerInstance>,
  project_dir: PathBuf,
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
  pub fn new(project_dir: PathBuf) -> Self {
    ApiServersService {
      instances: HashMap::new(),
      project_dir,
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

    let mut stdout_file_writer = self
      .get_stream_file_writer(&format!("mwa_{}_{}_stdout", &name, server_args.port))
      .await?;
    let mut stderr_file_writer = self
      .get_stream_file_writer(&format!("mwa_{}_{}_stderr", &name, server_args.port))
      .await?;
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

  pub fn server_instances(&'_ self) -> Iter<'_, String, ApiServerInstance> {
    self.instances.iter()
  }

  pub async fn shutdown(&mut self) {
    join_all(take(&mut self.logs_join_handles)).await;
  }

  pub async fn stop(&mut self, name: String) -> Result<(), String> {
    let mut instance = self.instances.remove(&name).ok_or(format!(
      "could not find api server instance with name {name}"
    ))?;
    let id = instance
      .handle
      .id()
      .ok_or(format!("instance with name {name} has already finished"))?;

    signal::kill(Pid::from_raw(id as i32), Signal::SIGTERM).unwrap();
    let result = instance
      .handle
      .wait()
      .await
      .map_err(|err| format!("could not await on instance closure: {err}"))?;
    info!("instance pid: {id}; name: {name} closed with result: {result}");
    Ok(())
  }

  async fn get_stream_file_writer(&self, filename: &str) -> Result<BufWriter<File>, String> {
    let mut path = self.project_dir.clone();
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
}
