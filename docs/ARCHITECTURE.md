# Architecture (Stage 0)

Read the design doc for the full picture. The rules that matter in code:

1. **One write path.** Every change is a `ledger::Mutation` applied by `Ledger::apply`, which returns a new `Ledger`. `server::state::AppState::apply_many` is the only thing that writes `ledger.yaml`, and it writes atomically (temp file + rename).
2. **Core is pure.** `crates/ledger` and `crates/derive` do no IO except `ledger::io`, and never read the clock. Dates come in as arguments.
3. **Exact numbers.** `rust_decimal::Decimal` for money, `chrono::NaiveDate` for dates. No `f64` outside chart payloads.
4. **One view per screen.** `server::routes::LedgerView` is assembled server-side; the UI renders it and never recomputes totals.
5. **Dependency direction.** `ui -> server -> derive -> ledger`. Nothing below `server` knows about HTTP.

Planned crates (not yet created): `portfolio`, `providers`, `store`, `importers`, `assistant`, `mcp`.
