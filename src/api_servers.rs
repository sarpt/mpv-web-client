use std::collections::{HashMap, hash_map::Iter};

use log::info;
use nix::{
  sys::signal::{self, Signal},
  unistd::Pid,
};
use tokio::process::{Child, Command};

pub struct ApiServerInstance {
  pub local: bool,
  pub address: String,
  handle: Child,
}

pub struct ApiServersService {
  instances: HashMap<String, ApiServerInstance>,
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
  pub fn new() -> Self {
    ApiServersService {
      instances: HashMap::new(),
    }
  }

  pub fn spawn(&mut self, name: String, server_args: &ServerArguments) -> Result<(), String> {
    let mut cmd = Command::new(LOCAL_SERVER_BIN_NAME);

    let address = format!("{}:{}", LOCAL_SERVER_IP_ADDR, server_args.port);
    cmd.args([ADDR_ARG, &address]);

    for dir in server_args.dir {
      cmd.args([DIR_ARG, dir]);
    }

    if server_args.watch_dir {
      cmd.arg(WATCH_DIR_ARG);
    }

    let handle = cmd
      .spawn()
      .map_err(|err| format!("could not spawn an api instance on address {address}: {err}"))?;

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
}
