// Render an Azur Lane Live2D skin's idle loop to a seamless looping mp4.
// Usage: node render.js <modelDir> [outMp4] [--w 1920 --h 1080 --fps 30 --ox 0.5 --oy 0.55 --fit fit|stretch|crop --seconds <override>]
const http = require('http');
const fs = require('fs');
const path = require('path');
const { execFileSync } = require('child_process');
const puppeteer = require('puppeteer');

function arg(name, def){ const i=process.argv.indexOf('--'+name); return i>=0?process.argv[i+1]:def; }

const MODEL_DIR = path.resolve(process.argv[2] || '');
if(!MODEL_DIR || !fs.existsSync(MODEL_DIR)){ console.error('model dir not found:', MODEL_DIR); process.exit(1); }
const model3 = fs.readdirSync(MODEL_DIR).find(f=>f.endsWith('.model3.json'));
if(!model3){ console.error('no .model3.json in', MODEL_DIR); process.exit(1); }
const name = model3.replace('.model3.json','');
const OUT = path.resolve(process.argv[3] && !process.argv[3].startsWith('--') ? process.argv[3] : path.join(__dirname,'out',name+'.mp4'));

const W=+arg('w',1920), H=+arg('h',1080), FPS=+arg('fps',30);
const OX=+arg('ox',0.5), OY=+arg('oy',0.55), FIT=arg('fit','fit');
const WALL = __dirname;

// idle duration -> frame count
const idlePath = path.join(MODEL_DIR,'motions','idle.motion3.json');
let seconds = +arg('seconds', 0);
if(!seconds){
  const idle = JSON.parse(fs.readFileSync(idlePath,'utf8'));
  seconds = idle.Meta.Duration;
}
const FRAMES = Math.round(seconds*FPS);

// --- tiny static server: /vendor/* and /index.html from WALL, /model/* from MODEL_DIR ---
const MIME={'.js':'text/javascript','.html':'text/html','.json':'application/json','.png':'image/png','.moc3':'application/octet-stream'};
function send(res,fp){
  if(!fs.existsSync(fp)){res.writeHead(404);res.end('nf');return;}
  res.writeHead(200,{'Content-Type':MIME[path.extname(fp)]||'application/octet-stream','Access-Control-Allow-Origin':'*'});
  fs.createReadStream(fp).pipe(res);
}
const server = http.createServer((req,res)=>{
  const u = decodeURIComponent(req.url.split('?')[0]);
  if(u==='/'||u==='/index.html') return send(res,path.join(WALL,'index.html'));
  if(u.startsWith('/vendor/')) return send(res,path.join(WALL,u));
  if(u.startsWith('/model/')) return send(res,path.join(MODEL_DIR,u.slice('/model/'.length)));
  res.writeHead(404); res.end('nf');
});

(async ()=>{
  await new Promise(r=>server.listen(0,r));
  const port = server.address().port;
  // Per-pid frames dir so concurrent renders (e.g. rotate-daemon + a manual apply) never
  // clobber each other's PNGs — otherwise ffmpeg would encode a mixed frame set and the
  // atomic rename below would install that corruption as a "valid" cache mp4.
  const framesDir = path.join(WALL,'frames',String(process.pid)); fs.rmSync(framesDir,{recursive:true,force:true}); fs.mkdirSync(framesDir,{recursive:true});
  fs.mkdirSync(path.dirname(OUT),{recursive:true});

  const url = `http://127.0.0.1:${port}/?w=${W}&h=${H}&fps=${FPS}&ox=${OX}&oy=${OY}&fit=${encodeURIComponent(FIT)}&model=/model/${encodeURIComponent(model3)}`;
  console.log(`[render] ${name}  ${W}x${H}@${FPS}  ${FIT}  ${seconds}s -> ${FRAMES} frames`);

  const browser = await puppeteer.launch({
    headless: true,
    args:['--no-sandbox','--use-gl=angle','--use-angle=swiftshader','--enable-unsafe-swiftshader',
          '--ignore-gpu-blocklist','--enable-webgl','--hide-scrollbars',`--window-size=${W},${H}`]
  });
  const page = await browser.newPage();
  await page.setViewport({width:W,height:H,deviceScaleFactor:1});
  page.on('console',m=>{ if(m.type()==='error') console.log('[page error]',m.text()); });
  await page.goto(url,{waitUntil:'load',timeout:60000});

  // wait for model ready (or error)
  await page.waitForFunction('window.__ready===true || window.__err!==null',{timeout:60000});
  const err = await page.evaluate('window.__err');
  if(err){ console.error('[page] model load failed:',err); await browser.close(); server.close(); process.exit(1); }

  for(let i=0;i<FRAMES;i++){
    await page.evaluate('window.__frame()');
    await page.screenshot({path:path.join(framesDir,String(i).padStart(5,'0')+'.png'),clip:{x:0,y:0,width:W,height:H}});
    if(i%4===0 || i===FRAMES-1) console.log(`PROGRESS ${i+1} ${FRAMES}`);  // line-delimited for the GUI
  }
  await browser.close(); server.close();

  console.log('[ffmpeg] encoding looping mp4...');
  // Write to a temp file and atomically rename on success so an interrupted/failed
  // render (kill, crash, ffmpeg error) never leaves a truncated mp4 at the cache path.
  // A partial cache file is silently reused by apply.sh and can corrupt playback
  // (e.g. a broken surface letting a neighboring monitor's wallpaper bleed through).
  const TMP = OUT + '.tmp-' + process.pid + '.mp4';
  try {
    execFileSync('ffmpeg',['-y','-loglevel','error','-nostats','-framerate',String(FPS),'-i',path.join(framesDir,'%05d.png'),
      '-c:v','libx264','-pix_fmt','yuv420p','-crf','18','-preset','medium',
      '-movflags','+faststart',TMP],{stdio:['ignore','ignore','inherit']});
    fs.renameSync(TMP, OUT);   // atomic on the same filesystem
  } catch(e){
    fs.rmSync(TMP,{force:true});   // never leave a partial temp file behind
    throw e;
  } finally {
    fs.rmSync(framesDir,{recursive:true,force:true});   // drop this run's intermediate PNGs
  }
  console.log('[done]',OUT);
})().catch(e=>{console.error(e);process.exit(1);});
