use std::error::Error;

use oar_http_facade::{run, OarHttpFacadeConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt().init();

    run(OarHttpFacadeConfig::default()).await?;
    Ok(())
}
