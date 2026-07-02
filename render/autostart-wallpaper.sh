#!/usr/bin/env bash
# Launched by XDG autostart on login. Restores the per-monitor Live2D wallpapers from
# al-wallpaper state, then starts the rotation daemon. Logs to autostart.log.
P="$HOME/azurlane/al-wallpaper"
HERE="$P/scripts"
LOG="$P/render/autostart.log"
echo "=== $(date) autostart ===" >> "$LOG"

# wait up to ~30s for the compositor to report at least one output
for i in $(seq 1 30); do
  if bash "$HERE/apply.sh" --outputs 2>/dev/null | grep -q .; then break; fi
  sleep 1
done

# restore saved per-output assignments; fall back to a default skin
mapfile -t SAVED < <(node "$HERE/state.js" outputs 2>/dev/null)
if [ "${#SAVED[@]}" -gt 0 ]; then
  for line in "${SAVED[@]}"; do
    n="${line%% *}"; sk="${line##* }"
    bash "$HERE/apply.sh" "$sk" --output "$n" >> "$LOG" 2>&1
  done
else
  DEF="$(cat "$P/render/default.txt" 2>/dev/null || echo qiye_9)"
  bash "$HERE/apply.sh" "$DEF" >> "$LOG" 2>&1
fi

# start the rotation + power daemons (no-ops if disabled; each self-locks against duplicates)
setsid -f bash "$HERE/rotate-daemon.sh" >/dev/null 2>&1
setsid -f bash "$HERE/power-daemon.sh" >/dev/null 2>&1
echo "autostart done" >> "$LOG"
