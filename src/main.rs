use std::error::Error;

use parrot::client::Client;
use tracing::metadata::LevelFilter;
use tracing_subscriber::{prelude::*, EnvFilter};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    dotenvy::dotenv().ok();

    tracing_subscriber::registry()
        .with(EnvFilter::builder()
            .with_default_directive(LevelFilter::INFO.into())
            .from_env_lossy())
        .with(tracing_subscriber::fmt::layer())
        .init();

    let mut parrot = Client::default().await?;
    if let Err(err) = parrot.start().await {
        tracing::error!(err = ?err, "Fatality! Parrot crashed");
    };

    Ok(())
}
