#!/usr/bin/env python3
"""Generate a browser playback page for rain-response review captures
(art/studies/rain-response-review). The page plays the frame streams of several
captures side by side at 60 fps (3 frames per 20 Hz tick), native 256×128 nets and a
4× nearest-neighbour crop, with pause, step, speed and a stream toggle. It is a
generated artifact written next to the captures; open it with a browser from `file://`.
It does not judge anything: it exists so a reviewer can watch, which stills cannot show.

  playback.py OUT.html --title T --column LABEL=CAPTURE_DIR/STREAM ... --from A --to B
              [--face F --cx CX --cy CY --radius R]

Paths are written relative to OUT.html's directory.
"""
import argparse
import html
import json
import os
import sys

FACE_ORIGIN = {4: (64, 0), 3: (0, 64), 0: (64, 64), 1: (128, 64), 2: (192, 64)}
FPT = 3

PAGE = """<!doctype html>
<meta charset="utf-8">
<title>__TITLE__</title>
<style>
 body{background:#141418;color:#ddd;font:13px system-ui;margin:12px}
 .row{display:flex;gap:14px;align-items:flex-start;flex-wrap:wrap}
 .col{display:flex;flex-direction:column;gap:4px}
 canvas{image-rendering:pixelated;image-rendering:crisp-edges;background:#000}
 button{margin-right:6px}
 .hud{margin:8px 0;color:#aab}
</style>
<h3>__TITLE__</h3>
<div class="hud">
 <button id="play">pause</button><button id="step">step</button>
 speed <select id="speed"><option>1</option><option>0.5</option><option>0.25</option><option>0.1</option></select>
 zoom <select id="zoom"><option>4</option><option>6</option><option>8</option></select>
 <span id="pos"></span>
</div>
<div class="row" id="native"></div>
<div class="row" id="zoomed"></div>
<p class="hud">Streams: __STREAMS__. Frames __FROM__–__TO__ (ticks), 3 per tick at 60 fps. Crop __CROP__.
Native panels are the 256×128 net (Top at 64,0; Left 0,64; Front 64,64; Right 128,64; Back 192,64).</p>
<script>
const M = __MANIFEST__;
const nat = document.getElementById('native'), zoomed = document.getElementById('zoomed');
const cols = M.columns.map(c => {
  const a = document.createElement('div'); a.className='col';
  const la = document.createElement('div'); la.textContent = c.label; a.appendChild(la);
  const cn = document.createElement('canvas'); cn.width=256; cn.height=128; cn.style.width='512px'; cn.style.height='256px'; a.appendChild(cn);
  nat.appendChild(a);
  const b = document.createElement('div'); b.className='col';
  const lb = document.createElement('div'); lb.textContent = c.label + ' crop'; b.appendChild(lb);
  const cz = document.createElement('canvas'); b.appendChild(cz);
  zoomed.appendChild(b);
  return {c, cn, cz, imgs: []};
});
const N = M.frames.length;
let i = 0, playing = true, acc = 0, last = performance.now();
const speed = document.getElementById('speed'), zoom = document.getElementById('zoom'), pos = document.getElementById('pos');
function img(col, k){ if(!col.imgs[k]){ const im = new Image(); im.src = col.c.dir + '/' + M.frames[k]; col.imgs[k]=im; } return col.imgs[k]; }
function draw(){
  const z = +zoom.value, cr = M.crop;
  for(const col of cols){
    const im = img(col, i); if(!im.complete) continue;
    col.cn.getContext('2d').drawImage(im, 0, 0);
    if(cr){ col.cz.width = cr.w*z; col.cz.height = cr.h*z; const g = col.cz.getContext('2d'); g.imageSmoothingEnabled=false; g.drawImage(im, cr.x, cr.y, cr.w, cr.h, 0, 0, cr.w*z, cr.h*z); }
  }
  pos.textContent = 'frame ' + M.frames[i] + '  tick ' + (M.from + Math.floor(i/3)) + '.' + (i%3) + '  (' + (i/60).toFixed(2) + ' s from window start)';
}
function tick(now){
  if(playing){ acc += (now - last) * +speed.value; while(acc >= 1000/60){ acc -= 1000/60; i = (i + 1) % N; } }
  last = now; draw(); requestAnimationFrame(tick);
}
for(let k=0;k<N;k++) for(const col of cols) img(col,k);
document.getElementById('play').onclick = e => { playing = !playing; e.target.textContent = playing ? 'pause' : 'play'; };
document.getElementById('step').onclick = () => { playing = false; document.getElementById('play').textContent='play'; i = (i + 1) % N; draw(); };
requestAnimationFrame(tick);
</script>
"""


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("out")
    ap.add_argument("--title", required=True)
    ap.add_argument("--column", action="append", required=True, help="LABEL=DIR/STREAM")
    ap.add_argument("--from", dest="lo", type=int, required=True)
    ap.add_argument("--to", dest="hi", type=int, required=True)
    ap.add_argument("--face", type=int)
    ap.add_argument("--cx", type=int)
    ap.add_argument("--cy", type=int)
    ap.add_argument("--radius", type=int, default=12)
    a = ap.parse_args()
    base = os.path.dirname(os.path.abspath(a.out))
    columns = []
    for spec in a.column:
        label, path = spec.split("=", 1)
        columns.append({"label": label, "dir": os.path.relpath(os.path.abspath(path), base)})
    frames = [f"frame_{n:05}.png" for n in range(a.lo * FPT, a.hi * FPT + FPT)]
    crop = None
    if a.face is not None:
        ox, oy = FACE_ORIGIN[a.face]
        u, v = a.cx * 4 + 2, a.cy * 4 + 2
        x0, y0 = max(0, u - a.radius), max(0, v - a.radius)
        x1, y1 = min(64, u + a.radius), min(64, v + a.radius)
        crop = {"x": ox + x0, "y": oy + y0, "w": x1 - x0, "h": y1 - y0}
    manifest = {"columns": columns, "frames": frames, "from": a.lo, "crop": crop}
    page = (PAGE.replace("__TITLE__", html.escape(a.title)).replace("__MANIFEST__", json.dumps(manifest))
            .replace("__STREAMS__", html.escape(", ".join(f"{c['label']} = {c['dir']}" for c in columns)))
            .replace("__FROM__", str(a.lo)).replace("__TO__", str(a.hi))
            .replace("__CROP__", html.escape(json.dumps(crop))))
    with open(a.out, "w") as f:
        f.write(page)
    print("playback page", a.out)


if __name__ == "__main__":
    sys.exit(main())
