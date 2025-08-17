use serde::Serialize;

pub mod api_servers;
pub mod frontend;
pub mod management;

#[derive(Serialize)]
pub struct ApiErr<'a> {
  pub err_msg: &'a str,
}
