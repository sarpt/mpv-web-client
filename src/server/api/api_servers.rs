use http_body_util::combinators::BoxBody;
use hyper::{Response, StatusCode, body::Bytes};
use serde::Serialize;

use crate::{
  api_servers::ApiServersService,
  server::{
    api::ApiErr,
    common::{ServiceError, empty_body, json_response},
  },
};

pub fn spawn_local_server(
  name: String,
  servers_service: &mut ApiServersService,
) -> Result<Response<BoxBody<Bytes, ServiceError>>, ServiceError> {
  match servers_service.spawn(name) {
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

pub async fn stop_local_server(
  name: String,
  servers_service: &mut ApiServersService,
) -> Result<Response<BoxBody<Bytes, ServiceError>>, ServiceError> {
  match servers_service.stop(name).await {
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

pub fn get_all_instances(
  servers_service: &mut ApiServersService,
) -> Result<Response<BoxBody<Bytes, ServiceError>>, ServiceError> {
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
