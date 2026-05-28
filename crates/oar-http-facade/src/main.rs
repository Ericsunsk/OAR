use std::env;
use std::error::Error;

use oar_http_facade::{run_with_runtime, OarHttpFacadeConfig, OarHttpFacadeRuntime};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let _ = dotenvy::dotenv();
    tracing_subscriber::fmt().init();

    let config = OarHttpFacadeConfig::from_env_map(&|key| env::var(key).ok())?;
    let runtime = OarHttpFacadeRuntime::from_env_map_async(&|key| env::var(key).ok()).await?;
    run_with_runtime(config, runtime).await?;
    Ok(())
}
