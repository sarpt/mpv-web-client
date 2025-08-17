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

impl ApiServersService {
  pub fn new() -> Self {
    ApiServersService {
      instances: HashMap::new(),
    }
  }

  pub fn spawn(&mut self, name: String) -> Result<(), String> {
    let handle = Command::new("mpv-web-api")
      .spawn()
      .map_err(|err| format!("could not spawn an api instance: {}", err))?;
    let instance = ApiServerInstance {
      local: true,
      address: "127.0.0.1:3001".to_owned(),
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
