#!/usr/bin/env bash
# Portable Live2D live-wallpaper apply (COSMIC / Hyprland / sway — any wlr-layer-shell compositor).
#   apply.sh <skin>                  apply to ALL monitors (+ save per-output state)
#   apply.sh <skin> --output <NAME>  apply to ONE monitor only (leaves others running)
#   apply.sh <skin> --fit <MODE>     force fit MODE (fit|stretch|crop) for the applied output(s)
#   apply.sh --stop                  remove the wallpaper (all)
#   apply.sh --outputs               print detected "NAME WxH" lines
# Without --fit, each output uses its saved per-monitor fit (state.js fit <NAME>; default "fit").
# Emits a line protocol on stdout for the GUI:
#   OUTPUTS <n> | TARGET <name> <w> <h> <i> <n> | CACHED <name> | RENDER <name> <w> <h>
#   PROGRESS <done> <total> | APPLIED <name> <skin> | DONE <skin> <n> | ERR <msg>
set -uo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
RENDER_DIR="$HOME/azurlane/wallpaper"
MODELS="$HOME/azurlane/extract/out_all/Live2DOutput"
OUTDIR="$RENDER_DIR/out"
export DOTNET_ROOT="$HOME/.dotnet"
mkdir -p "$OUTDIR"

detect_outputs(){
  if command -v wlr-randr >/dev/null 2>&1; then
    wlr-randr --json 2>/dev/null | node -e 'let s="";process.stdin.on("data",d=>s+=d).on("end",()=>{try{for(const o of JSON.parse(s)){if(!o.enabled)continue;const m=(o.modes||[]).find(m=>m.current);if(m)console.log(o.name+" "+m.width+"x"+m.height);}}catch(e){}});' && return 0
  fi
  if command -v cosmic-randr >/dev/null 2>&1; then
    cosmic-randr list 2>/dev/null | sed "s/\x1b\[[0-9;]*m//g" | awk '/\(enabled\)/{name=$1} /\(current\)/{for(i=1;i<=NF;i++) if($i ~ /^[0-9]+x[0-9]+$/){print name" "$i; break}}' && return 0
  fi
  if command -v hyprctl >/dev/null 2>&1; then
    hyprctl -j monitors 2>/dev/null | node -e 'let s="";process.stdin.on("data",d=>s+=d).on("end",()=>{try{for(const m of JSON.parse(s))console.log(m.name+" "+m.width+"x"+m.height);}catch(e){}});' && return 0
  fi
  return 1
}

kill_wait(){ # robustly kill processes matching the given pkill/pgrep args (used for -x mpvpaper)
  pkill "$@" 2>/dev/null
  for _ in 1 2 3 4 5; do pgrep "$@" >/dev/null 2>&1 || return 0; sleep 0.2; done
  pkill -9 "$@" 2>/dev/null; sleep 0.2
}
# kill only the mpvpaper instance whose cmdline has <output> as a standalone arg (robust, no false matches)
mpv_pids_for(){ pgrep -af mpvpaper | awk -v n="$1" '{for(i=2;i<=NF;i++) if($i==n){print $1; break}}'; }
kill_output(){
  local p; p=$(mpv_pids_for "$1"); [ -n "$p" ] && kill $p 2>/dev/null
  for _ in 1 2 3; do sleep 0.2; p=$(mpv_pids_for "$1"); [ -z "$p" ] && return 0; done
  [ -n "$p" ] && kill -9 $p 2>/dev/null
}

case "${1:-}" in
  --stop)    pkill -x mpvpaper 2>/dev/null && echo "stopped" || echo "nothing running"; exit 0;;
  --outputs) detect_outputs; exit 0;;
  "")        echo "usage: apply.sh <skin> [--output NAME] | --stop | --outputs"; exit 1;;
esac

SKIN="$1"; shift || true
ONLY=""; FIT_ARG=""
while [ $# -gt 0 ]; do
  case "$1" in
    --output) ONLY="${2:-}"; shift 2 || break;;
    --fit)    FIT_ARG="${2:-}"; shift 2 || break;;
    *)        shift;;
  esac
done
[ -d "$MODELS/$SKIN" ] || { echo "ERR no such skin: $SKIN"; exit 1; }

# resolve an output's fit mode: explicit --fit wins, else the saved per-monitor setting.
fit_for(){ if [ -n "$FIT_ARG" ]; then echo "$FIT_ARG"; else node "$HERE/state.js" fit "$1" 2>/dev/null || echo fit; fi; }
# cache path: default "fit" stays unsuffixed (backward-compatible with existing renders).
mp4_for(){ local s="$1" w="$2" h="$3" f="$4"; if [ "$f" = "fit" ]; then echo "$OUTDIR/${s}_${w}x${h}.mp4"; else echo "$OUTDIR/${s}_${w}x${h}_${f}.mp4"; fi; }

mapfile -t ALL < <(detect_outputs)
[ "${#ALL[@]}" -eq 0 ] && { echo "ERR no outputs detected"; exit 1; }

# power settings: pause/stop mpv when the wallpaper is hidden (e.g. a fullscreen game), optional fps cap.
read -r PWR_MODE PWR_FPS PWR_BAT < <(node "$HERE/state.js" power 2>/dev/null) || true
PWR_MODE="${PWR_MODE:-pause}"; PWR_FPS="${PWR_FPS:-0}"
MPV_FLAGS=(); MPV_OPTS="no-audio loop-file=inf hwdec=auto-safe --really-quiet"
case "$PWR_MODE" in pause) MPV_FLAGS+=(-p);; stop) MPV_FLAGS+=(-s);; esac
[ "${PWR_FPS:-0}" -gt 0 ] 2>/dev/null && MPV_OPTS="$MPV_OPTS --vf=fps=$PWR_FPS"

# build the work list (all, or just the one requested)
WORK=()
for e in "${ALL[@]}"; do
  [ -z "$ONLY" ] || [ "${e%% *}" = "$ONLY" ] && WORK+=("$e")
done
[ "${#WORK[@]}" -eq 0 ] && { echo "ERR output not found: $ONLY"; exit 1; }

echo "OUTPUTS ${#WORK[@]}"
# (no blanket kill — each output's old wallpaper stays up until its replacement is ready)

i=0
for entry in "${WORK[@]}"; do
  i=$((i+1))
  NAME="${entry%% *}"; RES="${entry##* }"; W="${RES%x*}"; H="${RES#*x}"
  FIT="$(fit_for "$NAME")"; [ -n "$FIT" ] || FIT="fit"
  MP4="$(mp4_for "$SKIN" "$W" "$H" "$FIT")"
  echo "TARGET $NAME $W $H $i ${#WORK[@]}"
  if [ -f "$MP4" ]; then
    echo "CACHED $NAME"
  else
    echo "RENDER $NAME $W $H"
    ( cd "$RENDER_DIR" && node render.js "$MODELS/$SKIN" "$MP4" --w "$W" --h "$H" --fit "$FIT" ) 2>/dev/null
    [ -f "$MP4" ] || { echo "ERR render failed for $NAME"; continue; }
  fi
  # replace just this output's wallpaper once its video is ready (others stay up)
  kill_output "$NAME"
  setsid -f mpvpaper ${MPV_FLAGS[@]+"${MPV_FLAGS[@]}"} -o "$MPV_OPTS" "$NAME" "$MP4" >/dev/null 2>&1
  echo "APPLIED $NAME $SKIN"
  node "$HERE/state.js" set-output "$NAME" "$SKIN"
done
echo "DONE $SKIN ${#WORK[@]}"
