use hyper::Request;
use route_recognizer::Router;

enum PathRoutes {
  Frontend,
}

pub enum Routes {
  Frontend(Option<String>),
}

pub fn get_route(req: &Request<hyper::body::Incoming>) -> Option<Routes> {
  let mut router = Router::new();

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
  }
}
