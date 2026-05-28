use std::env;
use std::error::Error;

use oar_http_facade::{run, OarHttpFacadeConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt().init();

    let config = OarHttpFacadeConfig::from_env_map(&|key| env::var(key).ok())?;
    run(config).await?;
    Ok(())
}
