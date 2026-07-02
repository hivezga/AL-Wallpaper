#!/usr/bin/env node
// Render a short looping animation of each skin's idle motion at its NATIVE painting aspect,
// encoded as a small GIF the GUI plays in the detail panel (a true animated preview without
// an in-app Cubism engine). Output: <root>/assets/preview_anim/<codename>.gif
//   node preview_anim.js <codename> [<codename> ...]   render the given skins (skip if present)
//   node preview_anim.js --all                         render every skin in the catalog
//   node preview_anim.js --force <codename ...>        re-render even if the file exists
// Mirrors preview.js (same headless pipeline + native-aspect detection); steps window.__frame()
// once per output frame, screenshots each, then ffmpeg → palette-optimised looping GIF.
const http = require('http'), fs = require('fs'), path = require('path'), os = require('os');
const { execFileSync } = require('child_process');
const ROOT = path.dirname(__dirname);                         // …/al-wallpaper
const WALL = path.join(ROOT, 'render');                       // index.html + vendor + puppeteer
const MODELS = path.join(os.homedir(), 'azurlane', 'extract', 'out_all', 'Live2DOutput');
const OUT = path.join(ROOT, 'assets', 'preview_anim');
const MAX = 360;       // longest side of the GIF (small — it's a preview)
const FPS = 12;        // preview frame rate
const MAX_FRAMES = 72; // cap loop length (~6s) so long idles don't bloat the GIF
const puppeteer = require(path.join(WALL, 'node_modules', 'puppeteer'));

let args = process.argv.slice(2).join(' ').split(/\s+/).filter(Boolean);
const force = args.includes('--force'); args = args.filter(a => a !== '--force');
let names;
if (args.includes('--all')) {
  const cat = JSON.parse(fs.readFileSync(path.join(ROOT, 'data', 'catalog.json'), 'utf8'));
  names = cat.skins.map(s => s.codename);
} else {
  names = args;
}
if (!names.length) { console.error('usage: preview_anim.js <codename...> | --all [--force]'); process.exit(1); }

const MIME = { '.js': 'text/javascript', '.html': 'text/html', '.json': 'application/json', '.png': 'image/png', '.moc3': 'application/octet-stream' };
function send(res, fp) {
  if (!fs.existsSync(fp)) { res.writeHead(404); res.end(); return; }
  res.writeHead(200, { 'Content-Type': MIME[path.extname(fp)] || 'application/octet-stream', 'Access-Control-Allow-Origin': '*' });
  fs.createReadStream(fp).pipe(res);
}

(async () => {
  fs.mkdirSync(OUT, { recursive: true });
  const tmpRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'al-anim-'));
  const browser = await puppeteer.launch({
    headless: true,
    args: ['--no-sandbox', '--use-gl=angle', '--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist', '--enable-webgl', '--hide-scrollbars'],
  });
  const page = await browser.newPage();
  let ok = 0, skip = 0, fail = 0;
  for (const name of names) {
    const dst = path.join(OUT, name + '.gif');
    if (!force && fs.existsSync(dst)) { skip++; continue; }
    const dir = path.join(MODELS, name);
    const m3 = fs.existsSync(dir) ? fs.readdirSync(dir).find(f => f.endsWith('.model3.json')) : null;
    if (!m3) { console.log('skip (no model):', name); fail++; continue; }

    const server = http.createServer((req, res) => {
      const u = decodeURIComponent(req.url.split('?')[0]);
      if (u === '/' || u === '/index.html') return send(res, path.join(WALL, 'index.html'));
      if (u.startsWith('/vendor/')) return send(res, path.join(WALL, u));
      if (u.startsWith('/model/')) return send(res, path.join(dir, u.slice('/model/'.length)));
      res.writeHead(404); res.end();
    });
    await new Promise(r => server.listen(0, r));
    const port = server.address().port;
    const base = `http://127.0.0.1:${port}/?model=/model/${encodeURIComponent(m3)}&fps=${FPS}`;
    const framesDir = fs.mkdtempSync(path.join(tmpRoot, name + '-'));
    try {
      // pass 1: load square, read the model's native bounds aspect (same as preview.js)
      await page.setViewport({ width: 600, height: 600, deviceScaleFactor: 1 });
      await page.goto(base + '&w=600&h=600&fit=fit', { waitUntil: 'load', timeout: 60000 });
      await page.waitForFunction('window.__ready===true || window.__err!==null', { timeout: 60000 });
      if (await page.evaluate('window.__err')) { console.log('fail:', name, await page.evaluate('window.__err')); fail++; server.close(); continue; }
      const asp = await page.evaluate(() => { const m = window.__model; m.position.set(0, 0); m.scale.set(1); m.update(0); window.__app.renderer.render(window.__app.stage); const b = m.getBounds(true); return b.width / Math.max(1, b.height); });
      const W = asp >= 1 ? MAX : Math.round(MAX * asp);
      const H = asp >= 1 ? Math.round(MAX / asp) : MAX;

      // how many frames: full idle loop (seamless) capped at MAX_FRAMES
      let seconds = 4;
      try { seconds = JSON.parse(fs.readFileSync(path.join(dir, 'motions', 'idle.motion3.json'), 'utf8')).Meta.Duration || 4; } catch {}
      const FRAMES = Math.min(MAX_FRAMES, Math.max(12, Math.round(seconds * FPS)));

      // pass 2: render at native aspect, step + screenshot each frame
      await page.setViewport({ width: W, height: H, deviceScaleFactor: 1 });
      await page.goto(base + `&w=${W}&h=${H}&fit=fit`, { waitUntil: 'load', timeout: 60000 });
      await page.waitForFunction('window.__ready===true || window.__err!==null', { timeout: 60000 });
      if (await page.evaluate('window.__err')) { console.log('fail:', name, await page.evaluate('window.__err')); fail++; server.close(); continue; }
      for (let k = 0; k < 20; k++) await page.evaluate('window.__frame()'); // warm up past fade-in
      for (let k = 0; k < FRAMES; k++) {
        await page.evaluate('window.__frame()');
        await page.screenshot({ path: path.join(framesDir, String(k).padStart(4, '0') + '.png'), clip: { x: 0, y: 0, width: W, height: H } });
      }

      // encode: two-pass palette for a clean small looping GIF
      const pal = path.join(framesDir, 'palette.png');
      execFileSync('ffmpeg', ['-y', '-loglevel', 'error', '-framerate', String(FPS), '-i', path.join(framesDir, '%04d.png'),
        '-vf', 'palettegen=stats_mode=diff', pal]);
      execFileSync('ffmpeg', ['-y', '-loglevel', 'error', '-framerate', String(FPS), '-i', path.join(framesDir, '%04d.png'), '-i', pal,
        '-lavfi', 'paletteuse=dither=bayer:bayer_scale=3', '-loop', '0', dst]);
      ok++; process.stdout.write(`\r[anim] ${ok + skip + fail}/${names.length}  ${name} ${W}x${H} ${FRAMES}f        `);
    } catch (e) {
      console.log('err:', name, e.message); fail++;
    } finally {
      server.close();
      fs.rmSync(framesDir, { recursive: true, force: true });
    }
  }
  await browser.close();
  fs.rmSync(tmpRoot, { recursive: true, force: true });
  process.stdout.write(`\n[anim] done ok=${ok} skip=${skip} fail=${fail} -> ${OUT}\n`);
})().catch(e => { console.error(e); process.exit(1); });
