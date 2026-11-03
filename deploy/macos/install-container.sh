#!/usr/bin/env bash
# Run Worthwatch in Apple's `container` tool and start it at every login.
#
#   ./deploy/macos/install-container.sh            build, start, and add the login item
#   ./deploy/macos/install-container.sh --remove   stop it and remove the login item (data stays)
#
# Needs a Mac with Apple silicon and Apple's container tool
# (https://github.com/apple/container/releases). With Docker instead, run
# deploy/container/run.sh once and turn on Docker Desktop's "Start when you sign in".
set -euo pipefail

LABEL=com.worthwatch.container
REPO="$(cd "$(dirname "$0")/../.." && pwd)"
PLIST="$HOME/Library/LaunchAgents/$LABEL.plist"
LOGS="$HOME/Library/Logs/Worthwatch"

say() { printf '\033[1m%s\033[0m\n' "$*"; }
die() { printf 'install-container.sh: %s\n' "$*" >&2; exit 1; }

[[ "$(uname -s)" == "Darwin" ]] || die "this script is for macOS"

if [[ "${1:-}" == "--remove" ]]; then
  launchctl bootout "gui/$UID/$LABEL" 2>/dev/null || true
  rm -f "$PLIST"
  WORTHWATCH_ENGINE=container "$REPO/deploy/container/run.sh" --stop || true
  say "Removed the login item. Your data is still in ~/Library/Application Support/Worthwatch."
  exit 0
fi

[[ "$(uname -m)" == "arm64" ]] || die "Apple's container tool needs Apple silicon. Use Docker: deploy/container/run.sh"
command -v container >/dev/null || die "install Apple's container tool first: https://github.com/apple/container/releases"
# The old native LaunchAgent and this one would fight over port 7777.
if launchctl print "gui/$UID/com.worthwatch.server" >/dev/null 2>&1; then
  die "the native Worthwatch is running. Remove it first: ./deploy/macos/uninstall.sh"
fi

mkdir -p "$LOGS" "$(dirname "$PLIST")"
# Remember the settings you ran this with, for the login item.
CONFIG="$HOME/.config/worthwatch/container.env"
mkdir -p "$(dirname "$CONFIG")"
( umask 077
  for key in WORTHWATCH_PORT WORTHWATCH_DATA_DIR WORTHWATCH_TAPS_HOST_DIR WORTHWATCH_TAILSCALE_OWNER; do
    [[ -n "${!key:-}" ]] && printf '%s=%s\n' "$key" "${!key}"
  done > "$CONFIG" )
say "Building the image (a few minutes the first time)"
container system start
container build --tag worthwatch:local --file "$REPO/deploy/Dockerfile" "$REPO"

launchctl bootout "gui/$UID/$LABEL" 2>/dev/null || true
sed -e "s|__REPO__|$REPO|g" -e "s|__HOME__|$HOME|g" "$REPO/deploy/macos/$LABEL.plist" > "$PLIST"
plutil -lint "$PLIST" >/dev/null
launchctl bootstrap "gui/$UID" "$PLIST"

PORT="${WORTHWATCH_PORT:-7777}"
for _ in $(seq 1 60); do
  if curl -fsS "http://127.0.0.1:$PORT/api/health" >/dev/null 2>&1; then
    say "Worthwatch is running at http://127.0.0.1:$PORT, in its own VM."
    echo "It starts again at every login. Logs: $LOGS/container.log"
    exit 0
  fi
  sleep 1
done
die "it didn't answer on http://127.0.0.1:$PORT. Read $LOGS/container.log"
