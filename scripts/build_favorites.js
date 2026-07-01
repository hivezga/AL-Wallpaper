#!/usr/bin/env node
// Seed data/favorites.json with a curated set of iconic shipgirls that have an L2D skin.
// For each target ship, pick the nicest skin (prefer a named, non-oath skin; else any).
const fs = require('fs'), path = require('path');
const ROOT = path.resolve(__dirname, '..');
const cat = require(path.join(ROOT, 'data', 'catalog.json'));

// curated wishlist (English ship names). Ones without an L2D skin are silently skipped.
const WISH = [
  'Enterprise', 'Belfast', 'Bismarck', 'Prinz Eugen', 'Taihou', 'Atago', 'Takao',
  'Saint Louis', 'Illustrious', 'Formidable', 'Hood', 'Roon', 'Friedrich der Grosse',
  'Akagi', 'Kaga', 'New Jersey', 'Bremerton', 'Gascogne', 'Sirius', 'Unicorn',
  'Cleveland', 'Helena', 'Yamashiro', 'Azuma', 'Cheshire',
];

function pickSkin(ship) {
  const ms = cat.skins.filter(s => s.ship.toLowerCase() === ship.toLowerCase());
  if (!ms.length) return null;
  ms.sort((a, b) => {
    if (a.is_oath !== b.is_oath) return a.is_oath ? 1 : -1;          // non-oath first
    const an = a.skin_name ? 0 : 1, bn = b.skin_name ? 0 : 1;        // named skins first
    if (an !== bn) return an - bn;
    return a.codename.localeCompare(b.codename);
  });
  return ms[0];
}

const favorites = [];
const report = [];
for (const ship of WISH) {
  const s = pickSkin(ship);
  if (s) { favorites.push(s.codename); report.push(`${ship.padEnd(20)} ${s.codename.padEnd(16)} ${s.skin_name || '(base)'}`); }
  else report.push(`${ship.padEnd(20)} — no L2D skin —`);
}
fs.writeFileSync(path.join(ROOT, 'data', 'favorites.json'), JSON.stringify({ favorites }, null, 2));
console.log(report.join('\n'));
console.log(`\n${favorites.length} favorites written.`);
