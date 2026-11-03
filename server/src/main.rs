mod api;
mod backup;
mod mcp;
mod security;
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

/// Files Worthwatch writes (ledger, transactions, backups, the token) are readable by
/// this user only, whatever the shell's umask.
fn private_files() {
    #[cfg(unix)]
    // SAFETY: umask only swaps the process's file mode mask; it can't fail.
    unsafe {
        libc::umask(0o077);
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    private_files();
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
                "worthwatch                 run the server\nworthwatch seed [--force]  write a sample ledger, quotes and fund holdings to WORTHWATCH_DATA\nworthwatch mcp             MCP server on stdio for Claude Desktop; talks to the running app at WORTHWATCH_URL\nworthwatch backup          write a backup of WORTHWATCH_DATA to its backups/ folder\nworthwatch restore <file> [--force]  unpack a backup into WORTHWATCH_DATA (stop the server first)\n\nWORTHWATCH_DATA  data dir (default data): ledger.yaml, quotes.json, holdings/, backups/\nWORTHWATCH_BIND  address (default 127.0.0.1:7777)\nWORTHWATCH_OFFLINE=1  never fetch prices, FX or holdings files\nWORTHWATCH_ALLOWED_HOSTS  extra host names allowed besides localhost (comma separated)\nWORTHWATCH_TAILSCALE_OWNER  Tailscale login(s) that may open the whole app over tailscale serve\nWORTHWATCH_TAPS_DIR  folder the Apple Pay taps are read from (a read-only mount in a container)"
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
    api::budget::feed::watch_icloud(&state);
    let policy = std::sync::Arc::new(security::Policy::from_env());
    let app = api::router(state)
        .merge(ui::router())
        .layer(axum::middleware::from_fn_with_state(policy, security::guard));

    tracing::info!("worthwatch listening on http://{bind} (data: {})", data_dir.display());
    if std::env::var_os("WORTHWATCH_IN_CONTAINER").is_some() {
        tracing::info!(
            "in a container: publish the port on 127.0.0.1 only (-p 127.0.0.1:7777:7777), as deploy/container/run.sh does"
        );
    } else if !bind.ip().is_loopback() {
        tracing::warn!(
            "listening on {bind}, not just this computer. Anyone who can reach this port can see and change your data."
        );
    }
    let listener = tokio::net::TcpListener::bind(bind).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
