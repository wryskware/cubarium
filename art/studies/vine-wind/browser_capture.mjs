// Own local Chromium only; no server or live cube connection.
import {spawn} from 'node:child_process';
import {mkdir,writeFile} from 'node:fs/promises';
import {resolve,join} from 'node:path';
import {pathToFileURL} from 'node:url';
import assert from 'node:assert/strict';
const dir=resolve(process.argv[2]);const profile=join(dir,`vine-browser-profile-${Date.now()}`);await mkdir(profile);
const child=spawn('chromium',['--headless','--no-sandbox','--disable-gpu','--disable-dev-shm-usage','--remote-debugging-port=0','--window-size=1650,1100',`--user-data-dir=${profile}`,pathToFileURL(join(dir,'viewer.html')).href],{stdio:['ignore','ignore','pipe']});
let ws;
try{
 const endpoint=await new Promise((resolve,reject)=>{let s='';const timeout=setTimeout(()=>reject(Error('startup timeout')),15000);child.stderr.on('data',d=>{s+=d;const m=s.match(/DevTools listening on (ws:\/\/\S+)/);if(m){clearTimeout(timeout);resolve(m[1]);}});child.on('exit',c=>reject(Error(`Chromium ${c}`)));});
 const targets=await(await fetch(`http://${new URL(endpoint).host}/json/list`)).json();ws=new WebSocket(targets.find(t=>t.type==='page').webSocketDebuggerUrl);await new Promise((r,j)=>{ws.onopen=r;ws.onerror=j;});
 let id=0;const pending=new Map();ws.onmessage=e=>{const m=JSON.parse(e.data);if(m.id){const p=pending.get(m.id);pending.delete(m.id);m.error?p.reject(Error(JSON.stringify(m.error))):p.resolve(m.result);}};
 const send=(method,params={})=>new Promise((resolve,reject)=>{const key=++id;pending.set(key,{resolve,reject});ws.send(JSON.stringify({id:key,method,params}));});
 const evaluate=async expression=>{const r=await send('Runtime.evaluate',{expression,awaitPromise:true,returnByValue:true});assert(!r.exceptionDetails,JSON.stringify(r.exceptionDetails));return r.result.value;};
 await evaluate('window.ready');
 const cadence=await evaluate('new Promise(resolve=>{const a=[];let begin;function probe(t){begin??=t;a.push([t,index]);if(t-begin<6000)requestAnimationFrame(probe);else resolve(a);}requestAnimationFrame(probe);})');
 for(const [scene,mode,index] of [['interior','full',480],['interior','full',1199],['interior','growth',1100],['vertex','full',480],['vertex','growth',1183],['right','full',480]]){
  await evaluate(`(async()=>{playing=false;index=${index};document.querySelector('#scene').value='${scene}';document.querySelector('#mode').value='${mode}';await select();paint();})()`);
  const shot=await send('Page.captureScreenshot',{format:'png'});await writeFile(join(dir,`browser-${scene}-${mode}-${index}.png`),Buffer.from(shot.data,'base64'),{flag:'wx'});
 }
 const span=cadence.at(-1)[0]-cadence[0][0];const report={samples:cadence.length,span_ms:span,raf_fps:(cadence.length-1)*1000/span,unique_frames:new Set(cadence.map(x=>x[1])).size,scope:'Local preloaded PNG browser playback only, not renderer throughput or physical display'};
 await writeFile(join(dir,'browser-cadence.json'),JSON.stringify(report,null,2),{flag:'wx'});console.log(report);
}finally{ws?.close();child.kill('SIGTERM');}
