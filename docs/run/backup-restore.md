# Back up and restore

Worthwatch backs up your data every day. This page shows you how to make a backup on demand, copy backups off the machine, and restore one.

## What a backup contains

A backup is one `.tar.gz` file named `worthwatch-<date>-<time>.tar.gz`. It holds:

- `ledger.yaml`: your accounts, balances and settings.
- `quotes.json`: fetched prices and rates.
- `holdings/`: fund holdings files.
- `transactions/`: imported statement rows, Apple Pay taps, and categories you picked by hand.

Backups go to `backups/` in your data folder. Worthwatch keeps the newest 30, plus the newest one of each of the last 30 days, and deletes the rest. A burst of backups on one day never pushes out earlier days.

## Make a backup now

- **In the app:** select **Backups** in the sidebar, then select **Back up now**.
- **Binary:**

  ```sh
  WORTHWATCH_DATA=/path/to/data worthwatch backup
  ```

- **Docker:**

  ```sh
  docker compose -f deploy/docker-compose.yml exec worthwatch /worthwatch backup
  ```

Worthwatch also makes one automatically each day it runs, and before every restore.

## Copy backups off the machine

A backup on the same disk doesn't protect you from a dead disk or a lost laptop. Copy backups somewhere else regularly.

- **In the app:** on **Backups**, select a backup to download it.
- **Binary:** copy `<data>/backups/` to a cloud drive or another disk.
- **Docker:** copy the folder out of the volume:

  ```sh
  docker compose -f deploy/docker-compose.yml cp worthwatch:/data/backups ./worthwatch-backups
  ```

## Restore a backup

Restore unpacks a backup into the data folder. It checks the backup first, and only replaces files if the ledger in it loads.

### Restore with the binary

1. Stop Worthwatch.
2. Run `restore` with the backup file:

   ```sh
   WORTHWATCH_DATA=/path/to/data worthwatch restore worthwatch-2026-11-03-142210.tar.gz
   ```

   If the folder already has a ledger, `restore` refuses. Add `--force` to replace it. Worthwatch backs up the current files first and prints where.

3. Start Worthwatch again.

### Restore with Docker

1. Copy the backup into the volume:

   ```sh
   docker compose -f deploy/docker-compose.yml cp worthwatch-2026-11-03-142210.tar.gz worthwatch:/data/backups/
   ```

2. Stop the app and run the restore in a one-off container:

   ```sh
   docker compose -f deploy/docker-compose.yml stop worthwatch
   docker compose -f deploy/docker-compose.yml run --rm worthwatch restore /data/backups/worthwatch-2026-11-03-142210.tar.gz --force
   ```

3. Start the app again:

   ```sh
   docker compose -f deploy/docker-compose.yml start worthwatch
   ```

## Move to a new machine

1. Install Worthwatch on the new machine, but don't start it yet. See [Get started](../use/getting-started.md). The first start creates an empty ledger, and `restore` then needs `--force`.
2. Copy your latest backup to the new machine.
3. Restore it into an empty data folder:

   ```sh
   WORTHWATCH_DATA=/path/to/new/data worthwatch restore worthwatch-2026-11-03-142210.tar.gz
   ```

4. Start Worthwatch.
