# Architecture (Stage 1)

Read the design doc for the full picture. The rules that matter in code:

1. **One write path.** Every change is a `ledger::Mutation` applied by `Ledger::apply`, which returns a new `Ledger`. `server::state::AppState::apply_many` is the only thing that writes `ledger.yaml`, and it writes atomically (temp file + rename).
2. **Core is pure.** `crates/ledger` and `crates/derive` do no IO except `ledger::io`, and never read the clock. Dates come in as arguments.
3. **Exact numbers.** `rust_decimal::Decimal` for money, `chrono::NaiveDate` for dates. No `f64` outside chart payloads.
4. **One view per screen.** `server::routes::LedgerView` is assembled server-side; the UI renders it and never recomputes totals.
5. **The file is the truth, even when broken.** `server::state` polls `ledger.yaml` (mtime + size, 4×/s; polling so Docker bind mounts work) and reloads it. A file that fails to parse or validate leaves the last good `Ledger` in memory, sets a `Diagnostic` (line number when the YAML parser gave one), and makes `apply` refuse with 409 until the file is fixed, so a hand edit is never overwritten. Changes fan out to the UI over `/api/events` (SSE) and the UI re-fetches `/api/view`.
6. **Dependency direction.** `ui -> server -> derive -> ledger`. Nothing below `server` knows about HTTP.

## Stage 1 data flow

```mermaid
flowchart LR
    subgraph disk["data dir (WORTHWATCH_DATA)"]
        Y[(ledger.yaml)]
    end
    subgraph server["worthwatch (Axum)"]
        W[watcher thread<br/>hash file 4x/s]
        S[AppState<br/>last good Ledger + Diagnostic + version]
        A[/POST /api/apply/]
        V[/GET /api/view/]
        E[/GET /api/events SSE/]
        D[derive: totals, series,<br/>history, deltas]
    end
    subgraph ui["browser (Svelte 5)"]
        VS[view store]
        H[Home]
        C[Check-in]
        AD[Account]
    end
    Editor([you, in a text editor]) -- save --> Y
    Y -- read + validate --> W
    W -- ok: swap ledger / err: keep last good, set Diagnostic --> S
    S -- version++ --> E
    E -- change event --> VS
    VS -- refetch --> V
    V -- LedgerView --> VS
    S --> D --> V
    VS --> H & C & AD
    C -- Vec<Mutation> --> A
    A -- Ledger::apply + validate --> S
    A -- atomic write (tmp + rename) --> Y
    A -. 409 while Diagnostic set .-> C
```

Reading it: the file is the only store. The UI never writes the file; it sends mutations, the server validates and writes atomically, the watcher sees the new bytes, and every open tab refreshes through the event stream. A hand edit takes the same path from the watcher onward. A broken file parks a `Diagnostic` on the state, which the view carries to the banner and which makes `apply` refuse.

## Server routes

| Route | What |
|---|---|
| `GET /api/view` | `LedgerView`: totals, series (with per-group values), deltas, every account with history and holdings, the current `problem` if any |
| `POST /api/apply` | `Vec<Mutation>` → new `LedgerView`; 422 on a validation error, 409 while the file on disk is broken |
| `GET /api/events` | SSE, one `change` event per ledger version |
| `worthwatch seed [--force]` | writes a 14-month sample ledger to `WORTHWATCH_DATA` |

## UI

Hash routes `#/`, `#/check-in`, `#/accounts/<id>`. Global state is three stores under `ui/src/lib/stores`: `view` (the `LedgerView` plus the SSE subscription), `route`, `prefs` (blur, theme). Screens under `ui/src/screens` render the view; the only browser-side arithmetic is the live preview on Check-in (`lib/checkin.ts`, unit-tested), and the saved numbers always come back from the server. Charts live in `lib/charts` (uPlot for time series). Colours are CSS tokens in `tokens.css`; components never use colour literals.

Playwright smoke flow: `ui/e2e/smoke.spec.ts` runs the real binary on a temp data dir.

Planned crates (not yet created): `portfolio`, `providers`, `store`, `importers`, `assistant`, `mcp`.
