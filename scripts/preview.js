#!/usr/bin/env node
// Render a low-res preview frame of each skin at its NATIVE painting aspect, so the GUI's
// "fit on screen" preview shows the real artwork shape (square / 16:9 / 3:2 / …) instead of
// guessing from the 3:4 gallery thumbnail. Output: <root>/assets/preview/<codename>.png
//   node preview.js <codename> [<codename> ...]   render the given skins (skip if present)
//   node preview.js --all                         render every skin in the catalog
//   node preview.js --force <codename ...>        re-render even if the file exists
const http = require('http'), fs = require('fs'), path = require('path'), os = require('os');
const ROOT = path.dirname(__dirname);                       // …/al-wallpaper
const WALL = path.join(os.homedir(), 'azurlane', 'wallpaper'); // index.html + vendor + puppeteer
const MODELS = path.join(os.homedir(), 'azurlane', 'extract', 'out_all', 'Live2DOutput');
const OUT = path.join(ROOT, 'assets', 'preview');
const MAX = 512; // longest side of the preview image
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
if (!names.length) { console.error('usage: preview.js <codename...> | --all [--force]'); process.exit(1); }

const MIME = { '.js': 'text/javascript', '.html': 'text/html', '.json': 'application/json', '.png': 'image/png', '.moc3': 'application/octet-stream' };
function send(res, fp) {
  if (!fs.existsSync(fp)) { res.writeHead(404); res.end(); return; }
  res.writeHead(200, { 'Content-Type': MIME[path.extname(fp)] || 'application/octet-stream', 'Access-Control-Allow-Origin': '*' });
  fs.createReadStream(fp).pipe(res);
}

(async () => {
  fs.mkdirSync(OUT, { recursive: true });
  const browser = await puppeteer.launch({
    headless: true,
    args: ['--no-sandbox', '--use-gl=angle', '--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist', '--enable-webgl', '--hide-scrollbars'],
  });
  const page = await browser.newPage();
  let ok = 0, skip = 0, fail = 0;
  for (const name of names) {
    const dst = path.join(OUT, name + '.png');
    if (!force && fs.existsSync(dst)) { skip++; continue; }
    const dir = path.join(MODELS, name);
    const m3 = fs.existsSync(dir) ? fs.readdirSync(dir).find(f => f.endsWith('.model3.json')) : null;
    if (!m3) { console.log('skip (no model):', name); fail++; continue; }

    // per-skin static server (serves index.html/vendor from WALL, /model/* from this skin's dir)
    const server = http.createServer((req, res) => {
      const u = decodeURIComponent(req.url.split('?')[0]);
      if (u === '/' || u === '/index.html') return send(res, path.join(WALL, 'index.html'));
      if (u.startsWith('/vendor/')) return send(res, path.join(WALL, u));
      if (u.startsWith('/model/')) return send(res, path.join(dir, u.slice('/model/'.length)));
      res.writeHead(404); res.end();
    });
    await new Promise(r => server.listen(0, r));
    const port = server.address().port;
    const base = `http://127.0.0.1:${port}/?model=/model/${encodeURIComponent(m3)}`;
    try {
      // pass 1: load square, read the model's native bounds aspect
      await page.setViewport({ width: 600, height: 600, deviceScaleFactor: 1 });
      await page.goto(base + '&w=600&h=600&fit=fit', { waitUntil: 'load', timeout: 60000 });
      await page.waitForFunction('window.__ready===true || window.__err!==null', { timeout: 60000 });
      if (await page.evaluate('window.__err')) { console.log('fail:', name, await page.evaluate('window.__err')); fail++; server.close(); continue; }
      const asp = await page.evaluate(() => { const m = window.__model; m.position.set(0, 0); m.scale.set(1); m.update(0); window.__app.renderer.render(window.__app.stage); const b = m.getBounds(true); return b.width / Math.max(1, b.height); });
      // pass 2: render the full painting at its native aspect (fit==aspect ⇒ no bars)
      const W = asp >= 1 ? MAX : Math.round(MAX * asp);
      const H = asp >= 1 ? Math.round(MAX / asp) : MAX;
      await page.setViewport({ width: W, height: H, deviceScaleFactor: 1 });
      await page.goto(base + `&w=${W}&h=${H}&fit=fit`, { waitUntil: 'load', timeout: 60000 });
      await page.waitForFunction('window.__ready===true || window.__err!==null', { timeout: 60000 });
      if (await page.evaluate('window.__err')) { console.log('fail:', name, await page.evaluate('window.__err')); fail++; server.close(); continue; }
      for (let k = 0; k < 20; k++) await page.evaluate('window.__frame()'); // warm up past fade-in
      await page.screenshot({ path: dst, clip: { x: 0, y: 0, width: W, height: H } });
      ok++; process.stdout.write(`\r[preview] ${ok + skip + fail}/${names.length}  ${name} ${W}x${H}        `);
    } catch (e) {
      console.log('err:', name, e.message); fail++;
    } finally {
      server.close();
    }
  }
  await browser.close();
  process.stdout.write(`\n[preview] done ok=${ok} skip=${skip} fail=${fail} -> ${OUT}\n`);
})().catch(e => { console.error(e); process.exit(1); });
