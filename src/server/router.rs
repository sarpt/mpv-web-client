use http_body_util::BodyExt;
use hyper::{Method, Request, body::Incoming};
use route_recognizer::Router;
use serde::Deserialize;

use crate::common::semver::Semver;

enum PathRoutes {
  Frontend,
  Api(ApiPathRoutes),
}

enum ApiPathRoutes {
  FrontendLatest,
  FrontendUpdate,
  Shutdown,
  ApiServers(ApiServersPathRoutes),
}

enum ApiServersPathRoutes {
  Spawn,
  All,
}

pub enum Routes {
  Frontend(Option<String>, Vec<String>),
  Api(ApiRoutes),
}

pub enum ApiRoutes {
  FrontendLatest,
  FrontendUpdate(Semver),
  Shutdown,
  ApiServers(ApiServersRoutes),
}

pub enum ApiServersRoutes {
  Spawn(String),
  All,
}

pub enum RoutingErr {
  Unmatched,
  InvalidMethod,
  InvalidRequestBody(String),
}

pub async fn get_route(req: Request<hyper::body::Incoming>) -> Result<Routes, RoutingErr> {
  let mut router = Router::new();

  router.add(
    "/api/frontend/latest",
    PathRoutes::Api(ApiPathRoutes::FrontendLatest),
  );
  router.add(
    "/api/frontend/update",
    PathRoutes::Api(ApiPathRoutes::FrontendUpdate),
  );
  router.add(
    "/api/servers/spawn",
    PathRoutes::Api(ApiPathRoutes::ApiServers(ApiServersPathRoutes::Spawn)),
  );
  router.add(
    "/api/servers",
    PathRoutes::Api(ApiPathRoutes::ApiServers(ApiServersPathRoutes::All)),
  );
  router.add("/api/shutdown", PathRoutes::Api(ApiPathRoutes::Shutdown));
  router.add("/*path", PathRoutes::Frontend);
  router.add("/", PathRoutes::Frontend);

  let match_result = router.recognize(req.uri().path());

  let routes = match match_result {
    Ok(m) => m,
    Err(_) => return Err(RoutingErr::Unmatched),
  };

  match routes.handler() {
    PathRoutes::Frontend => Ok(Routes::Frontend(
      routes.params().find("path").map(|val| val.to_owned()),
      parse_accepted_encodings(req),
    )),
    PathRoutes::Api(api_path) => match api_path {
      ApiPathRoutes::ApiServers(api_servers_path) => match api_servers_path {
        ApiServersPathRoutes::Spawn => {
          if req.method() != Method::POST {
            return Err(RoutingErr::InvalidMethod);
          }

          let body = parse_request_body::<LocalApiServerSpawnRequest>(req).await?;
          Ok(Routes::Api(ApiRoutes::ApiServers(ApiServersRoutes::Spawn(
            body.name,
          ))))
        }
        ApiServersPathRoutes::All => Ok(Routes::Api(ApiRoutes::ApiServers(ApiServersRoutes::All))),
      },
      ApiPathRoutes::Shutdown => Ok(Routes::Api(ApiRoutes::Shutdown)),
      ApiPathRoutes::FrontendLatest => Ok(Routes::Api(ApiRoutes::FrontendLatest)),
      ApiPathRoutes::FrontendUpdate => {
        if req.method() != Method::POST {
          return Err(RoutingErr::InvalidMethod);
        }

        let body = parse_request_body::<FrontendUpdateRequest>(req).await?;
        Ok(Routes::Api(ApiRoutes::FrontendUpdate(body.version)))
      }
    },
  }
}

#[derive(Deserialize)]
struct FrontendUpdateRequest {
  version: Semver,
}

#[derive(Deserialize)]
struct LocalApiServerSpawnRequest {
  name: String,
}

async fn parse_request_body<T>(req: Request<Incoming>) -> Result<T, RoutingErr>
where
  T: for<'a> Deserialize<'a>,
{
  let body_bytes = req
    .into_body()
    .collect()
    .await
    .map_err(|err| RoutingErr::InvalidRequestBody(format!("cannot collect request body: {err}")))?
    .to_bytes();
  let request_string = String::from_utf8(body_bytes.into()).map_err(|err| {
    RoutingErr::InvalidRequestBody(format!("cannot convert body to string: {err}"))
  })?;

  let request: T = serde_json::from_str(request_string.as_ref()).map_err(|err| {
    RoutingErr::InvalidRequestBody(format!("incorrect request body provided: {err}"))
  })?;
  Ok(request)
}

const ENCODINGS_SEPARATOR: &str = ",";
const ACCEPT_ANY_ENCODING: &str = "*";
fn parse_accepted_encodings(req: Request<hyper::body::Incoming>) -> Vec<String> {
  let mut encodings = req
    .headers()
    .get("Accept-Encoding")
    .map_or(Vec::new(), |head| {
      head.to_str().map_or(Vec::new(), split_encodings)
    });
  if encodings.is_empty() {
    encodings.push(ACCEPT_ANY_ENCODING.to_owned());
  }

  encodings
}

fn split_encodings(s: &str) -> Vec<String> {
  s.split(ENCODINGS_SEPARATOR)
    .map(|s| s.trim().to_owned())
    .collect::<Vec<String>>()
}
