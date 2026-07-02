// Batch thumbnail generator for extracted Live2D skins (frame-0 composed render).
// Usage: node thumb.js --parent <Live2DOutput> --out <pngDir> [--w 640 --h 360] <name> [<name> ...]
const http=require('http'), fs=require('fs'), path=require('path'), puppeteer=require('puppeteer');
function arg(n,d){const i=process.argv.indexOf('--'+n);return i>=0?process.argv[i+1]:d;}
const PARENT=path.resolve(arg('parent',''));
const OUT=path.resolve(arg('out','thumbs'));
const W=+arg('w',640),H=+arg('h',360),OY=+arg('oy',0.55),WARMUP=+arg('frames',20);
// positional args (skin names) are everything that isn't a --flag or its value
const NAMES=[]; for(let i=2;i<process.argv.length;i++){const a=process.argv[i]; if(a.startsWith('--')){i++;continue;} NAMES.push(a);}
const WALL=__dirname;
const MIME={'.js':'text/javascript','.html':'text/html','.json':'application/json','.png':'image/png'};
function send(res,fp){if(!fs.existsSync(fp)){res.writeHead(404);res.end();return;}res.writeHead(200,{'Content-Type':MIME[path.extname(fp)]||'application/octet-stream','Access-Control-Allow-Origin':'*'});fs.createReadStream(fp).pipe(res);}
const server=http.createServer((req,res)=>{const u=decodeURIComponent(req.url.split('?')[0]);
  if(u==='/'||u==='/index.html')return send(res,path.join(WALL,'index.html'));
  if(u.startsWith('/vendor/'))return send(res,path.join(WALL,u));
  if(u.startsWith('/m/'))return send(res,path.join(PARENT,u.slice(3)));
  res.writeHead(404);res.end();});
(async()=>{
  fs.mkdirSync(OUT,{recursive:true});
  await new Promise(r=>server.listen(0,r)); const port=server.address().port;
  const browser=await puppeteer.launch({headless:true,args:['--no-sandbox','--use-gl=angle','--use-angle=swiftshader','--enable-unsafe-swiftshader','--ignore-gpu-blocklist','--enable-webgl']});
  const page=await browser.newPage(); await page.setViewport({width:W,height:H,deviceScaleFactor:1});
  let ok=0,fail=0;
  for(const name of NAMES){
    const m3=fs.existsSync(path.join(PARENT,name))?fs.readdirSync(path.join(PARENT,name)).find(f=>f.endsWith('.model3.json')):null;
    if(!m3){console.log('skip(no model):',name);fail++;continue;}
    const url=`http://127.0.0.1:${port}/?w=${W}&h=${H}&oy=${OY}&model=/m/${encodeURIComponent(name)}/${encodeURIComponent(m3)}`;
    try{
      await page.goto(url,{waitUntil:'load',timeout:45000});
      await page.waitForFunction('window.__ready===true||window.__err!==null',{timeout:45000});
      const err=await page.evaluate('window.__err'); if(err){console.log('fail:',name,err);fail++;continue;}
      // advance frames for a livelier pose (and past any fade-in)
      for(let k=0;k<WARMUP;k++) await page.evaluate('window.__frame()');
      await page.screenshot({path:path.join(OUT,name+'.png'),clip:{x:0,y:0,width:W,height:H}});
      ok++; process.stdout.write(`\r[thumb] ${ok+fail}/${NAMES.length}  `);
    }catch(e){console.log('err:',name,e.message);fail++;}
  }
  process.stdout.write(`\n[thumb] done ok=${ok} fail=${fail} -> ${OUT}\n`);
  await browser.close(); server.close();
})().catch(e=>{console.error(e);process.exit(1);});
