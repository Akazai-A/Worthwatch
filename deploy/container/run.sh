#!/usr/bin/env bash
# Run Worthwatch in a locked-down container, with Apple's `container` tool or Docker.
#
#   ./deploy/container/run.sh               build the image if needed, then start it
#   ./deploy/container/run.sh --rebuild     rebuild the image (after git pull), then restart
#   ./deploy/container/run.sh --foreground  stay attached; the login item uses this
#   ./deploy/container/run.sh --stop        stop and remove the container (your data stays)
#
# What it guarantees, whatever you set:
#   - the port is published on 127.0.0.1 only: nothing else on your network can reach it
#   - only Worthwatch's own data is mounted, plus (iCloud feed only) the one taps folder,
#     read-only. Never your home folder, never the rest of iCloud Drive.
#   - Docker: read-only image, no Linux capabilities, no privilege escalation, as uid 65532
#
# Settings (environment variables):
#   WORTHWATCH_ENGINE           container | docker   (default: container if installed, else docker)
#   WORTHWATCH_PORT             port on 127.0.0.1    (default 7777)
#   WORTHWATCH_DATA_DIR         Apple container only: folder for your data
#                               (default: ~/Library/Application Support/Worthwatch)
#   WORTHWATCH_VOLUME           Docker only: named volume for your data (default: the one
#                               docker compose made, deploy_worthwatch-data, if it exists;
#                               otherwise worthwatch-data)
#   WORTHWATCH_TAPS_HOST_DIR    iCloud feed only: the Worthwatch taps folder (mounted read-only)
#   WORTHWATCH_TAILSCALE_OWNER  your Tailscale login, to open the dashboard over Tailscale
#
# Settings can also live in ~/.config/worthwatch/container.env (KEY=value lines), which is
# what the login item reads; install-container.sh writes it.
set -euo pipefail

CONFIG="${WORTHWATCH_CONFIG:-$HOME/.config/worthwatch/container.env}"
if [[ -f "$CONFIG" ]]; then
  while IFS='=' read -r key value; do
    case "$key" in
      WORTHWATCH_ENGINE|WORTHWATCH_PORT|WORTHWATCH_DATA_DIR|WORTHWATCH_VOLUME|WORTHWATCH_TAPS_HOST_DIR|WORTHWATCH_TAILSCALE_OWNER)
        [[ -z "${!key:-}" ]] && export "$key=$value" ;;
    esac
  done < "$CONFIG"
fi

NAME=worthwatch
IMAGE=worthwatch:local
INIT_IMAGE=busybox:1.37
REPO="$(cd "$(dirname "$0")/../.." && pwd)"
PORT="${WORTHWATCH_PORT:-7777}"
MODE=start
case "${1:-}" in
  --rebuild) MODE=rebuild ;;
  --foreground) MODE=foreground ;;
  --stop) MODE=stop ;;
  "") ;;
  *) echo "usage: $0 [--rebuild | --foreground | --stop]" >&2; exit 2 ;;
esac

say() { printf '\033[1m%s\033[0m\n' "$*"; }
die() { printf 'run.sh: %s\n' "$*" >&2; exit 1; }

[[ "$PORT" =~ ^[0-9]+$ ]] || die "WORTHWATCH_PORT must be a number"

ENGINE="${WORTHWATCH_ENGINE:-}"
if [[ -z "$ENGINE" ]]; then
  if command -v container >/dev/null; then ENGINE=container
  elif command -v docker >/dev/null; then ENGINE=docker
  else die "install Apple's container tool (https://github.com/apple/container/releases) or Docker first"
  fi
fi
command -v "$ENGINE" >/dev/null || die "$ENGINE is not installed"

ENVS=()
MOUNTS=()
if [[ -n "${WORTHWATCH_TAPS_HOST_DIR:-}" ]]; then
  [[ -d "$WORTHWATCH_TAPS_HOST_DIR" ]] || die "WORTHWATCH_TAPS_HOST_DIR is not a folder: $WORTHWATCH_TAPS_HOST_DIR"
  case "$(basename "$WORTHWATCH_TAPS_HOST_DIR")" in
    Worthwatch) ;;
    *) die "WORTHWATCH_TAPS_HOST_DIR must be the Worthwatch taps folder itself, not a parent such as iCloud Drive" ;;
  esac
  MOUNTS+=(--volume "$WORTHWATCH_TAPS_HOST_DIR:/taps:ro")
  ENVS+=(--env WORTHWATCH_TAPS_DIR=/taps)
fi
[[ -n "${WORTHWATCH_TAILSCALE_OWNER:-}" ]] && ENVS+=(--env "WORTHWATCH_TAILSCALE_OWNER=$WORTHWATCH_TAILSCALE_OWNER")

if [[ "$ENGINE" == container ]]; then
  DATA="${WORTHWATCH_DATA_DIR:-$HOME/Library/Application Support/Worthwatch}"
  mkdir -p "$DATA" && chmod 700 "$DATA"
  MOUNTS=(--volume "$DATA:/data" "${MOUNTS[@]+"${MOUNTS[@]}"}")
  container system start >/dev/null 2>&1 || true
  # Run as your own uid (not root) so the process can write the folder you own on the Mac.
  RUN=(container run --name "$NAME" --publish "127.0.0.1:$PORT:7777" --user "$(id -u):$(id -g)")
  have_image() { container image inspect "$IMAGE" >/dev/null 2>&1; }
  remove() { container stop "$NAME" >/dev/null 2>&1 || true; container rm "$NAME" >/dev/null 2>&1 || true; }
else
  VOLUME="${WORTHWATCH_VOLUME:-}"
  if [[ -z "$VOLUME" ]]; then
    if docker volume inspect deploy_worthwatch-data >/dev/null 2>&1; then VOLUME=deploy_worthwatch-data
    else VOLUME=worthwatch-data; fi
  fi
  MOUNTS=(--volume "$VOLUME:/data" "${MOUNTS[@]+"${MOUNTS[@]}"}")
  RUN=(docker run --name "$NAME" --publish "127.0.0.1:$PORT:7777"
       --read-only --tmpfs /tmp:size=16m --cap-drop ALL --security-opt no-new-privileges:true
       --user 65532:65532 --pids-limit 256 --memory 512m)
  have_image() { docker image inspect "$IMAGE" >/dev/null 2>&1; }
  remove() { docker rm -f "$NAME" >/dev/null 2>&1 || true; }
fi

if [[ "$MODE" == stop ]]; then
  remove
  say "Stopped. Your data is still there."
  exit 0
fi

if [[ "$ENGINE" == docker ]]; then
  other="$(docker ps --filter publish="$PORT" --format '{{.Names}}' | grep -vx "$NAME" || true)"
  [[ -z "$other" ]] || die "port $PORT is already used by container '$other'. If that's the old docker compose setup, stop it first: docker compose -f deploy/docker-compose.yml down (your data stays)"
fi

if [[ "$MODE" == rebuild ]] || ! have_image; then
  say "Building the image with $ENGINE (a few minutes the first time)"
  "$ENGINE" build --tag "$IMAGE" --file "$REPO/deploy/Dockerfile" "$REPO"
fi

remove
if [[ "$ENGINE" == docker ]]; then
  # Images before this one ran as root, so an existing volume can be owned by root. Hand it to
  # the non-root user once more; a no-op when it already is. No network, only the caps chown needs.
  docker run --rm --network none --user 0:0 --cap-drop ALL --cap-add CHOWN --cap-add DAC_OVERRIDE \
    --cap-add FOWNER --volume "$VOLUME:/data" "$INIT_IMAGE" chown -R 65532:65532 /data
fi
if [[ "$MODE" == foreground ]]; then
  exec "${RUN[@]}" --rm "${MOUNTS[@]}" "${ENVS[@]+"${ENVS[@]}"}" "$IMAGE"
fi

if [[ "$ENGINE" == docker ]]; then
  "${RUN[@]}" --detach --restart unless-stopped "${MOUNTS[@]}" "${ENVS[@]+"${ENVS[@]}"}" "$IMAGE" >/dev/null
else
  "${RUN[@]}" --detach "${MOUNTS[@]}" "${ENVS[@]+"${ENVS[@]}"}" "$IMAGE" >/dev/null
fi

URL="http://127.0.0.1:$PORT"
for _ in $(seq 1 30); do
  if curl -fsS "$URL/api/health" >/dev/null 2>&1; then
    say "Worthwatch is running at $URL ($ENGINE)"
    exit 0
  fi
  sleep 1
done
die "the container started but $URL didn't answer. Check: $ENGINE logs $NAME"
