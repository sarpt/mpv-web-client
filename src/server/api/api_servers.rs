use hyper::Response;
use serde::{Deserialize, Serialize};

use crate::{
  api_servers::{ApiServersService, ServerArguments},
  server::common::{ServiceResponse, empty_body, error_json_response, json_response},
};

#[derive(Deserialize)]
pub struct LocalApiServerSpawnRequest {
  name: String,
  port: Option<u16>,
  dir: Vec<String>,
  watch_dir: Option<bool>,
}

const DEFAULT_LOCAL_SERVER_PORT: u16 = 3001;

pub fn spawn_local_server(
  req: LocalApiServerSpawnRequest,
  servers_service: &mut ApiServersService,
) -> ServiceResponse {
  if req.dir.is_empty() {
    let response = error_json_response("at least one dir entry is required")?;
    return Ok(response);
  }

  let server_args = ServerArguments {
    port: req.port.unwrap_or(DEFAULT_LOCAL_SERVER_PORT),
    dir: &req.dir,
    watch_dir: req.watch_dir.unwrap_or(false),
  };

  match servers_service.spawn(req.name, &server_args) {
    Ok(()) => {
      let response = Response::new(empty_body());
      Ok(response)
    }
    Err(err) => {
      let response = error_json_response(format!("could not spawn a new api instance: {err}"))?;
      Ok(response)
    }
  }
}

#[derive(Deserialize)]
pub struct LocalApiServerStopRequest {
  name: String,
}

pub async fn stop_local_server(
  req: LocalApiServerStopRequest,
  servers_service: &mut ApiServersService,
) -> ServiceResponse {
  match servers_service.stop(req.name).await {
    Ok(()) => {
      let response = Response::new(empty_body());
      Ok(response)
    }
    Err(err) => {
      let response = error_json_response(format!("could not stop api instance: {err}"))?;
      Ok(response)
    }
  }
}

#[derive(Serialize)]
pub struct ApiServerInstance<'a> {
  pub local: bool,
  pub address: &'a str,
  pub name: &'a str,
}

#[derive(Serialize)]
pub struct ApiInstancesResponse<'a> {
  instances: &'a [ApiServerInstance<'a>],
}

pub fn get_all_instances(servers_service: &mut ApiServersService) -> ServiceResponse {
  let instances: Vec<ApiServerInstance> = servers_service
    .server_instances()
    .map(|(name, inst)| ApiServerInstance {
      local: inst.local,
      address: &inst.address,
      name,
    })
    .collect();
  let body = serde_json::to_string(&ApiInstancesResponse {
    instances: &instances,
  })?;
  let response = json_response(body);
  Ok(response)
}
