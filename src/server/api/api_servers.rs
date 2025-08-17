use hyper::{Response, StatusCode};
use serde::{Deserialize, Serialize};

use crate::{
  api_servers::ApiServersService,
  server::{
    api::ApiErr,
    common::{ServiceResponse, empty_body, json_response},
  },
};

#[derive(Deserialize)]
pub struct LocalApiServerSpawnRequest {
  name: String,
}

pub fn spawn_local_server(
  req: LocalApiServerSpawnRequest,
  servers_service: &mut ApiServersService,
) -> ServiceResponse {
  match servers_service.spawn(req.name) {
    Ok(()) => {
      let response = Response::new(empty_body());
      Ok(response)
    }
    Err(err) => {
      let body = serde_json::to_string(&ApiErr {
        err_msg: format!("could not spawn a new api instance: {err}"),
      })?;
      let mut response = json_response(body);
      *response.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
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
      let body = serde_json::to_string(&ApiErr {
        err_msg: format!("could not stop api instance: {err}"),
      })?;
      let mut response = json_response(body);
      *response.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
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
