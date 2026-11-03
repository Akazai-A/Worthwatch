# Contribute

This page shows you how to set up a development environment, run the checks, and send a change.

## Set up your environment

You need:

- Rust stable with `rustfmt` and `clippy`. The repository pins the toolchain in `rust-toolchain.toml`.
- Node.js 22 and pnpm 10.

Install the UI dependencies:

```sh
pnpm --dir ui install
```

## Run in development

Run the server and the UI dev server in two terminals:

```sh
# Terminal 1: the API server. Creates data/ledger.yaml on first run.
cargo run

# Terminal 2: the UI with hot reload. Proxies /api to the server.
pnpm --dir ui dev
```

Open <http://localhost:5173>. Run `cargo run -- seed` to load sample data.

To stop all network access while you work, set `WORTHWATCH_OFFLINE=1`.

## Run the checks

Run these before you open a pull request. CI runs the same ones.

```sh
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
pnpm --dir ui check
pnpm --dir ui build
pnpm --dir ui test
```

The Playwright smoke test runs the real binary against a temporary data folder:

```sh
pnpm --dir ui build && cargo build && pnpm --dir ui e2e
```

| Suite | What it covers |
|---|---|
| `cargo test` | Ledger validation, derive invariants, provider parsers on fixtures, paste parser, chat proposals, backups, server state. |
| `pnpm --dir ui test` | Vitest: parsing, formatting, check-in diffing, the market line, plan layout, palette matching. |
| `pnpm --dir ui e2e` | First run, check-in, hand edits, paste, look-through, Changes, Chat, Plan and backups in a real browser. |

No test may touch the network. Provider parsers are pure `parse_*` functions tested on files under `fixtures/`.

## Follow the code style

- **Data-oriented Rust.** Use plain structs and pure functions. Change the ledger only through `Mutation` and `apply`.
- **Thin Svelte components.** Keep state in stores under `ui/src/lib/stores`. Put logic in tested modules under `ui/src/lib`.
- **Colours come from tokens.** Use the CSS variables in `ui/src/tokens.css`. Don't write colour literals in components.
- **Show, then tell.** Prefer charts, bars and sparklines to blocks of text. Put the exact words in hover titles.

Read [Architecture](architecture.md) before you change how data moves.

## Send a pull request

1. Create a branch that starts with `f/`, such as `f/stage-8-auth`.
2. Make your change and run the checks.
3. Open a pull request. Include:
   - Screenshots of the real UI for any UI change, in dark and light themes. Save them under `docs/screenshots/<screen>/`, next to the other shots of that screen.
   - A Mermaid or ASCII diagram for any change to architecture or data flow.

## Set up CI

The CI workflow lives at `.github/workflows/ci.yml`. Forks run it as soon as you enable Actions.

It runs three jobs on every push and pull request:

| Job | What it checks |
|---|---|
| `build-and-test` | Formatting, lints, unit tests, the UI build, and the end-to-end smoke flow. |
| `audit` | Crates and npm packages against the advisory databases. |
| `docker` | That `deploy/Dockerfile` still builds, so a base-image update that breaks it fails its pull request. |

## Write docs

Docs follow the [Google developer documentation style guide](https://developers.google.com/style):

- Address the reader as "you", in the present tense.
- Use short sentences, one idea per paragraph.
- Start task headings with a verb, such as "Restore a backup".
- Use numbered steps for procedures and tables for reference.
