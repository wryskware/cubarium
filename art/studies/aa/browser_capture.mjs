// Local study browser only. No HTTP server, live cube, shim, or existing browser profile.
import {spawn} from 'node:child_process';
import {writeFile, mkdir} from 'node:fs/promises';
import {resolve, join} from 'node:path';
import {pathToFileURL} from 'node:url';
import assert from 'node:assert/strict';
const dir=resolve(process.argv[2]??'');
assert(process.argv[2], 'browser_capture.mjs STUDY_OUTPUTDIR');
const profile=join(dir,'browser-profile');
await mkdir(profile); // Never adopt an existing browser profile.
const child=spawn('chromium',['--headless','--no-sandbox','--disable-gpu','--disable-dev-shm-usage',
  '--remote-debugging-port=0','--window-size=1700,1600',`--user-data-dir=${profile}`,pathToFileURL(join(dir,'viewer.html')).href],{stdio:['ignore','ignore','pipe']});
let ws;
try {
  const endpoint=await new Promise((res,rej)=>{
    let text='';const timeout=setTimeout(()=>rej(Error('DevTools startup timeout')),15000);
    child.stderr.on('data',d=>{text+=d;const m=text.match(/DevTools listening on (ws:\/\/[^\s]+)/);if(m){clearTimeout(timeout);res(m[1]);}});
    child.on('exit',code=>{clearTimeout(timeout);rej(Error(`browser exited ${code}`));});
  });
  const url=new URL(endpoint);
  const targets=await (await fetch(`http://${url.host}/json/list`)).json();
  const page=targets.find(t=>t.type==='page');assert(page);
  ws=new WebSocket(page.webSocketDebuggerUrl);
  await new Promise((res,rej)=>{ws.onopen=res;ws.onerror=rej;});
  let id=0;const pending=new Map();
  ws.onmessage=e=>{const m=JSON.parse(e.data);if(m.id){const h=pending.get(m.id);pending.delete(m.id);m.error?h.reject(Error(JSON.stringify(m.error))):h.resolve(m.result);}};
  const send=(method,params={})=>new Promise((resolve,reject)=>{const key=++id;pending.set(key,{resolve,reject});ws.send(JSON.stringify({id:key,method,params}));});
  const evaluate=async expression=>{const r=await send('Runtime.evaluate',{expression,awaitPromise:true,returnByValue:true});assert(!r.exceptionDetails,JSON.stringify(r.exceptionDetails));return r.result.value;};
  const meal=await evaluate(`Boolean(document.querySelector('#track'))`);
  await evaluate(meal ? `window.ready` : `Promise.all(views.map(v=>v.image.decode()))`);
  const cadence=await evaluate(`new Promise(resolve=>{const samples=[];let begin;function probe(t){begin??=t;samples.push([t,index]);if(t-begin<6000)requestAnimationFrame(probe);else resolve(samples);}requestAnimationFrame(probe);})`);
  const times=cadence.slice(1).map((v,i)=>v[0]-cadence[i][0]).sort((a,b)=>a-b);
  const report={samples:cadence.length,span_ms:cadence.at(-1)[0]-cadence[0][0],
    raf_fps:(cadence.length-1)*1000/(cadence.at(-1)[0]-cadence[0][0]),p95_ms:times[Math.floor(times.length*.95)],
    max_ms:times.at(-1),unique_frame_indices:new Set(cadence.map(v=>v[1])).size,
    scope:'Local headless Chromium viewer cadence, not physical display or whole-world renderer throughput.'};
  if(meal) {
    for(const offset of [-1,0,1,6,15,30]) {
      await evaluate(`(async()=>{playing=false;index=(first.elapsed-590)*3+${offset};await new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)));})()`);
      const shot=await send('Page.captureScreenshot',{format:'png',captureBeyondViewport:false});
      await writeFile(join(dir,`browser-meal-onset-${offset}.png`),Buffer.from(shot.data,'base64'),{flag:'wx'});
    }
  }
  for(const [scene,frame] of meal ? [] : [['rooted',0],['rooted',45],['rooted',90],['seam',45],['rim',45],['quiet',0]]){
    await evaluate(`(async()=>{playing=false;index=${frame};document.querySelector('#scene').value='${scene}';document.querySelector('#scene').onchange();await Promise.all(views.map(v=>v.image.decode()));await new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)));})()`);
    const shot=await send('Page.captureScreenshot',{format:'png',captureBeyondViewport:false});
    await writeFile(join(dir,`browser-${scene}-${frame}.png`),Buffer.from(shot.data,'base64'),{flag:'wx'});
  }
  if(await evaluate(`Boolean(document.querySelector('#scale'))`)) {
    for(const [scale,selection,frame] of [['juvenile','states',45],['adult','feed-to-bud',60],['adult','feed-to-bud',69],['adult','feed-to-bud',78],['juvenile','bud-to-move',69]]) {
      await evaluate(`(async()=>{playing=false;index=${frame};document.querySelector('#scale').value='${scale}';document.querySelector('#case').value='${selection}';document.querySelector('#scene').value='rooted';document.querySelector('#scene').onchange();await Promise.all(views.map(v=>v.image.decode()));await new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)));})()`);
      const shot=await send('Page.captureScreenshot',{format:'png',captureBeyondViewport:false});
      await writeFile(join(dir,`browser-${scale}-${selection}-${frame}.png`),Buffer.from(shot.data,'base64'),{flag:'wx'});
    }
  }
  await writeFile(join(dir,'browser-cadence.json'),JSON.stringify(report,null,2)+'\n',{flag:'wx'});
  console.log(report);
} finally {
  ws?.close();child.kill('SIGTERM');
}
