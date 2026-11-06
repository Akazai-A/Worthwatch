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
- Node.js 22 or later. The install script turns on pnpm for you. If you install as a standard (not administrator) user, run `sudo corepack enable pnpm` from an administrator account first.
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

Worthwatch runs as you, not as root. Files it writes are readable by you only. It starts each time you log in.

To listen on another address, set `WORTHWATCH_BIND` when you install:

```sh
WORTHWATCH_BIND=127.0.0.1:7780 ./deploy/macos/install.sh
```

The script keeps `WORTHWATCH_BIND` and `WORTHWATCH_TAILSCALE_OWNER` from the last install when you don't set them, so an upgrade keeps your settings. To clear one, set it to empty.

## Open the dashboard over Tailscale

Over Tailscale, Worthwatch answers only the Apple Pay feed until you name your Tailscale login.

1. [Install Tailscale](https://tailscale.com/download/mac) and sign in. Find your login with `tailscale status --json` (look for `LoginName`).
2. Install again with your login:

   ```sh
   WORTHWATCH_TAILSCALE_OWNER=you@example.com ./deploy/macos/install.sh --no-build
   ```

3. Share Worthwatch with your tailnet:

   ```sh
   tailscale serve --bg 7777
   ```

4. On your phone, open the `https://….ts.net` address the command prints.

With the standalone Tailscale app, the command line is `/Applications/Tailscale.app/Contents/MacOS/Tailscale`. Add `alias tailscale=/Applications/Tailscale.app/Contents/MacOS/Tailscale` to `~/.zshrc`. For more, see [Access Worthwatch remotely](remote-access.md).

## Keep it running on a Mac mini

For a Mac that stays on as a server:

1. In **System Settings › Energy**, turn on **Prevent automatic sleeping when the display is off** and **Start up automatically after a power failure**.
2. Lock the screen with <kbd>Control</kbd> <kbd>Command</kbd> <kbd>Q</kbd> instead of logging out. The LaunchAgent runs only while you're logged in.
3. Decide how the Mac comes back after a power cut:
   - **FileVault on (recommended):** macOS turns off automatic login, so the Mac waits at its unlock screen. Unlock it with a keyboard, and Worthwatch starts. For a planned restart, run `sudo fdesetup authrestart` from an administrator account; the Mac unlocks once without asking.
   - **FileVault off:** turn on automatic login in **System Settings › Users & Groups**. The Mac comes back by itself, but anyone who takes it can read your data.

For a mini used only as a server, with no Apple ID and Worthwatch under a standard user, see [Keep Worthwatch secure](security.md#set-up-a-mac-mini-as-a-worthwatch-server).

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
| Worthwatch stops after you log out. | A LaunchAgent runs only while you're logged in. Lock the screen instead of logging out. See [Keep it running on a Mac mini](#keep-it-running-on-a-mac-mini). |
| Your phone gets only the Apple Pay feed over Tailscale. | Install with `WORTHWATCH_TAILSCALE_OWNER` set to your login. See [Open the dashboard over Tailscale](#open-the-dashboard-over-tailscale). |
| **Make Shortcut** fails. | Open the Shortcuts app on the Mac once and sign in to iCloud in it, then try again. |
