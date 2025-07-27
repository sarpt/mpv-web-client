use http_body_util::BodyExt;
use hyper::{Method, Request};
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
}

pub enum Routes {
  Frontend(Option<String>, Vec<String>),
  Api(ApiRoutes),
}

pub enum ApiRoutes {
  FrontendLatest,
  FrontendUpdate(Semver),
  Shutdown,
}

pub enum RoutingErr {
  Unmatched,
  InvalidMethod,
  InvalidRequest(String),
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
      ApiPathRoutes::Shutdown => Ok(Routes::Api(ApiRoutes::Shutdown)),
      ApiPathRoutes::FrontendLatest => Ok(Routes::Api(ApiRoutes::FrontendLatest)),
      ApiPathRoutes::FrontendUpdate => {
        if req.method() != Method::POST {
          return Err(RoutingErr::InvalidMethod);
        }

        let body_bytes = req
          .into_body()
          .collect()
          .await
          .map_err(|err| RoutingErr::InvalidRequest(format!("invalid body: {err}")))?
          .to_bytes();
        let request_string = String::from_utf8(body_bytes.into())
          .map_err(|err| RoutingErr::InvalidRequest(format!("invalid body: {err}")))?;

        let request: FrontendUpdateRequest = serde_json::from_str(request_string.as_ref())
          .map_err(|err| {
            RoutingErr::InvalidRequest(format!("incorrect version provided: {err}"))
          })?;
        Ok(Routes::Api(ApiRoutes::FrontendUpdate(request.version)))
      }
    },
  }
}

#[derive(Deserialize)]
struct FrontendUpdateRequest {
  version: Semver,
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
