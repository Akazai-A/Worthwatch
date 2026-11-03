# worthwatch — conventions for AI-assisted work

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
  or ASCII diagram. Screenshots live under `docs/screenshots/`.

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
- `crates/assistant` — chat: shorthand parser, `Llm` trait (Ollama, Anthropic, Manual, Off), and
  `resolve`, which checks a model's `Proposal` against the ledger into a `Plan`. Proposes only.
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
`WORTHWATCH_OFFLINE=1` (never fetch, never ask a model; set in the Playwright config),
`ANTHROPIC_API_KEY` (cloud chat opt-in only), `WORTHWATCH_URL` (where `worthwatch mcp` finds the app).
