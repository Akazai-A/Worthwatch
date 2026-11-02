# worthwatch — conventions for AI-assisted work

## Models
- All UI work (anything under `ui/`: Svelte components, CSS, TypeScript, UI tests)
  must be done with **Claude Fable 5.1** (`claude-fable-5-1`). Switch to that model
  before touching `ui/`. This is a hard requirement from the owner.
- Rust work (`crates/`, `server/`) has no model requirement.

## Layout
- `crates/ledger` — plain-text ledger schema, IO, validation, mutations.
- `crates/derive` — derived figures (net worth, allocations) from the ledger.
- `server` — Axum binary `worthwatch`, embeds the built UI from `ui/dist`.
- `ui` — Svelte 5 + Vite. `pnpm --dir ui build` before `cargo build --release`.
- `deploy/ci.yml` belongs at `.github/workflows/ci.yml` (moved by hand; remote file
  tools cannot write into `.github/`).

## Checks before any PR
```
cargo fmt --check && cargo clippy --workspace -- -D warnings && cargo test --workspace
pnpm --dir ui build && pnpm --dir ui test
```

## Env vars
`WORTHWATCH_DATA` (data dir, default `data`), `WORTHWATCH_BIND` (default `127.0.0.1:7777`).
