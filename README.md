# worthwatch

Self-hosted net worth and portfolio tracker. One Rust binary, your data in one folder, no bank connections.

Status: **Stage 5**: hand-edited or typed-in balances, monthly check-in, net worth dashboard, positions valued from fetched prices and Bank of Canada FX, ETF look-through and exposure, a Changes screen, and a Chat panel plus an MCP server that propose updates you approve. See `docs/` and the design doc for the plan.

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

## Chat and MCP

Chat (left rail) takes what you would text: `td chq 5102, visa down to 640, sold 20 XEQT in the TFSA`, `visa down 200`, `eq savings 18.5k`, or a question like `what is my net worth?`. Every message becomes a card listing each change (now, proposed, delta, effect on net worth) with Approve, Edit and Cancel. Nothing is written until you approve, and anything that does not match your accounts is shown as rejected with the reason.

Who reads the message is set in the ledger:

```yaml
settings:
  assistant: { provider: ollama }   # ollama (default, local) | cloud | shorthand (no model)
  # ollama_url: http://127.0.0.1:11434, model: llama3.1
```

- **ollama**: a local model; nothing leaves the machine. If Ollama is not running, the built-in parser answers and the card says so.
- **cloud**: Anthropic's API, opt-in. Set `ANTHROPIC_API_KEY` in the server's environment. Each message first shows the exact text that would be sent (account ids, names, kinds, currencies and symbols; never balances) and is sent only when you press Send.
- **shorthand**: no model at all.

**MCP (Claude Desktop).** `worthwatch mcp` is an MCP server on stdio that talks to the running app. Add it to Claude Desktop's config:

```json
{ "mcpServers": { "worthwatch": { "command": "/path/to/worthwatch", "args": ["mcp"], "env": { "WORTHWATCH_URL": "http://127.0.0.1:7777" } } } }
```

With Docker, the command is `docker` with args `["compose", "-f", "/path/to/worthwatch/deploy/docker-compose.yml", "exec", "-T", "worthwatch", "/worthwatch", "mcp"]`, and `docker compose --profile ai up -d` starts a local Ollama the app already points at (pull a model once with `docker compose exec ollama ollama pull llama3.1`). Tools: `get_net_worth`, `get_accounts`, `get_portfolio`, `get_exposure`, `propose_update`, `check_proposal`. A proposal from Claude Desktop appears in Chat as a pending card (the rail shows a count) and waits for your Approve or Discard.

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
crates/assistant  chat: shorthand parser, Ollama / Anthropic behind a trait, proposals checked against the ledger
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
