#!/usr/bin/env bash
# Stop Worthwatch and remove it from launchd. Your data in
# ~/Library/Application Support/Worthwatch is left alone.
set -euo pipefail
LABEL=com.worthwatch.server
launchctl bootout "gui/$UID/$LABEL" 2>/dev/null || true
rm -f "$HOME/Library/LaunchAgents/$LABEL.plist" "$HOME/.local/bin/worthwatch"
echo "Worthwatch is stopped and removed. Your data is still in ~/Library/Application Support/Worthwatch."
