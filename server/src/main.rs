mod routes;
mod state;
mod ui;

use std::net::SocketAddr;
use std::path::PathBuf;

use anyhow::Context;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse()?))
        .init();

    let data_dir: PathBuf =
        std::env::var("WORTHWATCH_DATA").map(PathBuf::from).unwrap_or_else(|_| "data".into());
    let bind: SocketAddr =
        std::env::var("WORTHWATCH_BIND").unwrap_or_else(|_| "127.0.0.1:7777".into()).parse()?;

    let state = state::AppState::open(&data_dir)
        .with_context(|| format!("opening data dir {}", data_dir.display()))?;
    let app = routes::router(state).merge(ui::router());

    tracing::info!("worthwatch listening on http://{bind} (data: {})", data_dir.display());
    let listener = tokio::net::TcpListener::bind(bind).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
