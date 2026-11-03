mod backup;
mod budget;
mod chat;
mod game;
mod mcp;
mod routes;
mod seed;
mod seed_budget;
mod seed_holdings;
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
        .with_writer(std::io::stderr)
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
        Some("mcp") => {
            // stdio belongs to the protocol; logs go to stderr only.
            let url = std::env::var("WORTHWATCH_URL").unwrap_or_else(|_| {
                let bind = std::env::var("WORTHWATCH_BIND").unwrap_or_else(|_| "127.0.0.1:7777".into());
                format!("http://{}", bind.replace("0.0.0.0", "127.0.0.1"))
            });
            return mcp::run(&url);
        }
        Some("backup") => {
            let b = backup::create(&data_dir(), chrono::Local::now())?;
            println!("wrote {}", data_dir().join(backup::DIR).join(&b.name).display());
            return Ok(());
        }
        Some("restore") => {
            let archive = args
                .iter()
                .skip(1)
                .find(|a| !a.starts_with("--"))
                .context("usage: worthwatch restore <backup.tar.gz> [--force]")?;
            let force = args.iter().any(|a| a == "--force");
            let r = backup::restore(std::path::Path::new(archive), &data_dir(), force, chrono::Local::now())?;
            println!("restored {} files into {}", r.files, data_dir().display());
            if let Some(prev) = r.previous {
                println!("what was there before is in {}", data_dir().join(backup::DIR).join(prev).display());
            }
            return Ok(());
        }
        Some("--help" | "-h" | "help") => {
            println!(
                "worthwatch                 run the server\nworthwatch seed [--force]  write a sample ledger, quotes and fund holdings to WORTHWATCH_DATA\nworthwatch mcp             MCP server on stdio for Claude Desktop; talks to the running app at WORTHWATCH_URL\nworthwatch backup          write a backup of WORTHWATCH_DATA to its backups/ folder\nworthwatch restore <file> [--force]  unpack a backup into WORTHWATCH_DATA (stop the server first)\n\nWORTHWATCH_DATA  data dir (default data): ledger.yaml, quotes.json, holdings/, backups/\nWORTHWATCH_BIND  address (default 127.0.0.1:7777)\nWORTHWATCH_OFFLINE=1  never fetch prices, FX or holdings files"
            );
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
    state.schedule_refresh();
    state.schedule_backups();
    budget::watch_icloud(&state);
    let app = routes::router(state.clone()).merge(budget::router(state)).merge(ui::router());

    tracing::info!("worthwatch listening on http://{bind} (data: {})", data_dir.display());
    let listener = tokio::net::TcpListener::bind(bind).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
