use tokio::process::{Child, Command};

pub struct ApiServerInstance {
  pub local: bool,
  pub address: String,
  pub name: String,
  handle: Child,
}

pub struct ApiServersService {
  instances: Vec<ApiServerInstance>,
}

impl ApiServersService {
  pub fn new() -> Self {
    ApiServersService {
      instances: Vec::new(),
    }
  }

  pub fn start(&mut self, name: String) -> Result<(), String> {
    let handle = Command::new("mpv-web-api")
      .spawn()
      .map_err(|err| format!("could not spawn an api instance: {}", err))?;
    let instance = ApiServerInstance {
      local: true,
      address: "127.0.0.1:3001".to_owned(),
      name,
      handle,
    };

    self.instances.push(instance);

    Ok(())
  }

  pub fn server_instances(&self) -> &[ApiServerInstance] {
    &self.instances
  }
}
