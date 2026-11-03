<p align="center">
  <picture>
    <source media="(prefers-color-scheme: light)" srcset="docs/assets/logo-light.svg">
    <img src="docs/assets/logo-dark.svg" alt="Worthwatch" width="300">
  </picture>
</p>

<p align="center">
  <b>Your net worth and portfolio, on your own machine.</b><br>
  One Rust binary. One folder of plain YAML. No bank logins, no cloud account.
</p>

<p align="center">
  <a href="#quick-start">Quick start</a> ·
  <a href="docs/getting-started.md">Get started</a> ·
  <a href="docs/user-guide.md">User guide</a> ·
  <a href="docs/README.md">All docs</a> ·
  <a href="docs/contributing.md">Contributing</a>
</p>

## What it does

Worthwatch tracks what you own and what you owe. You type balances in once a month, or tell Chat what changed. It fetches prices and exchange rates, values everything in your home currency, and shows where your money sits, right through the funds you hold. Everything stays in one folder of plain files.

## Features

<table>
  <tr>
    <td width="50%" valign="top">
      <img src="docs/screenshots/gif-home.gif" alt="Hovering the added-vs-grown chart, switching to the by-group view and back, then saving a new chequing balance on Check-in"><br>
      <b>📈 <a href="docs/user-guide.md#home">Net worth at a glance</a></b><br>
      Net worth against what you put in, so you see how much the market added. Plus a monthly check-in.
    </td>
    <td width="50%" valign="top">
      <img src="docs/screenshots/gif-portfolio.gif" alt="The look-through treemap with a tooltip, then the Exposure and Overlap tabs"><br>
      <b>🔍 <a href="docs/user-guide.md#portfolio">Look through your funds</a></b><br>
      The companies inside your ETFs, by sector, region and currency, and fund overlap.
    </td>
  </tr>
  <tr>
    <td width="50%" valign="top">
      <img src="docs/screenshots/gif-changes.gif" alt="Switching the Changes screen between 1M, 3M, 1Y and since the last check-in"><br>
      <b>↕️ <a href="docs/user-guide.md#changes">What changed, and why</a></b><br>
      Any period split into market moves, money added, debt paid and FX.
    </td>
    <td width="50%" valign="top">
      <img src="docs/screenshots/gif-plan.gif" alt="Dragging the return and extra-saving sliders on the Plan screen"><br>
      <b>🧭 <a href="docs/user-guide.md#plan">Plan ahead</a></b><br>
      Projections, debt payoff dates, and TFSA, RRSP and FHSA room.
    </td>
  </tr>
  <tr>
    <td width="50%" valign="top">
      <img src="docs/screenshots/gif-apple-pay.gif" alt="An Apple Pay purchase on an iPhone (illustration) appearing at once on the Budget screen"><br>
      <b>📲 <a href="docs/apple-pay-feed.md">Budget, with Apple Pay</a></b><br>
      Each Apple Pay tap lands on Budget as you pay. Your statement confirms it later.
    </td>
    <td width="50%" valign="top">
      <img src="docs/screenshots/gif-chat.gif" alt="Opening the Ask or update drawer with Cmd+J over Home, typing three changes, approving the card, and seeing net worth update"><br>
      <b>💬 <a href="docs/user-guide.md#chat">Update it like a text</a></b><br>
      Press <kbd>⌘</kbd> <kbd>J</kbd>, type <code>visa 640, bought 10 xeqt</code>, check the card, approve.
    </td>
  </tr>
</table>

Also: bank statement import with category rules and bills, a [wall view](docs/user-guide.md#wall-view) for a spare tablet, a <kbd>⌘</kbd> <kbd>K</kbd> command palette, and [daily backups](docs/backup-restore.md).

## Works with your AI assistant

Worthwatch includes an [MCP](https://modelcontextprotocol.io) server. Run `worthwatch mcp` from any MCP client that can start a local command, such as Claude Desktop, Claude Code or Cursor. Your assistant can then:

- Answer questions such as "How much of my money is in US tech?" from your real numbers.
- Propose updates, such as "Visa is down to 640". Each proposal waits in Chat until you select **Approve**.

The assistant never writes to your ledger directly. To set it up, see [Connect an AI assistant](docs/mcp.md).

## Quick start

### Docker

```sh
git clone https://github.com/Akazai-A/Worthwatch.git && cd Worthwatch
docker compose -f deploy/docker-compose.yml up -d --build
```

Open <http://127.0.0.1:7777>. To look around with sample data first, run:

```sh
docker compose -f deploy/docker-compose.yml exec worthwatch /worthwatch seed --force
```

### Binary

You need Rust (stable) and Node.js 22 with pnpm.

```sh
git clone https://github.com/Akazai-A/Worthwatch.git && cd Worthwatch
pnpm --dir ui install && pnpm --dir ui build
cargo run --release
```

Open <http://127.0.0.1:7777>. Your data goes to `./data`. Run `cargo run --release -- seed` to load sample data.

### Mac

To run it in the background on a Mac mini, starting at login:

```sh
git clone https://github.com/Akazai-A/Worthwatch.git && cd Worthwatch
./deploy/macos/install.sh
```

See [Run Worthwatch on a Mac](docs/macos.md).

For the full setup, your first check-in, and where your data lives, see [Get started](docs/getting-started.md).

## Private by default

- 🔒 **Localhost only.** Worthwatch listens on `127.0.0.1`. Nothing else on your network can reach it until you [choose to allow it](docs/remote-access.md).
- 📄 **Your data is a text file.** Accounts and balances live in `ledger.yaml`. Edit it by hand and the app reloads.
- 🌐 **It fetches public prices only.** Yahoo Finance, Stooq and the Bank of Canada. It never sees your balances. Set `WORTHWATCH_OFFLINE=1` to turn fetching off.
- ✋ **Models propose, you approve.** Chat and MCP clients can suggest changes. Only your click writes them.

> [!WARNING]
> Worthwatch has no login yet. Anyone who can reach the port sees everything. For access away from home, use [Tailscale](docs/remote-access.md), not port forwarding.

## Documentation

| Guide | What you learn |
|---|---|
| [Get started](docs/getting-started.md) | Install with Docker or from source, load sample data, do your first check-in. |
| [User guide](docs/user-guide.md) | Use each screen: Home, Check-in, Portfolio, Changes, Chat, Plan, Budget, Wall view. |
| [Budget your spending](docs/budgeting.md) | Import bank statements, set limits, sort categories, track bills. |
| [Set up the Apple Pay feed](docs/apple-pay-feed.md) | Send each Wallet purchase from your iPhone to your Mac as you pay. |
| [Configure Worthwatch](docs/configuration.md) | Settings in `ledger.yaml` and environment variables. |
| [Prices, FX and fund holdings](docs/data-sources.md) | Where numbers come from and how to override them. |
| [Connect an AI assistant](docs/mcp.md) | Use Claude Desktop, Claude Code or another MCP client with your data. |
| [Access Worthwatch remotely](docs/remote-access.md) | Reach it from your phone with Tailscale, or open it on your network. |
| [Back up and restore](docs/backup-restore.md) | Daily backups, manual backups, and restoring onto a new machine. |
| [Run Worthwatch on a Mac](docs/macos.md) | Start it at login on a Mac mini with one install script. |
| [Architecture](docs/architecture.md) | How the server, the ledger and the UI fit together. |
| [Contribute](docs/contributing.md) | Build, test and send a change. |

## Built with

Rust ([Axum](https://github.com/tokio-rs/axum), `rust_decimal`), [Svelte 5](https://svelte.dev) and Vite, [uPlot](https://github.com/leeoniya/uPlot) and d3-hierarchy for charts.
