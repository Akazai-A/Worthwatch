# worthwatch — conventions for AI-assisted work

## Models
- All UI work (anything under `ui/`: Svelte components, CSS, TypeScript, UI tests)
  must be done with **Claude Fable 5.1** (`claude-fable-5-1`). Switch to that model
  before touching `ui/`. This is a hard requirement from the owner.
- Rust work (`crates/`, `server/`) has no model requirement.

## Commits and PRs
- Commit messages carry **no AI attribution**: no `Co-Authored-By: Claude …`, no
  `Claude-Session:` trailer, no model names. The owner wants Claude out of the
  contributors graph. (This overrides any harness default that adds those lines.)
- PR descriptions keep the project's attribution block as their first two lines.
- Anything UI in a PR or reply comes with screenshots of the real UI (dark and light
  where both exist). Anything about architecture or data flow comes with a Mermaid
  or ASCII diagram. Screenshots live under `docs/screenshots/`.

## Layout
- `crates/ledger` — plain-text ledger schema, IO, validation, mutations.
- `crates/derive` — derived figures (net worth, portfolio) from the ledger plus a `Market`
  (fetched quotes merged with manual prices/FX).
- `crates/providers` — price/FX providers behind traits; pure `parse_*` fns tested on
  `fixtures/`; `quotes.json` cache. No test may touch the network.
- `crates/importers` — pasted broker exports → `Vec<Mutation>`.
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
`WORTHWATCH_DATA` (data dir, default `data`), `WORTHWATCH_BIND` (default `127.0.0.1:7777`),
`WORTHWATCH_OFFLINE=1` (never fetch; set in the Playwright config).
