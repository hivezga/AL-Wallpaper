#!/usr/bin/env node
// Build catalog.json: one entry per extracted L2D skin folder, joined to ship name + faction + skin name.
// Source: AzurLaneTools/AzurLaneData (EN). Falls back to bootstrap if sources missing.
const fs = require('fs'), path = require('path');
const ROOT = path.resolve(__dirname, '..');
const MODELS = process.env.MODELS || path.join(process.env.HOME, 'azurlane/extract/out_all/Live2DOutput');
const SRC = path.join(ROOT, 'data', 'sources');
const OUT = path.join(ROOT, 'data', 'catalog.json');

const folders = fs.readdirSync(MODELS).filter(f => fs.statSync(path.join(MODELS, f)).isDirectory());
const j = n => JSON.parse(fs.readFileSync(path.join(SRC, n), 'utf8'));
function humanize(code){ const b = code.replace(/_hx$/,'').replace(/_\d+$/,''); return b.charAt(0).toUpperCase()+b.slice(1); }

function build(){
  const haveReal = fs.existsSync(path.join(SRC,'ship_skin_template.json'));
  let byPainting = {}, byBase = {}, natByGroup = {}, nameByGid = {}, natMap = {};
  if (haveReal){
    const skin = j('ship_skin_template.json');
    const grp  = j('ship_data_group.json');
    const stat = j('ship_data_statistics.json');
    natMap = j('nationality_map.json');
    for (const v of Object.values(skin)) if (v && v.painting) {
      byPainting[v.painting] = v;
      const b = v.painting.replace(/_hx$/,'').replace(/_\d+$/,'');
      if (!(b in byBase)) byBase[b] = v;   // first skin of each ship → base group/faction
    }
    for (const v of Object.values(grp))  if (v && v.group_type != null) natByGroup[v.group_type] = v.nationality;
    for (const [k,v] of Object.entries(stat)) if (v && v.name) nameByGid[k] = v;
  }
  function shipName(group){
    for (const r of [1,0,2,3,4,5]) { const e = nameByGid[String(group*10+r)]; if (e) return e.name; }
    return null;
  }
  let unmatched = [];
  const skins = folders.map(code => {
    const is_oath = /_hx$/.test(code);
    let sk = byPainting[code] || byPainting[code.replace(/_hx$/,'')];
    let approx = false;
    if (!sk && haveReal) {
      const base = code.replace(/_hx$/,'').replace(/_\d+$/,'');
      if (byBase[base]) { sk = byBase[base]; approx = true; }   // faction/ship from base skin; skin name unknown
    }
    let ship = humanize(code), faction = 'other', skin_name = null, rarity = null;
    if (sk){
      const g = sk.ship_group;
      const nat = natByGroup[g];
      faction = (nat != null && natMap[String(nat)]) ? natMap[String(nat)] : 'other';
      ship = shipName(g) || ship;
      skin_name = approx ? null : (sk.name || null);
      if (approx) unmatched.push(code);
    } else if (haveReal) {
      unmatched.push(code);
    }
    return { codename: code, ship, faction, rarity, skin_name, is_oath,
             thumb: `assets/thumbs/${code}.png`, has_l2d: true,
             _source: sk ? 'azurlane-data' : 'bootstrap' };
  });
  return { skins, unmatched };
}

const { skins, unmatched } = build();
const byFaction = {};
for (const s of skins) byFaction[s.faction] = (byFaction[s.faction]||0)+1;
fs.writeFileSync(OUT, JSON.stringify({ generated: process.env.TODAY||'', count: skins.length, skins }, null, 2));
console.log(`catalog.json: ${skins.length} skins`);
console.log('per-faction:', JSON.stringify(byFaction, null, 0));
if (unmatched.length) console.log(`unmatched (${unmatched.length}):`, unmatched.join(', '));
