#!/usr/bin/env bash
# Pre-render skins at all current monitor resolutions so apply/rotation is instant.
#   prerender.sh <codename...>     OR     prerender.sh --favorites
set -uo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"; ROOT="$(dirname "$HERE")"
RENDER_DIR="$HOME/azurlane/wallpaper"
MODELS="$HOME/azurlane/extract/out_all/Live2DOutput"
OUTDIR="$RENDER_DIR/out"
export DOTNET_ROOT="$HOME/.dotnet"; mkdir -p "$OUTDIR"

LIST=("$@")
if [ "${1:-}" = "--favorites" ]; then
  mapfile -t LIST < <(node -e 'for(const c of (require(process.argv[1]+"/data/favorites.json").favorites||[])) console.log(c);' "$ROOT")
fi
[ "${#LIST[@]}" -eq 0 ] && { echo "nothing to render"; exit 0; }

# Distinct (resolution, fit) variants across all outputs — each monitor may want a different fit.
declare -A SEEN; VARIANTS=()
while read -r name res; do
  [ -n "$res" ] || continue
  fit="$(node "$HERE/state.js" fit "$name" 2>/dev/null)"; [ -n "$fit" ] || fit="fit"
  key="${res}_${fit}"
  [ -n "${SEEN[$key]:-}" ] && continue
  SEEN[$key]=1; VARIANTS+=("${res} ${fit}")
done < <(bash "$HERE/apply.sh" --outputs)
[ "${#VARIANTS[@]}" -eq 0 ] && { echo "no outputs"; exit 1; }

total=$(( ${#LIST[@]} * ${#VARIANTS[@]} )); done=0
for code in "${LIST[@]}"; do
  if [ ! -d "$MODELS/$code" ]; then echo "skip (no model): $code"; continue; fi
  for v in "${VARIANTS[@]}"; do
    r="${v%% *}"; fit="${v##* }"; w="${r%x*}"; h="${r#*x}"; done=$((done+1))
    if [ "$fit" = "fit" ]; then mp4="$OUTDIR/${code}_${w}x${h}.mp4"; else mp4="$OUTDIR/${code}_${w}x${h}_${fit}.mp4"; fi
    if [ -f "$mp4" ]; then
      echo "[$done/$total] cached  $code ${r} ${fit}"
    else
      echo "[$done/$total] render  $code ${r} ${fit}"
      ( cd "$RENDER_DIR" && node render.js "$MODELS/$code" "$mp4" --w "$w" --h "$h" --fit "$fit" ) >/dev/null 2>&1 || echo "  ! failed $code ${r} ${fit}"
    fi
  done
done
echo "prerender complete: ${#LIST[@]} skins × ${#VARIANTS[@]} variants"
