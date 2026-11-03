# Run Worthwatch on a Mac

This page shows you how to run Worthwatch natively on a Mac, so it starts at login and keeps running in the background.

> [!WARNING]
> The native install runs Worthwatch as you, with access to everything your user can read. For a Mac that also holds your iCloud, photos or passwords, use the container setup in [Keep Worthwatch secure](security.md) instead.

```text
 login ─▶ launchd ─▶ ~/.local/bin/worthwatch ─▶ http://127.0.0.1:7777
                       │  restarts it if it stops
                       ├─ data: ~/Library/Application Support/Worthwatch
                       ├─ logs: ~/Library/Logs/Worthwatch/worthwatch.log
                       └─ reads: iCloud Drive › Shortcuts › Worthwatch
```

## Before you begin

- macOS 14 Sonoma or later.
- Rust, installed with [rustup](https://rustup.rs).
- Node.js 22 or later. The install script turns on pnpm for you.
- A copy of the repository:

  ```sh
  git clone https://github.com/Akazai-A/Worthwatch.git
  cd Worthwatch
  ```

## Install Worthwatch

Run the install script from the repository:

```sh
./deploy/macos/install.sh
```

The script does the following:

1. Builds the web UI and the server. The first build takes a few minutes.
2. Copies the binary to `~/.local/bin/worthwatch`.
3. Adds a LaunchAgent at `~/Library/LaunchAgents/com.worthwatch.server.plist`.
4. Starts Worthwatch and opens <http://127.0.0.1:7777> once it answers.

Worthwatch runs as you, not as root. Files it writes are readable by you only. It starts each time you log in. To have it running after a power cut, turn on automatic login in **System Settings › Users & Groups**, and **Start up automatically after a power failure** in **System Settings › Energy**.

To listen on another address, set `WORTHWATCH_BIND` when you install:

```sh
WORTHWATCH_BIND=127.0.0.1:7780 ./deploy/macos/install.sh
```

To reach Worthwatch from your phone, use Tailscale. See [Access Worthwatch remotely](remote-access.md).

## Use the Tailscale feed, not Full Disk Access

macOS blocks a background program from reading iCloud Drive. The only switch that unblocks it, **Full Disk Access**, also opens your Mail, Messages, Safari data and the rest of iCloud Drive to Worthwatch. Don't turn it on.

Send Apple Pay taps over Tailscale instead. See [Set up the Apple Pay feed](../use/apple-pay-feed.md#use-tailscale). If you gave Worthwatch Full Disk Access before, remove it in **System Settings › Privacy & Security › Full Disk Access**.

## Upgrade Worthwatch

Pull the new code and run the install script again. Your data stays where it is.

```sh
git pull
./deploy/macos/install.sh
```

If you rebuilt the binary yourself, skip the build:

```sh
./deploy/macos/install.sh --no-build
```

## Move your data from Docker

1. In the Docker version, make a backup and download it from **Backups**. See [Back up and restore](backup-restore.md).
2. Stop the container:

   ```sh
   docker compose -f deploy/docker-compose.yml down
   ```

3. Install the native version as described in [Install Worthwatch](#install-worthwatch).
4. Restore the backup:

   ```sh
   WORTHWATCH_DATA="$HOME/Library/Application Support/Worthwatch" \
     ~/.local/bin/worthwatch restore ~/Downloads/worthwatch-<date>-<time>.tar.gz --force
   ```

## Check that it's running

```sh
launchctl print gui/$UID/com.worthwatch.server | grep state
curl -s http://127.0.0.1:7777/api/health
tail -f ~/Library/Logs/Worthwatch/worthwatch.log
```

## Uninstall Worthwatch

```sh
./deploy/macos/uninstall.sh
```

This stops Worthwatch and removes the binary and the LaunchAgent. Your data in `~/Library/Application Support/Worthwatch` stays. Delete that folder too if you want everything gone.

## Troubleshoot

| Symptom | What to do |
|---|---|
| The script says Rust or Node.js is missing. | Install it from the link in the message, open a new Terminal window, and run the script again. |
| The script says the server didn't answer. | Read `~/Library/Logs/Worthwatch/worthwatch.log`. *Address already in use* means something else is on port 7777: stop it, or install with another `WORTHWATCH_BIND`. |
| Worthwatch stops after you log out. | A LaunchAgent runs only while you're logged in. Turn on automatic login, and lock the screen instead of logging out. |
| **Make Shortcut** fails. | Open the Shortcuts app on the Mac once and sign in to iCloud in it, then try again. |
