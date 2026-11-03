# Get started

This page shows you how to install Worthwatch, look around with sample data, and record your first balances. It takes about ten minutes.

## Before you begin

Choose how you want to run Worthwatch:

- **Docker** needs Docker with Compose. This is the simplest option for a home server or NAS.
- **On a Mac** that stays on, use the [Mac install script](macos.md). It needs the same tools as building from source.
- **From source** needs Rust (stable, installed with [rustup](https://rustup.rs)) and Node.js 22 with [pnpm](https://pnpm.io). Use this on a laptop or if you want to change the code.

All three run the same single binary. The web UI is built into it.

## Install with Docker

1. Clone the repository:

   ```sh
   git clone https://github.com/Akazai-A/worthwatch.git
   cd worthwatch
   ```

2. Build and start the container:

   ```sh
   docker compose -f deploy/docker-compose.yml up -d --build
   ```

3. Open <http://127.0.0.1:7777>.

Your data lives in the `worthwatch-data` Docker volume, mounted at `/data` in the container.

## Install from source

1. Clone the repository:

   ```sh
   git clone https://github.com/Akazai-A/worthwatch.git
   cd worthwatch
   ```

2. Build the UI. The server embeds it at compile time, so build it first:

   ```sh
   pnpm --dir ui install
   pnpm --dir ui build
   ```

3. Build and start the server:

   ```sh
   cargo run --release
   ```

4. Open <http://127.0.0.1:7777>.

Your data lives in `./data`. To keep it somewhere else, set `WORTHWATCH_DATA`:

```sh
WORTHWATCH_DATA=~/worthwatch-data cargo run --release
```

The binary is at `target/release/worthwatch`. You can copy it anywhere and run it on its own.

## Look around with sample data

Load a 14-month sample ledger with prices, fund holdings and plan rules:

- Docker:

  ```sh
  docker compose -f deploy/docker-compose.yml exec worthwatch /worthwatch seed --force
  ```

- From source:

  ```sh
  cargo run --release -- seed --force
  ```

> [!CAUTION]
> `--force` replaces the ledger in your data folder. Use it on a fresh install only, or point `WORTHWATCH_DATA` at a separate folder.

The open page reloads by itself when the file changes.

## Record your first balances

On first run, Worthwatch creates an empty `ledger.yaml` and asks you to add an account.

1. On **Home**, select **Go to Check-in**.
2. Select **+ Add account**. Enter a name, such as `TD Chequing`, and the institution.
3. Select **Add account**.
4. Type the account's current balance in the **new value** field.
5. Repeat for each account. Enter debts, such as a credit card, as the positive amount you owe.
6. Select **Save check-in**.

Home now shows your net worth. Do a check-in once a month. The Changes screen compares any two check-ins.

For investment accounts, select **Paste holdings** on Check-in and paste the holdings table from your broker's website. Worthwatch reads the symbols and units, shows you what it will change, and saves after you apply. See [User guide](user-guide.md#check-in).

## Edit the ledger by hand

Everything you enter goes to `ledger.yaml` in your data folder. You can open it in any text editor. When you save, the app reloads.

If the file has a mistake, the app keeps showing the last good version, shows the line with the problem, and refuses to save until you fix it. Your edit is never overwritten.

For an annotated example, see [`data/ledger.example.yaml`](../data/ledger.example.yaml).

## What's next

- Learn each screen in the [User guide](user-guide.md).
- [Budget your spending](budgeting.md) from your bank statements and Apple Pay.
- [Connect Claude Desktop](mcp.md) to ask questions about your portfolio.
- [Access Worthwatch from your phone](remote-access.md).
- Set up [backups](backup-restore.md) off the machine.
