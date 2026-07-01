#!/usr/bin/env node
// Read/modify ~/.config/al-wallpaper/state.json — shared by apply.sh, rotate.sh, the autostart, and the GUI.
const fs = require('fs'), path = require('path'), os = require('os');
const DIR = path.join(process.env.XDG_CONFIG_HOME || path.join(os.homedir(), '.config'), 'al-wallpaper');
const FILE = path.join(DIR, 'state.json');
const DEFAULTS = { outputs: {}, fit: {}, rotation: { enabled: false, interval: '30m', scope: 'all', per_monitor: false, last_run: 0 }, power: { mode: 'pause', fps_cap: 0, battery: 'off' } };

function load(){
  try {
    const s = Object.assign({}, DEFAULTS, JSON.parse(fs.readFileSync(FILE, 'utf8')));
    s.power = Object.assign({}, DEFAULTS.power, s.power);   // backfill nested power defaults
    return s;
  }
  catch { return JSON.parse(JSON.stringify(DEFAULTS)); }
}
function save(s){ fs.mkdirSync(DIR, { recursive: true }); fs.writeFileSync(FILE, JSON.stringify(s, null, 2)); }

const [cmd, ...a] = process.argv.slice(2);
const s = load();
switch (cmd) {
  case 'init': save(s); break;
  case 'get': process.stdout.write(JSON.stringify(s, null, 2)); break;
  case 'set-output': s.outputs[a[0]] = a[1]; save(s); break;            // <name> <skin>
  case 'outputs': for (const [k, v] of Object.entries(s.outputs)) console.log(`${k} ${v}`); break;
  case 'set-fit': s.fit[a[0]] = a[1] || 'fit'; save(s); break;          // <name> <fit>
  case 'fit': process.stdout.write((s.fit && s.fit[a[0]]) || 'fit'); break; // <name> -> mode
  case 'set-rotation':                                                  // <enabled> <interval> <scope> <per_monitor>
    s.rotation.enabled = a[0] === 'true';
    s.rotation.interval = a[1] || s.rotation.interval;
    s.rotation.scope = a[2] || s.rotation.scope;
    s.rotation.per_monitor = a[3] === 'true';
    save(s); break;
  case 'set-last-run': s.rotation.last_run = Number(a[0]) || 0; save(s); break;
  case 'set-power':                                                    // <mode> <fps_cap> <battery>
    s.power.mode = ['off', 'pause', 'stop'].includes(a[0]) ? a[0] : 'pause';
    s.power.fps_cap = Math.max(0, Number(a[1]) || 0);
    s.power.battery = ['off', 'pause'].includes(a[2]) ? a[2] : s.power.battery;
    save(s); break;
  case 'power': console.log(`${s.power.mode} ${s.power.fps_cap} ${s.power.battery}`); break;
  case 'rotation':
    console.log(`${s.rotation.enabled} ${s.rotation.interval} ${s.rotation.scope} ${s.rotation.per_monitor} ${s.rotation.last_run}`);
    break;
  default: console.error('usage: state.js init|get|set-output|outputs|set-fit|fit|set-rotation|set-last-run|rotation|set-power|power'); process.exit(1);
}
