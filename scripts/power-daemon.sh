#!/usr/bin/env bash
# Battery-aware wallpaper pausing: while on battery, freeze mpvpaper (SIGSTOP) so the
# animated wallpaper stops drawing; resume (SIGCONT) on AC. DE-independent; polls
# /sys/class/power_supply every 30s. No-op when the battery setting is "off" or the
# machine has no battery (desktop). Self-locks against duplicate daemons.
set -uo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
LOCK="${XDG_CONFIG_HOME:-$HOME/.config}/al-wallpaper/power.lock"
mkdir -p "$(dirname "$LOCK")"
exec 9>"$LOCK"
flock -n 9 || exit 0   # only one daemon

# echo 1 if running on battery, 0 if on AC / unknown / no battery (desktop).
on_battery(){
  local f d
  # Prefer an AC adapter's online flag (most reliable).
  for f in /sys/class/power_supply/*/type; do
    [ -r "$f" ] || continue
    [ "$(cat "$f" 2>/dev/null)" = "Mains" ] || continue
    d="$(dirname "$f")"
    [ -r "$d/online" ] || continue
    if [ "$(cat "$d/online" 2>/dev/null)" = "0" ]; then echo 1; else echo 0; fi
    return
  done
  # No AC adapter: fall back to any battery's charge status.
  for f in /sys/class/power_supply/*/status; do
    [ -r "$f" ] || continue
    case "$(cat "$f" 2>/dev/null)" in
      Discharging) echo 1; return;;
      Charging|Full) echo 0; return;;
    esac
  done
  echo 0   # no power-supply info -> assume AC (desktop)
}

while true; do
  read -r MODE FPS BAT < <(node "$HERE/state.js" power 2>/dev/null) || true
  if [ "${BAT:-off}" = "pause" ] && [ "$(on_battery)" = "1" ]; then
    pkill -STOP -x mpvpaper 2>/dev/null   # freeze: no decode/render while on battery
  else
    pkill -CONT -x mpvpaper 2>/dev/null   # resume anything we previously froze
  fi
  sleep 30
done
