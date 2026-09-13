// Read-only diagnostic using an already-running isolated Chromium debugger.
// node scripts/viewer-cadence.mjs http://127.0.0.1:7393/ 15 9227
// Does not launch/attach to a personal browser or submit care commands.
const [pageUrl = 'http://127.0.0.1:7393/', secondsText = '15', portText = '9227'] = process.argv.slice(2);
const page = new URL(pageUrl);
const seconds = Number(secondsText), port = Number(portText);
if (page.protocol !== 'http:' || page.hostname !== '127.0.0.1' || page.username || page.password
    || !Number.isFinite(seconds) || seconds < 2 || seconds > 60
    || !Number.isInteger(port) || port < 1024 || port > 65535) {
  throw new Error('Expected a loopback HTTP viewer, 2–60 seconds, and a debugger port 1024–65535');
}
const debuggerUrl = `http://127.0.0.1:${port}`;
const target = await (await fetch(`${debuggerUrl}/json/new?${encodeURIComponent(pageUrl)}`, {method: 'PUT'})).json();
const socket = new WebSocket(target.webSocketDebuggerUrl);
await new Promise((resolve, reject) => { socket.onopen = resolve; socket.onerror = reject; });
let sequence = 0;
const pending = new Map();
socket.onmessage = ({data}) => {
  const message = JSON.parse(data), waiter = pending.get(message.id);
  if (!waiter) return;
  pending.delete(message.id);
  message.error ? waiter.reject(new Error(JSON.stringify(message.error))) : waiter.resolve(message.result);
};
function call(method, params = {}) {
  return new Promise((resolve, reject) => {
    const id = ++sequence;
    pending.set(id, {resolve, reject});
    socket.send(JSON.stringify({id, method, params}));
  });
}
try {
  await call('Runtime.enable');
  await call('Page.enable');
  await call('Emulation.setDeviceMetricsOverride', {width: 1400, height: 1000, deviceScaleFactor: 1, mobile: false});
  await new Promise(resolve => setTimeout(resolve, 1800));
  const expression = `(async () => {
    if (!document.getElementById('net')) throw new Error('Not a Cubarium viewer');
    const before = await (await fetch('/status', {cache:'no-store'})).json();
    const start = performance.now(), raf = [], draws = [], fetches = [];
    let active = true, newest = null;
    const arrayBuffer = Response.prototype.arrayBuffer;
    const drawImage = CanvasRenderingContext2D.prototype.drawImage;
    Response.prototype.arrayBuffer = async function (...args) {
      const value = await arrayBuffer.apply(this, args);
      if (active && new URL(this.url).pathname === '/frame' && value.byteLength === 61448) {
        newest = new DataView(value).getBigUint64(0, true).toString();
        fetches.push({t:performance.now(), seq:newest});
      }
      return value;
    };
    CanvasRenderingContext2D.prototype.drawImage = function (...args) {
      const result = drawImage.apply(this, args);
      if (active && this.canvas.id === 'net') draws.push({t:performance.now(), seq:newest});
      return result;
    };
    function sample(t) { if (active) { raf.push(t); requestAnimationFrame(sample); } }
    requestAnimationFrame(sample);
    try { await new Promise(resolve => setTimeout(resolve, ${seconds * 1000})); }
    finally {
      active = false;
      Response.prototype.arrayBuffer = arrayBuffer;
      CanvasRenderingContext2D.prototype.drawImage = drawImage;
    }
    const duration = (performance.now() - start)/1000;
    const after = await (await fetch('/status', {cache:'no-store'})).json();
    if (before.source.pid !== after.source.pid) throw new Error('Host restarted during measurement');
    function intervals(times) {
      const values = times.slice(1).map((t,i) => t-times[i]).sort((a,b)=>a-b);
      return {count:values.length, median_ms:values[Math.floor(values.length*.5)]??null,
        p95_ms:values[Math.floor(values.length*.95)]??null, max_ms:values.at(-1)??null,
        over_25ms:values.filter(x=>x>25).length};
    }
    const unique = draws.filter((d,i) => d.seq!==null && (i===0 || d.seq!==draws[i-1].seq));
    return {source:after.source,duration_seconds:duration,
      host_submitted_fps:(after.render_seq-before.render_seq)/duration,
      raf_fps:raf.length/duration,net_draw_fps:draws.length/duration,
      unique_net_frame_fps:unique.length/duration,frame_responses:fetches.length,
      raf_intervals:intervals(raf),new_net_frame_intervals:intervals(unique.map(x=>x.t)),
      note:'Isolated headless Chromium callback/canvas measurements, not physical scanout or personal-browser pacing. Instrumentation adds overhead; source/status boundaries have HTTP latency.'};
  })()`;
  const result = await call('Runtime.evaluate', {expression, awaitPromise: true, returnByValue: true});
  if (result.exceptionDetails) throw new Error(JSON.stringify(result.exceptionDetails));
  console.log(JSON.stringify(result.result.value, null, 2));
} finally {
  socket.close();
  await fetch(`${debuggerUrl}/json/close/${target.id}`);
}
