# worthwatch

Self-hosted net worth and portfolio tracker. One Rust binary, your data in one folder, no bank connections.

Status: **Stage 2**: hand-edited or typed-in balances, monthly check-in, net worth dashboard with history, positions valued from fetched prices (Yahoo Finance or Stooq) and Bank of Canada FX with manual fallbacks, a Portfolio screen, and pasting a Wealthsimple/Questrade holdings export into Check-in. See `docs/` and the design doc for the plan.

## Run (dev)

```sh
# terminal 1: the server (creates data/ledger.yaml on first run)
cargo run
# terminal 2: the UI with hot reload, proxying /api to the server
pnpm --dir ui install && pnpm --dir ui dev
```

Open http://localhost:5173. `cargo run -- seed` writes a realistic sample ledger to look at first. To run the single binary with the UI embedded:

```sh
pnpm --dir ui build && cargo run --release
```

Open http://127.0.0.1:7777. See "Security" before exposing it to other devices.

## Run (Docker)

```sh
docker compose -f deploy/docker-compose.yml up -d --build
```

The container binds to localhost on the host by default. `WORTHWATCH_HOST=0.0.0.0 docker compose ... up -d` exposes it on your network, `WORTHWATCH_PORT=8080` changes the host port.

## Prices and FX

Public market data is the only thing the app fetches. On startup and once a day it asks Yahoo Finance for a close per held symbol (a year of history the first time a symbol appears) and the Bank of Canada for the daily rate of each currency you hold. Results go to `quotes.json` beside the ledger; delete it to start over. A price or rate you type on Check-in or in `settings.fx` always wins for its date and is the fallback when nothing was fetched. Turn sources off or switch them in the ledger:

```yaml
settings:
  base_currency: CAD
  providers: { prices: yahoo, fx: bank_of_canada }   # prices: yahoo | stooq | off · fx: bank_of_canada | off
```

`WORTHWATCH_OFFLINE=1` never touches the network; the app then runs on typed and previously fetched data and says so. Symbols use Yahoo spelling (`XEQT.TO`, `VTI`); the Stooq provider maps them. Bank of Canada rates are CAD-based, so with another base currency set `fx: off` and type rates.

## Security

Worthwatch holds your finances and, for now, has **no login**. Anyone who can reach the port sees everything.

- **Default: localhost only.** The binary binds `127.0.0.1:7777` and the Docker compose file maps the port to `127.0.0.1` on the host, so nothing on your Wi-Fi can reach it.
- **Opting into your network.** Binary: `WORTHWATCH_BIND=0.0.0.0:7777`. Docker: `WORTHWATCH_HOST=0.0.0.0`. Only do this on a network you trust; every device on it can then open the dashboard.
- **Remote access.** Use Tailscale, WireGuard or another VPN and keep the bind on localhost or the VPN interface. Do not port-forward it from your router.
- **Your data** lives in one folder (`WORTHWATCH_DATA`, `/data` in Docker) as plain YAML. Back it up like any other document; nothing leaves your machine.
- Password auth and a display-only token are planned for a later stage.

## Layout

```
crates/ledger     your data: schema, validation, Mutation + apply, atomic YAML write
crates/derive     pure functions: valuation, net worth series, portfolio, market merge
crates/providers  Yahoo / Stooq / Bank of Canada behind traits, fixture-tested parsers, quotes.json
crates/importers  pasted broker holdings -> mutations
server/         the binary: axum API + embedded UI
ui/             Svelte 5 + Vite + TypeScript
deploy/         Dockerfile, compose
data/             ledger.yaml and quotes.json (yours, gitignored), ledger.example.yaml
```

## Tests

```sh
cargo test --workspace            # validator rules, derive invariants, provider parsers on fixtures, paste parser, server state
pnpm --dir ui test                # vitest: parsing, formatting, check-in diffing, market status line
pnpm --dir ui build && cargo build && pnpm --dir ui e2e   # Playwright smoke flow against the real binary
```
