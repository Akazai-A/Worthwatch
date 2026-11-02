# Architecture (Stage 2)

Read the design doc for the full picture. The rules that matter in code:

1. **One write path.** Every change is a `ledger::Mutation` applied by `Ledger::apply`, which returns a new `Ledger`. `server::state::AppState::apply_many` is the only thing that writes `ledger.yaml`, and it writes atomically (temp file + rename).
2. **Core is pure.** `crates/ledger` and `crates/derive` do no IO except `ledger::io`, and never read the clock. Dates come in as arguments.
3. **Exact numbers.** `rust_decimal::Decimal` for money, `chrono::NaiveDate` for dates. No `f64` outside chart payloads.
4. **One view per screen.** `server::routes::LedgerView` is assembled server-side; the UI renders it and never recomputes totals.
5. **The file is the truth, even when broken.** `server::state` polls `ledger.yaml` (mtime + size, 4×/s; polling so Docker bind mounts work) and reloads it. A file that fails to parse or validate leaves the last good `Ledger` in memory, sets a `Diagnostic` (line number when the YAML parser gave one), and makes `apply` refuse with 409 until the file is fixed, so a hand edit is never overwritten. Changes fan out to the UI over `/api/events` (SSE) and the UI re-fetches `/api/view`.
6. **Dependency direction.** `ui -> server -> derive -> ledger`, with `providers` and `importers` as siblings the server glues in (`importers -> ledger`; `providers` depends on neither). Nothing below `server` knows about HTTP.
7. **Two files, two kinds of data.** `ledger.yaml` holds what you would type: accounts, balances, positions, prices and FX rates you entered. `quotes.json` beside it holds what the app fetched. It is never hand-edited and can be deleted; the next refresh rebuilds it. `derive::Market` merges the two for valuation: a typed price wins over a fetched one for the same date, and a typed FX rate is the fallback when no fetched rate covers the date.
8. **Providers at the boundary only.** `PriceProvider` and `FxProvider` are the only traits; each has a `Manual` (fixed answers) and `Off` impl, and every real provider is a pure `parse_*` function over fixture-tested bytes plus a thin blocking HTTP shim. No test touches the network; `WORTHWATCH_OFFLINE=1` makes the binary never touch it either.
9. **Never silently zero.** A holding with no price, or an account in a currency with no rate, is flagged (`missing_price`, `missing_fx`, `unpriced`) and shown as "no price" / "no FX" in brass. Stale fetched prices keep their date on screen.

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

## Stage 2 data flow: prices, FX, pasted holdings

```mermaid
flowchart LR
    subgraph net["public market data (opt-out via settings.providers)"]
        YH[Yahoo Finance chart JSON]
        ST[Stooq CSV]
        BOC[Bank of Canada Valet JSON]
    end
    subgraph disk["data dir"]
        Y[(ledger.yaml<br/>typed: accounts, positions,<br/>manual prices, manual fx)]
        Q[(quotes.json<br/>fetched: daily closes,<br/>fx rates, errors)]
    end
    subgraph server["worthwatch (Axum)"]
        P[providers::Yahoo / Stooq<br/>parse_chart / parse_csv]
        F[providers::BankOfCanada<br/>parse_valet]
        C[QuoteCache::refresh<br/>backfill 1y on first sight,<br/>top up after]
        R[/POST /api/refresh/]
        T[daily tick: startup +<br/>every 30 min if not fetched today]
        M[derive::Market::merge<br/>manual wins on its date;<br/>manual fx = fallback]
        D[derive: value_account,<br/>series, portfolio]
        V[/GET /api/view<br/>market + portfolio sections/]
        PH[/POST /api/parse-holdings/]
        I[importers::parse_holdings<br/>+ reconcile]
        A[/POST /api/apply/]
    end
    subgraph ui["browser"]
        ML[MarketLine: prices as of,<br/>USD/CAD rate, Refresh]
        PF[Portfolio]
        PP[PastePanel on Check-in]
    end
    YH & ST --> P
    BOC --> F
    P & F --> C
    R --> C
    T --> C
    C -- atomic write --> Q
    Q & Y --> M --> D --> V --> ML & PF
    ML -- click --> R
    PP -- text + account + date --> PH --> I
    I -- Plan: rows, skipped, Vec<Mutation> --> PP
    PP -- Apply: Vec<Mutation> --> A -- trades, prices, book costs --> Y
```

Reading it: fetching only ever writes `quotes.json`; the ledger is still written only by `/api/apply`. A refresh bumps the version so open tabs re-fetch the view. Pasting is two round trips: `/api/parse-holdings` is pure (parse, resolve symbols against the account, diff against current units, propose mutations) and writes nothing; the user sees the preview and `Apply` sends the proposed mutations through the one write path.

## Server routes

| Route | What |
|---|---|
| `GET /api/view` | `LedgerView`: totals, series (with per-group values), deltas, every account with history and holdings, the current `problem` if any |
| `POST /api/apply` | `Vec<Mutation>` → new `LedgerView`; 422 on a validation error, 409 while the file on disk is broken |
| `POST /api/refresh` | fetch prices and FX for every held symbol and used currency now; always answers with the view, per-symbol failures in `market.errors` |
| `POST /api/parse-holdings` | `{ account, date, text }` → parsed rows, skipped lines, untouched positions and the proposed `mutations`; writes nothing |
| `GET /api/events` | SSE, one `change` event per ledger version |
| `worthwatch seed [--force]` | writes a 14-month sample ledger and a `quotes.json` with three months of daily closes to `WORTHWATCH_DATA` |

## UI

Hash routes `#/`, `#/check-in`, `#/portfolio`, `#/accounts/<id>`. Global state is three stores under `ui/src/lib/stores`: `view` (the `LedgerView` plus the SSE subscription), `route`, `prefs` (blur, theme). Screens under `ui/src/screens` render the view; the only browser-side arithmetic is the live preview on Check-in (`lib/checkin.ts`, unit-tested), and the saved numbers always come back from the server. Charts live in `lib/charts` (uPlot for time series). Colours are CSS tokens in `tokens.css`; components never use colour literals.

Playwright smoke flow: `ui/e2e/smoke.spec.ts` runs the real binary on a temp data dir.

`lib/market.ts` (tested) words the freshness line; the paste preview and the portfolio numbers come from the server.

Crates: `ledger`, `derive`, `providers`, `importers`. Planned: `store` (SQLite, when holdings snapshots arrive), `assistant`, `mcp`.
