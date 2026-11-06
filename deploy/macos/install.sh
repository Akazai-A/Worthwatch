#!/usr/bin/env bash
# Build Worthwatch and run it on this Mac under launchd, starting at login.
#
#   ./deploy/macos/install.sh            # build, install, start
#   ./deploy/macos/install.sh --no-build # reinstall the binary already in target/release
#
#   WORTHWATCH_TAILSCALE_OWNER=you@example.com ./deploy/macos/install.sh --no-build
#                                        # let your Tailscale login open the dashboard
#
# WORTHWATCH_BIND and WORTHWATCH_TAILSCALE_OWNER are kept from the installed LaunchAgent
# when you don't set them, so an upgrade keeps your settings. Set one to empty to clear it.
#
# Your data lives in ~/Library/Application Support/Worthwatch, logs in
# ~/Library/Logs/Worthwatch. Re-run this script after `git pull` to upgrade.
set -euo pipefail

LABEL=com.worthwatch.server
REPO="$(cd "$(dirname "$0")/../.." && pwd)"
BIN="$HOME/.local/bin/worthwatch"
DATA="$HOME/Library/Application Support/Worthwatch"
LOGS="$HOME/Library/Logs/Worthwatch"
PLIST="$HOME/Library/LaunchAgents/$LABEL.plist"

# The value installed last time, so a plain upgrade keeps it.
installed() { [[ -f "$PLIST" ]] && /usr/libexec/PlistBuddy -c "Print :EnvironmentVariables:$1" "$PLIST" 2>/dev/null || true; }
BIND="${WORTHWATCH_BIND-$(installed WORTHWATCH_BIND)}"
BIND="${BIND:-127.0.0.1:7777}"
OWNER="${WORTHWATCH_TAILSCALE_OWNER-$(installed WORTHWATCH_TAILSCALE_OWNER)}"
BUILD=1
[[ "${1:-}" == "--no-build" ]] && BUILD=0

say() { printf '\033[1m%s\033[0m\n' "$*"; }
die() { printf 'install.sh: %s\n' "$*" >&2; exit 1; }

[[ "$(uname -s)" == "Darwin" ]] || die "this script is for macOS. On Linux, use deploy/docker-compose.yml."

if (( BUILD )); then
  command -v cargo >/dev/null || die "Rust is missing. Install it from https://rustup.rs, then run this again."
  if ! command -v pnpm >/dev/null; then
    command -v corepack >/dev/null || die "Node.js is missing. Install Node 20 or later (https://nodejs.org), then run this again."
    say "Turning on pnpm through corepack"
    corepack enable pnpm
  fi
  say "Building the web UI"
  pnpm --dir "$REPO/ui" install --frozen-lockfile
  pnpm --dir "$REPO/ui" build
  say "Building the server (a few minutes the first time)"
  cargo build --release --locked --manifest-path "$REPO/Cargo.toml" -p worthwatch
fi
[[ -x "$REPO/target/release/worthwatch" ]] || die "no binary at target/release/worthwatch. Run without --no-build."

say "Installing to $BIN"
mkdir -p "$(dirname "$BIN")" "$DATA" "$LOGS" "$(dirname "$PLIST")"
chmod 700 "$DATA"
# Stop the old copy before replacing its binary.
launchctl bootout "gui/$UID/$LABEL" 2>/dev/null || true
install -m 0755 "$REPO/target/release/worthwatch" "$BIN"

sed -e "s|__HOME__|$HOME|g" -e "s|__BIND__|$BIND|g" "$REPO/deploy/macos/$LABEL.plist" > "$PLIST"
if [[ -n "$OWNER" ]]; then
  plutil -insert EnvironmentVariables.WORTHWATCH_TAILSCALE_OWNER -string "$OWNER" "$PLIST"
  say "Your Tailscale login ($OWNER) can open the dashboard through tailscale serve"
fi
plutil -lint "$PLIST" >/dev/null

say "Starting $LABEL"
launchctl bootstrap "gui/$UID" "$PLIST"
launchctl kickstart -k "gui/$UID/$LABEL" >/dev/null

URL="http://$BIND"
for _ in $(seq 1 30); do
  if curl -fsS "$URL/api/health" >/dev/null 2>&1; then
    say "Worthwatch is running at $URL"
    echo "It starts again at every login. Logs: $LOGS/worthwatch.log"
    echo "To remove it: ./deploy/macos/uninstall.sh (your data stays)."
    open "$URL" 2>/dev/null || true
    exit 0
  fi
  sleep 1
done
die "the server did not answer at $URL within 30 seconds. See $LOGS/worthwatch.log"
