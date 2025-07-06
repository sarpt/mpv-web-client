use http_body_util::BodyExt;
use hyper::{Method, Request};
use route_recognizer::Router;

use crate::frontend::releases::Release;

enum PathRoutes {
  Frontend,
  Api(ApiPathRoutes),
}

enum ApiPathRoutes {
  FrontendLatest,
  FrontendUpdate,
}

pub enum Routes {
  Frontend(Option<String>),
  Api(ApiRoutes),
}

pub enum ApiRoutes {
  FrontendLatest,
  FrontendUpdate(Release),
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
    )),
    PathRoutes::Api(api_path) => match api_path {
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

        let release: Release = serde_json::from_slice(request_string.as_ref()).map_err(|err| {
          RoutingErr::InvalidRequest(format!("incorrect release provided: {err}"))
        })?;
        Ok(Routes::Api(ApiRoutes::FrontendUpdate(release)))
      }
    },
  }
}
