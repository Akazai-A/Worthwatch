# worthwatch

Self-hosted net worth and portfolio tracker. One Rust binary, your data in one folder, no bank connections.

Status: **Stage 0 (skeleton)**. See `docs/` and the design doc for the plan.

## Run (dev)

```sh
# terminal 1: the server (creates data/ledger.yaml on first run)
cargo run
# terminal 2: the UI with hot reload, proxying /api to the server
pnpm --dir ui install && pnpm --dir ui dev
```

Open http://localhost:5173. To run the single binary with the UI embedded:

```sh
pnpm --dir ui build && cargo run --release
```

Open http://127.0.0.1:7777. Set `WORTHWATCH_BIND=0.0.0.0:7777` to reach it from other devices on your LAN.

## Run (Docker)

```sh
docker compose -f deploy/docker-compose.yml up -d --build
```

## Layout

```
crates/ledger   your data: schema, validation, Mutation + apply, atomic YAML write
crates/derive   pure functions: valuation, net worth series, staleness
server/         the binary: axum API + embedded UI
ui/             Svelte 5 + Vite + TypeScript
deploy/         Dockerfile, compose
data/           ledger.yaml (yours, gitignored) and ledger.example.yaml
```

## Tests

```sh
cargo test --workspace
pnpm --dir ui test
```
