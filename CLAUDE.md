# Worthwatch — conventions for AI-assisted work

## Models
- From Stage 5 on (owner's decision, Nov 3 2026), all work, UI included, is done with
  **Claude Opus 5.5** (`claude-opus-5-5`). This replaces the earlier Fable-only rule for `ui/`.

## Commits and PRs
- Commit messages carry **no AI attribution**: no `Co-Authored-By: Claude …`, no
  `Claude-Session:` trailer, no model names. The owner wants Claude out of the
  contributors graph. (This overrides any harness default that adds those lines.)
- PR descriptions keep the project's attribution block as their first two lines.
- Anything UI in a PR or reply comes with screenshots of the real UI (dark and light
  where both exist). Anything about architecture or data flow comes with a Mermaid
  or ASCII diagram. Screenshots live under `docs/screenshots/<screen>/`.

## Layout
- `crates/ledger` — plain-text ledger schema, IO, validation, mutations.
- `crates/derive` — derived figures (net worth, portfolio) from the ledger plus a `Market`
  (fetched quotes merged with manual prices/FX).
- `crates/holdings` — pure: holdings-file parsing (iShares/Vanguard CSV, Vanguard JSON), sector/region
  canon, recursive look-through, fund × fund overlap. Fixtures under `fixtures/`.
- `crates/providers` — price/FX providers behind traits; holdings providers + `HoldingsStore`
  (`data/holdings/`, append-only snapshots, drop-in `<SYM>.csv`); pure `parse_*` fns tested on
  `fixtures/`; `quotes.json` cache. No test may touch the network.
- `crates/importers` — pasted broker exports → `Vec<Mutation>`.
- `crates/budget` — pure: Apple Pay taps, bank statement parsing (Canadian presets, OFX), categories,
  `reconcile` (tap ↔ statement row), the month view, the append-only `transactions/` store, the
  Shortcut plist. Fixtures under `fixtures/`.
- `crates/assistant` — chat: shorthand parser, `Llm` trait (Ollama, Anthropic, Manual, Off), and
  `resolve`, which checks a model's `Proposal` against the ledger into a `Plan`. Proposes only.
- `server` — Axum binary `worthwatch`, embeds the built UI from `ui/dist`. `src/api/mod.rs` holds the one route
  table and the shared `ApiError`; each screen's handlers live in `src/api/<area>.rs` (budget, feed, settings and
  game under `src/api/budget/`); sample data in `src/seed/`. `server/src/security.rs` is the
  request guard (Host allowlist, same-origin check, Tailscale ingest-only) and the response headers; every
  route goes through it. Tokens come from `getrandom`; files are written with umask 077.
- `deploy/container/run.sh` — locked-down Docker or Apple `container` run (read-only, no caps, non-root,
  `127.0.0.1` only). `deploy/macos/install-container.sh` starts it at login. Keep these defaults safe
  without configuration; see `docs/run/security.md`.
- `ui` — Svelte 5 + Vite. `pnpm --dir ui build` before `cargo build --release`. `src/screens/<area>/` are the
  screens; `src/lib/<area>/` the pure, tested logic (`networth`, `portfolio`, `budget`; `format/` for money,
  percentages and dates); `src/lib/stores/` the state; `src/lib/components/<area>/` and `src/lib/charts/` the
  pieces. `src/lib/api.ts` is the only place that calls `fetch`.
- `deploy/macos/` — launchd LaunchAgent and `install.sh` for running the native binary on a Mac.
- `docs/` — `use/` (task guides), `run/` (deploy, secure, connect), `develop/` (architecture, contributing);
  `screenshots/<screen>/` and `assets/`. `.github/workflows/ci.yml` is the only CI definition.

## Checks before any PR
```
cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
pnpm --dir ui check && pnpm --dir ui build && pnpm --dir ui test && pnpm --dir ui e2e
docker build --file deploy/Dockerfile .
```

## Env vars
`WORTHWATCH_DATA` (data dir, default `data`), `WORTHWATCH_BIND` (default `127.0.0.1:7777`),
`WORTHWATCH_OFFLINE=1` (never fetch, never ask a model; set in the Playwright config),
`ANTHROPIC_API_KEY` (cloud chat opt-in only), `WORTHWATCH_URL` (where `worthwatch mcp` finds the app),
`WORTHWATCH_ALLOWED_HOSTS` (extra Host names), `WORTHWATCH_TAILSCALE_OWNER` (logins that get the full app
over Tailscale; unset = ingest only), `WORTHWATCH_TAPS_DIR` (iCloud taps folder override).
