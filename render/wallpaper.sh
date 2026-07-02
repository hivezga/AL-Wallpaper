#!/usr/bin/env bash
# Azur Lane Live2D live wallpaper manager (COSMIC/Wayland via mpvpaper)
# Renders & applies per-monitor at native resolution; remembers last-applied as boot default.
#   wallpaper.sh <skin|english>   render(if needed) + set on all monitors + save as default
#   wallpaper.sh -f <skin>        force re-render then set
#   wallpaper.sh --list           list available extracted skins
#   wallpaper.sh --search <text>  grep skin codenames
#   wallpaper.sh --stop           remove the live wallpaper
#   wallpaper.sh --default        print current saved default
# English names resolve via aliases.tsv (english <TAB> codename-prefix); ambiguous names list the skins.
# Extra render knobs pass through, e.g.:  wallpaper.sh qiye_9 --ox 0.4 --fill 0.9
set -euo pipefail
P="$HOME/azurlane/al-wallpaper/render"
MODELS="$HOME/azurlane/extract/out_all/Live2DOutput"
OUTDIR="$P/out"; DEFAULT_FILE="$P/default.txt"; ALIASES="$P/aliases.tsv"
export DOTNET_ROOT="$HOME/.dotnet"
mkdir -p "$OUTDIR"

case "${1:-}" in
  --list)    ls "$MODELS"; exit 0;;
  --search)  shift; ls "$MODELS" | grep -i "${1:-}" || echo "(no match)"; exit 0;;
  --stop)    pkill -x mpvpaper 2>/dev/null && echo "wallpaper stopped" || echo "no wallpaper running"; exit 0;;
  --default) cat "$DEFAULT_FILE" 2>/dev/null || echo "(none set)"; exit 0;;
esac

FORCE=0
if [ "${1:-}" = "-f" ] || [ "${1:-}" = "--force" ]; then FORCE=1; shift; fi
QUERY="${1:-}"; shift || true
[ -z "${QUERY:-}" ] && { echo "usage: wallpaper.sh <skin|english> | --list | --search <t> | --stop | --default"; exit 1; }

# --- resolve query -> exact skin folder ---
resolve(){
  local q="$1"
  [ -d "$MODELS/$q" ] && { echo "$q"; return; }
  local code; code=$(awk -F'\t' -v k="$(echo "$q"|tr A-Z a-z)" 'tolower($1)==k{print $2}' "$ALIASES" 2>/dev/null | head -1)
  [ -z "$code" ] && code="$q"
  mapfile -t hits < <(ls "$MODELS" | grep -iE "^${code}(_|$)" || true)
  if   [ "${#hits[@]}" -eq 1 ]; then echo "${hits[0]}"
  elif [ "${#hits[@]}" -gt 1 ]; then echo "MULTI:${hits[*]}"
  else echo ""; fi
}
RES_SKIN="$(resolve "$QUERY")"
if [ -z "$RES_SKIN" ]; then echo "no match for '$QUERY' (try: wallpaper.sh --search <text>)"; exit 1; fi
if [[ "$RES_SKIN" == MULTI:* ]]; then
  echo "'$QUERY' has multiple skins — pick one:"; for s in ${RES_SKIN#MULTI:}; do echo "  wallpaper.sh $s"; done; exit 0
fi
SKIN="$RES_SKIN"

# detect enabled outputs + current resolution
mapfile -t OUTS < <(cosmic-randr list 2>/dev/null | sed 's/\x1b\[[0-9;]*m//g' | awk '
  /\(enabled\)/ {name=$1}
  /\(current\)/ {for(i=1;i<=NF;i++) if($i ~ /^[0-9]+x[0-9]+$/){print name" "$i; break}}')
[ "${#OUTS[@]}" -eq 0 ] && { echo "no outputs detected"; exit 1; }

pkill -x mpvpaper 2>/dev/null || true
sleep 0.3
for entry in "${OUTS[@]}"; do
  OUT_NAME="${entry%% *}"; RES="${entry##* }"; W="${RES%x*}"; H="${RES#*x}"
  MP4="$OUTDIR/${SKIN}_${W}x${H}.mp4"
  if [ "$FORCE" = 1 ] || [ ! -f "$MP4" ]; then
    echo "[*] rendering $SKIN @ ${W}x${H} for $OUT_NAME ..."
    ( cd "$P" && node render.js "$MODELS/$SKIN" "$MP4" --w "$W" --h "$H" "$@" )
  else
    echo "[*] $OUT_NAME: using cached ${SKIN}_${W}x${H}.mp4"
  fi
  setsid -f mpvpaper -o "no-audio loop-file=inf hwdec=auto-safe --really-quiet" "$OUT_NAME" "$MP4" >/dev/null 2>&1
  echo "[+] $OUT_NAME -> $SKIN"
done
echo "$SKIN" > "$DEFAULT_FILE"
echo "[done] '$SKIN' is live on ${#OUTS[@]} monitor(s) and saved as boot default.  (stop: wallpaper.sh --stop)"
