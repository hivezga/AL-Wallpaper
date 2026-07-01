#!/usr/bin/env bash
# Lightweight DE-independent rotation scheduler: wakes every 60s, rotates when the
# configured interval has elapsed since last_run (robust across reboots; no systemd).
set -uo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
LOCK="${XDG_CONFIG_HOME:-$HOME/.config}/al-wallpaper/rotate.lock"
mkdir -p "$(dirname "$LOCK")"
exec 9>"$LOCK"
flock -n 9 || exit 0   # only one daemon

secs(){ case "$1" in 5m)echo 300;;15m)echo 900;;30m)echo 1800;;1h)echo 3600;;6h)echo 21600;;daily)echo 86400;;weekly)echo 604800;;monthly)echo 2592000;;*)echo 1800;;esac; }

while true; do
  read -r EN INT SCOPE PM LAST < <(node "$HERE/state.js" rotation 2>/dev/null) || true
  if [ "${EN:-false}" = "true" ]; then
    now=$(date +%s); iv=$(secs "${INT:-30m}")
    if [ $(( now - ${LAST:-0} )) -ge "$iv" ]; then
      bash "$HERE/rotate.sh" >> "$HOME/azurlane/al-wallpaper/data/rotate.log" 2>&1 || true
    fi
  fi
  sleep 60
done
