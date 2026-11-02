mod routes;
mod seed;
mod state;
mod ui;

use std::net::SocketAddr;
use std::path::PathBuf;

use anyhow::Context;
use tracing_subscriber::EnvFilter;

fn data_dir() -> PathBuf {
    std::env::var("WORTHWATCH_DATA").map(PathBuf::from).unwrap_or_else(|_| "data".into())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse()?))
        .init();

    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("seed") => {
            let force = args.iter().any(|a| a == "--force");
            let path = seed::write_seed(&data_dir(), chrono::Local::now().date_naive(), force)?;
            println!("wrote sample ledger to {}", path.display());
            return Ok(());
        }
        Some("--help" | "-h" | "help") => {
            println!("worthwatch            run the server\nworthwatch seed [--force]   write a sample ledger to WORTHWATCH_DATA");
            return Ok(());
        }
        _ => {}
    }

    let data_dir = data_dir();
    let bind: SocketAddr =
        std::env::var("WORTHWATCH_BIND").unwrap_or_else(|_| "127.0.0.1:7777".into()).parse()?;

    let state = state::AppState::open(&data_dir)
        .with_context(|| format!("opening data dir {}", data_dir.display()))?;
    state.watch();
    let app = routes::router(state).merge(ui::router());

    tracing::info!("worthwatch listening on http://{bind} (data: {})", data_dir.display());
    let listener = tokio::net::TcpListener::bind(bind).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
