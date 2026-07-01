#!/usr/bin/env bash
# Pick random skin(s) from the configured scope and apply. Used by rotate-daemon.sh and "Rotate now".
set -uo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(dirname "$HERE")"
read -r EN INT SCOPE PM LAST < <(node "$HERE/state.js" rotation)

mapfile -t POOL < <(node -e '
const root=process.argv[1], scope=process.argv[2];
const c=require(root+"/data/catalog.json"); let sk=c.skins;
if(scope==="favorites"){ let f=[]; try{f=require(root+"/data/favorites.json").favorites||[];}catch{} const set=new Set(f); sk=sk.filter(s=>set.has(s.codename)); }
else if(scope.startsWith("faction:")){ const f=scope.slice(8); sk=sk.filter(s=>s.faction===f); }
for(const s of sk) console.log(s.codename);
' "$ROOT" "$SCOPE")
[ "${#POOL[@]}" -eq 0 ] && { echo "rotate: empty pool for scope=$SCOPE"; exit 0; }
rand(){ echo "${POOL[$((RANDOM % ${#POOL[@]}))]}"; }

if [ "$PM" = "true" ]; then
  mapfile -t OUTS < <(bash "$HERE/apply.sh" --outputs)
  for e in "${OUTS[@]}"; do n="${e%% *}"; bash "$HERE/apply.sh" "$(rand)" --output "$n" >/dev/null 2>&1; done
else
  bash "$HERE/apply.sh" "$(rand)" >/dev/null 2>&1
fi
node "$HERE/state.js" set-last-run "$(date +%s)"
echo "rotated scope=$SCOPE per_monitor=$PM"
