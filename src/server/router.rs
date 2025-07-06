use hyper::Request;
use route_recognizer::Router;

enum PathRoutes {
  Frontend,
  Api(ApiPathRoutes),
}

enum ApiPathRoutes {
  FrontendLatest,
}

pub enum Routes {
  Frontend(Option<String>),
  Api(ApiRoutes),
}

pub enum ApiRoutes {
  FrontendLatest,
}

pub fn get_route(req: &Request<hyper::body::Incoming>) -> Option<Routes> {
  let mut router = Router::new();

  router.add(
    "/api/frontend/latest",
    PathRoutes::Api(ApiPathRoutes::FrontendLatest),
  );
  router.add("/*path", PathRoutes::Frontend);
  router.add("/", PathRoutes::Frontend);

  let match_result = router.recognize(req.uri().path());

  let routes = match match_result {
    Ok(m) => m,
    Err(_) => return None,
  };

  match routes.handler() {
    PathRoutes::Frontend => Some(Routes::Frontend(
      routes.params().find("path").map(|val| val.to_owned()),
    )),
    PathRoutes::Api(api_path) => match api_path {
      ApiPathRoutes::FrontendLatest => Some(Routes::Api(ApiRoutes::FrontendLatest)),
    },
  }
}
