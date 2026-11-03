# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Users

One person: the owner, Zai, in Canada. Worthwatch is a personal tool, not a multi-user product. Zai uses it to:

- Check net worth and how it is moving, usually at a desk on the Mac or in a browser on the LAN.
- Record balances once a month (Check-in), or in passing through Chat ("td chq 6000, visa 640").
- Understand the portfolio: which companies the ETFs actually hold, sector, region and currency exposure, and how much funds overlap.
- Glance at the phone while away, over Tailscale.
- Leave a full-screen Wall view running on a spare tablet.

The owner is fluent with finance terms (TFSA, RRSP, FHSA, look-through, FX, ETF tickers) and with keyboard tools (⌘K palette, ⌘⇧H to blur amounts).

## Product Purpose

Worthwatch tracks what one person owns and owes, values it in CAD from public prices and exchange rates, and shows where the money actually sits, down through the funds it holds. Success means the owner can answer three questions in a few seconds: what am I worth, is it going up, and why did it change.

## Positioning

- **Self-hosted and private.** One Rust binary with one folder of plain YAML (`ledger.yaml`). It has no bank logins and no cloud account, and it listens on localhost by default.
- **Look-through by default.** Funds are opened up to three levels deep, so exposure and overlap reflect the companies the owner actually holds, not just the tickers.
- **Explains change, not just totals.** The Changes bridge splits a period into market moves, money added, debt paid down and FX.
- **Models propose, the owner approves.** Chat and MCP clients can only suggest changes. Only a click on Approve writes them.

## Operating Context

- **Runtime:** the native binary under launchd on a Mac mini at `127.0.0.1:7777`. Docker is also supported. Remote access goes through Tailscale. LAN exposure is opt-in config.
- **Rituals:** a monthly Check-in; Apple Pay taps arriving through an iPhone Shortcut; bank statement imports (CSV/OFX from Canadian banks); daily backups.
- **Data sources:** Yahoo Finance with Stooq as fallback for prices, Bank of Canada for FX, and issuer holdings files for look-through.
- **Surfaces:**
  - Primary: Home, Check-in, Portfolio (Positions, Look-through, Exposure, Overlap), Changes, Plan, Chat, Account detail.
  - Secondary: Budget, Settings, Backups.
  - Ambient: Wall view.
  - Global: ⌘K palette.

## Capabilities and Constraints

- **Priority order:** net worth and portfolio visualisation first. Budgeting is secondary and deliberately basic.
- **Base currency:** CAD. Holdings and accounts may be in USD and are converted at Bank of Canada rates.
- **Stack:** Svelte 5 with Vite in `ui/`, embedded in the Rust server (`server`). Charts use uPlot and d3-hierarchy. No component library.
- **Login:** none yet. Anyone who can reach the port sees everything, so remote access must go through Tailscale.
- **Offline:** `WORTHWATCH_OFFLINE=1` stops all fetching and all model calls.
- **Privacy:** ⌘⇧H blurs every amount, for screen sharing or a public place.
- **Themes:** dark and light are both first-class. Light follows the OS unless the owner sets a theme.
- **Terminology:**
  - check-in: the monthly balance entry
  - look-through
  - exposure
  - overlap
  - bridge: the Changes waterfall
  - tap: an Apple Pay purchase not yet on a statement
  - stale: an account not updated recently
- **Undecided:** authentication, and any multi-user mode.

## Brand Commitments

- **Name:** "Worthwatch" with a capital W wherever people read it (UI, README, docs). Lowercase `worthwatch` only for identifiers: binary, crate, package, launchd label, paths, env vars.
- **Logo:** `docs/assets/logo-light.svg` and `docs/assets/logo-dark.svg`.
- **Voice:** plain, short, second person, present tense. No hype. Docs follow Google developer documentation style.
- **Feel:** a calm, private, personal instrument, not a SaaS dashboard. The owner wants the UI to stop reading as generic AI-generated design and to have its own character.
- **Information design:** visual encodings over blocks of text wherever they reduce load without losing detail. That means charts, sparklines, delta bars, colour-coded dots and small multiples, with detail on hover or expand.

## Evidence on Hand

- **Sample data:** `worthwatch seed --force` generates a realistic but fictional ledger, holdings and a month of budget data. Use it for screenshots and demos, never real balances.
- **Screenshots:** the current UI in dark and light, in `docs/screenshots/` (hero, home, portfolio, look-through, exposure, overlap, changes, plan, chat, budget, wall, palette, settings, mobile variants).
- **Docs:** `README.md`, `docs/user-guide.md`, `docs/architecture.md` and the rest of `docs/`.
- **Absent:** no testimonials, users, press or benchmarks. Do not invent any.

## Product Principles

1. **Answer first, explain on demand.** The headline figure and its direction come before any table. Detail lives behind hover, expand or a click into the account.
2. **Show it, don't tell it.** Prefer an encoding (a line, a bar, a dot, a tile) to a sentence whenever it carries the same meaning.
3. **A colour means one thing everywhere.** Asset-group colours are fixed across every chart and screen.
4. **Nothing changes without the owner's say-so.** Every suggestion from a model or an import is reviewable, and Approve is explicit.
5. **Private and plain-text by construction.** Never imply a cloud, an account, or a sync service that does not exist.

## Accessibility & Inclusion

No product-specific requirement has been stated. Work to WCAG 2.2 AA for contrast, keyboard access and reduced motion. The existing `prefers-reduced-motion` rule in `ui/src/tokens.css` and the keyboard palette are commitments to keep.
