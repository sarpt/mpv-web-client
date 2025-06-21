use std::error::Error;

use crate::server::serve;

mod server;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn Error>> {
  serve().await
}
