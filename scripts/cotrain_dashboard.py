#!/usr/bin/env python3
"""Cubarium co-training progress dashboard.

Reads runs/dashboard/sources.txt (one run per line: `name host path log`), copies each
run's generations.jsonl and its `w ...` window log lines into runs/dashboard/cache/<name>/
(remote hosts are only ever READ: ssh stat / ssh cat), and writes one self-contained
runs/dashboard/index.html (data embedded, auto-refresh 60 s, works from file://).

  scripts/cotrain_dashboard.py            sync, then render
  scripts/cotrain_dashboard.py --no-sync  render from the cache only
  scripts/cotrain_dashboard.py --sync-only

A source that fails to sync keeps its cached copy and shows as stale.
"""
import argparse, json, math, os, re, shlex, subprocess, sys, time, urllib.request
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DASH = ROOT / "runs" / "dashboard"
CACHE = DASH / "cache"
LINEAGES = ["littershredder", "frondgrazer", "lanternjaw"]
CHARTJS_URL = "https://cdnjs.cloudflare.com/ajax/libs/Chart.js/4.4.1/chart.umd.min.js"
AWS_KEY = os.path.expanduser("~/.ssh/cubarium-aws.pem")
W_RE = re.compile(r"^\s*w ")
KV_RE = re.compile(r"(\w+)=([-+0-9.eE]+|nan|inf)")
WIN_RE = re.compile(r"^\s*w (\d+)\.(\d+)\s+step (\d+)\s+(\d+)s\s+worlds (\d+)\s*\|(.*)$")


# ---------------------------------------------------------------- sources / sync
def read_sources(path):
    out = []
    for raw in path.read_text().splitlines():
        line = raw.split("#", 1)[0].strip()
        if not line:
            continue
        parts = line.split()
        if len(parts) < 4:
            print(f"sources: skipping malformed line: {raw!r}", file=sys.stderr)
            continue
        name, host, rpath, log = parts[:4]
        out.append(dict(name=name, host=host, path=rpath.rstrip("/"), log=log))
    return out


def ssh_cmd(host):
    base = ["ssh", "-o", "BatchMode=yes", "-o", "ConnectTimeout=10",
            "-o", "ServerAliveInterval=15", "-o", "ServerAliveCountMax=4"]
    if host.startswith("aws:"):
        return base + ["-i", AWS_KEY, "-o", "StrictHostKeyChecking=accept-new",
                       "ec2-user@" + host[4:]]
    return base + [host]


def rpath(p):
    """Remote path: ssh runs in the home dir, so relative paths work as they are."""
    return p[2:] if p.startswith("~/") else p


def local_resolve(p):
    q = Path(os.path.expanduser(p))
    if q.is_absolute():
        return q
    for base in (Path.home(), ROOT):
        if (base / q).exists():
            return base / q
    return Path.home() / q


def remote_read(host, args, timeout):
    """Run a read-only command (stat / cat) on a remote host."""
    cmd = ssh_cmd(host) + [" ".join(shlex.quote(a) for a in args)]
    r = subprocess.run(cmd, capture_output=True, timeout=timeout)
    return r


def stat_files(src, paths):
    """-> {path: (size, mtime)} for the files that exist."""
    res = {}
    if src["host"] == "local":
        for p in paths:
            try:
                st = local_resolve(p).stat()
                res[p] = (st.st_size, int(st.st_mtime))
            except OSError:
                pass
        return res
    r = remote_read(src["host"], ["stat", "-c", "%s %Y %n", "--"] + [rpath(p) for p in paths], 60)
    back = {rpath(p): p for p in paths}
    for line in r.stdout.decode(errors="replace").splitlines():
        bits = line.split(" ", 2)
        if len(bits) == 3 and bits[2] in back:
            res[back[bits[2]]] = (int(bits[0]), int(bits[1]))
    if r.returncode != 0 and not res:
        raise RuntimeError("ssh/stat failed: " + r.stderr.decode(errors="replace").strip()[-200:])
    return res


def fetch(src, path):
    if src["host"] == "local":
        return local_resolve(path).read_bytes()
    r = remote_read(src["host"], ["cat", "--", rpath(path)], 600)
    if r.returncode != 0:
        raise RuntimeError("cat failed: " + r.stderr.decode(errors="replace").strip()[-200:])
    return r.stdout


def atomic_write(path, data):
    tmp = path.with_suffix(path.suffix + ".tmp")
    tmp.write_bytes(data)
    os.replace(tmp, path)


def sync_one(src):
    d = CACHE / src["name"]
    d.mkdir(parents=True, exist_ok=True)
    mp = d / "meta.json"
    try:
        meta = json.loads(mp.read_text())
    except Exception:
        meta = {}
    jpath = src["path"] + "/generations.jsonl"
    meta["last_attempt"] = int(time.time())
    try:
        st = stat_files(src, [jpath, src["log"]])
        if jpath not in st:
            raise RuntimeError("generations.jsonl not found at " + jpath)
        for key, p, outname in (("jsonl", jpath, "generations.jsonl"), ("log", src["log"], "w-lines.log")):
            if p not in st:
                meta[key] = None  # log may not exist (yet); keep any cached copy
                continue
            size, mtime = st[p]
            old = meta.get(key) or {}
            if old.get("size") == size and old.get("mtime") == mtime and (d / outname).exists():
                continue
            data = fetch(src, p)
            if key == "log":
                lines = [l for l in data.decode(errors="replace").splitlines() if W_RE.match(l)]
                data = ("\n".join(lines) + ("\n" if lines else "")).encode()
            atomic_write(d / outname, data)
            meta[key] = {"size": size, "mtime": mtime}
        meta["synced"] = int(time.time())
        meta["error"] = None
    except Exception as e:  # keep cached copy, report stale
        meta["error"] = f"{type(e).__name__}: {e}"[:300]
        print(f"sync {src['name']}: {meta['error']}", file=sys.stderr)
    mp.write_text(json.dumps(meta))
    return meta


# ---------------------------------------------------------------- summarise
def num(x):
    return x if isinstance(x, (int, float)) and not isinstance(x, bool) and math.isfinite(x) else None


def mean(xs):
    xs = [x for x in (num(v) for v in xs) if x is not None]
    return sum(xs) / len(xs) if xs else None


def median(xs):
    xs = sorted(x for x in (num(v) for v in xs) if x is not None)
    if not xs:
        return None
    n = len(xs)
    return xs[n // 2] if n % 2 else (xs[n // 2 - 1] + xs[n // 2]) / 2


def readout(ev, key):
    for item in ev.get("readout_mean") or []:
        if isinstance(item, (list, tuple)) and len(item) == 2 and item[0] == key:
            return num(item[1])
    return None


def screen_of(c):
    s = c.get("screen")
    if s in ("inside", "margin", "refused"):
        return s
    return "refused" if c.get("screened_out") else "inside"


def per_lineage(evs, field):
    out = []
    for i in range(3):
        out.append(mean([(e.get(field) or [None] * 3)[i] if len(e.get(field) or []) > i else None for e in evs]))
    return out


def summarise_gen(g):
    cands = g.get("candidates") or []
    plain = [c for c in cands if c.get("label") != "default"]
    default = next((c for c in cands if c.get("label") == "default"), None)
    ran = [c for c in plain if not c.get("screened_out") and c.get("evaluations") and num(c.get("fitness")) is not None]
    best = min(ran, key=lambda c: c["fitness"]) if ran else None
    counts = {"inside": 0, "margin": 0, "refused": 0}
    for c in plain:
        counts[screen_of(c)] += 1
    row = dict(
        g=g.get("generation"), wall_s=num(g.get("wall_s")), elapsed_s=num(g.get("elapsed_s")),
        sigma=num(g.get("sigma")), n_cands=len(plain), n_ran=len(ran), screen=counts,
        best=num(best["fitness"]) if best else None,
        median=median([c["fitness"] for c in ran]),
        default=num(default.get("fitness")) if default else None,
        best_label=best.get("label") if best else None,
        pen=None, lin=None,
    )
    if best:
        evs = best["evaluations"]
        row["pen"] = {k: mean([(e.get("penalty") or {}).get(k) for e in evs])
                      for k in ("bound", "peak", "lively", "share", "persist", "total")}
        lin = {}
        alive = per_lineage(evs, "lineages_end")
        late = per_lineage(evs, "late_births")
        ready = per_lineage(evs, "readiness")
        for i, name in enumerate(LINEAGES):
            lin[name] = dict(
                alive=alive[i], late=late[i], ready=ready[i],
                moving=mean([readout(e, name + ".moving_share") for e in evs]),
                starved=mean([readout(e, name + ".starved_per_h") for e in evs]))
        row["lin"] = lin
    return row


def parse_window_line(line):
    m = WIN_RE.match(line)
    if not m:
        return None
    g, w, step, wall, worlds, rest = m.groups()
    lin = {}
    for seg in rest.split("|"):
        seg = seg.strip()
        if not seg:
            continue
        name = seg.split()[0]
        kv = {k: float(v) for k, v in KV_RE.findall(seg)}
        lin[name] = dict(n=kv.get("n"), twin=kv.get("twin"), pair=kv.get("pair"),
                         ratio=kv.get("ratio"), rms=kv.get("rms"))
    return dict(g=int(g), w=int(w), step=int(step), wall=float(wall), worlds=int(worlds), lin=lin)


def summarise(src, meta):
    d = CACHE / src["name"]
    gens, windows = [], {}
    jp = d / "generations.jsonl"
    if jp.exists():
        with open(jp, "r", errors="replace") as f:
            for line in f:
                try:
                    g = json.loads(line)
                except ValueError:
                    continue  # partial last line while the run is writing
                gens.append(summarise_gen(g))
                for w in g.get("windows") or []:
                    lin = {}
                    for L in w.get("lineages") or []:
                        t, p = num(L.get("twin_diff")), num(L.get("pair_spread"))
                        lin[L.get("lineage")] = dict(n=L.get("bodies"), twin=t, pair=p,
                                                     ratio=(t / p if t is not None and p else None),
                                                     rms=num(L.get("update_rms")))
                    windows[(w.get("generation"), w.get("window"))] = dict(
                        g=w.get("generation"), w=w.get("window"), step=w.get("brain_step"),
                        wall=num(w.get("wall_s")), worlds=len(w.get("world_keys") or []), lin=lin)
    lp = d / "w-lines.log"
    if lp.exists():
        for line in lp.read_text(errors="replace").splitlines():
            w = parse_window_line(line)
            if w:
                windows[(w["g"], w["w"])] = w
    wins = [windows[k] for k in sorted(windows)]
    walls = [g["wall_s"] for g in gens if g["wall_s"]]
    mt = [m["mtime"] for m in (meta.get("jsonl"), meta.get("log")) if m]
    return dict(
        name=src["name"], host=src["host"], path=src["path"],
        gens=gens, windows=wins,
        min_per_gen=(sum(walls) / len(walls) / 60 if walls else None),
        last_update=max(mt) if mt else None,
        jsonl_mtime=(meta.get("jsonl") or {}).get("mtime"),
        synced=meta.get("synced"), error=meta.get("error"),
        n_windows_per_gen=(max((w["w"] for w in wins), default=-1) + 1) if wins else None,
    )


# ---------------------------------------------------------------- render
def chartjs_source():
    p = CACHE / "chart.umd.min.js"
    if not p.exists():
        try:
            with urllib.request.urlopen(CHARTJS_URL, timeout=20) as r:
                atomic_write(p, r.read())
        except Exception as e:
            print(f"chart.js download failed ({e}); page will load it from cdnjs", file=sys.stderr)
            return None
    return p.read_text()


def render(runs):
    js = chartjs_source()
    if js:
        tag = "<script>" + js.replace("</script", "<\\/script") + "</script>"
    else:
        tag = f'<script src="{CHARTJS_URL}"></script>'
    data = json.dumps(dict(generated=int(time.time()), runs=runs), allow_nan=False)
    data = data.replace("</", "<\\/")
    html = TEMPLATE.replace("__CHARTJS__", tag).replace("__DATA__", data)
    out = DASH / "index.html"
    atomic_write(out, html.encode())
    return out


TEMPLATE = r"""<!doctype html>
<html lang="en"><head><meta charset="utf-8">
<meta http-equiv="refresh" content="60">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>Cubarium co-training</title>
<style>
:root{--bg:#f6f7f9;--card:#fff;--fg:#1d2330;--mute:#667085;--line:#e3e6ec;--ok:#1a8f55;--warn:#b7791f;--bad:#c0392b}
@media (prefers-color-scheme:dark){:root{--bg:#12151c;--card:#1b2029;--fg:#e4e8f0;--mute:#8f99ab;--line:#2c3340;--ok:#4cc38a;--warn:#e3b341;--bad:#f0746a}}
*{box-sizing:border-box}
body{margin:0;background:var(--bg);color:var(--fg);font:13px/1.4 system-ui,sans-serif;padding:12px 16px 40px}
h1{font-size:17px;margin:0}h2{font-size:14px;margin:18px 0 6px}
header{display:flex;gap:12px;align-items:baseline;flex-wrap:wrap;margin-bottom:8px}
.mute{color:var(--mute)}
table{border-collapse:collapse;width:100%;background:var(--card);border:1px solid var(--line);border-radius:6px;overflow:hidden}
th,td{padding:5px 9px;text-align:right;border-bottom:1px solid var(--line);white-space:nowrap}
th{font-weight:600;color:var(--mute);font-size:11px;text-transform:uppercase;letter-spacing:.03em}
th:first-child,td:first-child,td.l,th.l{text-align:left}
tr:last-child td{border-bottom:0}
.chip{display:inline-block;padding:1px 8px;border-radius:9px;font-size:11px;font-weight:600;border:1px solid currentColor}
.live{color:var(--ok)}.stale{color:var(--warn)}.dead{color:var(--bad)}
.neg{color:var(--ok)}.pos{color:var(--bad)}
.grid{display:grid;gap:8px;grid-template-columns:repeat(auto-fill,minmax(270px,1fr))}
.grid.wide{grid-template-columns:repeat(auto-fill,minmax(420px,1fr))}
.card{background:var(--card);border:1px solid var(--line);border-radius:6px;padding:6px 8px 4px}
.card h3{margin:0 0 2px;font-size:11.5px;font-weight:600;color:var(--mute)}
.box{position:relative;height:165px}.wide .box{height:230px}
details{margin-top:14px}
summary{cursor:pointer;font-size:14px;font-weight:600;padding:4px 0;display:flex;gap:10px;align-items:center;flex-wrap:wrap}
summary .mute{font-weight:400;font-size:12px}
a{color:inherit}
</style></head><body>
<header><h1>Cubarium co-training</h1><span class="mute" id="stamp"></span>
<span class="mute">lower fitness is better &middot; best/median over candidates that ran, default = shipped defaults</span></header>
<table id="status"></table>
<h2>All runs</h2>
<div class="grid wide" id="global"></div>
<div id="runs"></div>
__CHARTJS__
<script>
const DATA = __DATA__;
const css = n => getComputedStyle(document.documentElement).getPropertyValue(n).trim();
const FG = css('--fg'), MUTE = css('--mute'), GRID = css('--line');
Chart.defaults.color = MUTE; Chart.defaults.borderColor = GRID; Chart.defaults.scale.grid.color = GRID;
Chart.defaults.font.size = 11; Chart.defaults.animation = false;
Chart.defaults.maintainAspectRatio = false;
try{matchMedia('(prefers-color-scheme: dark)').addEventListener('change',()=>location.reload())}catch(e){}

const RUNCOL = ['#3b82c4','#e0701a','#2a9d6f','#c2578f','#d9a21b','#4fb3d9','#8a6fd1','#8d8d8d'];
const LINCOL = {littershredder:'#d08a2e', frondgrazer:'#3aa66a', lanternjaw:'#d2554f'};
const LINSHORT = {littershredder:'shredder', frondgrazer:'grazer', lanternjaw:'jaw'};
const LINS = ['littershredder','frondgrazer','lanternjaw'];
const now = Date.now()/1000;

function fmtAge(s){ if(s==null) return '–'; s=Math.max(0,Math.round(s));
  if(s<90) return s+'s'; if(s<5400) return Math.round(s/60)+'m';
  if(s<172800) return (s/3600).toFixed(1)+'h'; return (s/86400).toFixed(1)+'d'; }
const f = (x,d=2)=> x==null?'–':Number(x).toFixed(d);
const sgn = (x,d=1)=> x==null?'–':(x>0?'+':'')+Number(x).toFixed(d);
const cls = x => x==null?'':(x<0?'neg':(x>0?'pos':''));
function state(r){ const age = r.last_update==null?null:now-r.last_update;
  if(r.error) return ['stale','stale (sync failed)',r.error];
  if(age==null) return ['dead','no data',''];
  if(age>900) return ['stale','stale','no new output for '+fmtAge(age)];
  return ['live','live','']; }

document.getElementById('stamp').textContent = 'page built ' + new Date(DATA.generated*1000).toLocaleTimeString();

// ---- status table
(function(){
  let h = '<tr><th class="l">Run</th><th class="l">Host</th><th>Gens done</th><th>In progress</th><th>Last update</th><th>Min / gen</th><th>Best</th><th>Default</th><th>Best &minus; def</th><th>Median &minus; def</th><th class="l">State</th></tr>';
  for(const r of DATA.runs){
    const g = r.gens[r.gens.length-1], w = r.windows[r.windows.length-1];
    const done = g? g.g+1 : 0;
    let prog = '–';
    if(w && (!g || w.g > g.g)) prog = 'gen '+w.g+' w '+w.w+(r.n_windows_per_gen?'/'+r.n_windows_per_gen:'');
    const bd = g&&g.best!=null&&g.default!=null? g.best-g.default : null;
    const md = g&&g.median!=null&&g.default!=null? g.median-g.default : null;
    const [c,t,tip] = state(r);
    const age = r.last_update==null?null:now-r.last_update;
    h += `<tr><td class="l"><a href="#run-${r.name}"><b>${r.name}</b></a></td><td class="l mute">${r.host}</td><td>${done}</td><td>${prog}</td>`
      + `<td title="synced ${r.synced?fmtAge(now-r.synced)+' ago':'never'}">${fmtAge(age)} ago</td><td>${r.min_per_gen==null?'–':f(r.min_per_gen,1)}</td>`
      + `<td>${g?f(g.best,1):'–'}</td><td>${g?f(g.default,1):'–'}</td><td class="${cls(bd)}">${sgn(bd)}</td><td class="${cls(md)}">${sgn(md)}</td>`
      + `<td class="l"><span class="chip ${c}" title="${tip.replace(/"/g,'&quot;')}">${t}</span></td></tr>`;
  }
  document.getElementById('status').innerHTML = h;
})();

// ---- chart helpers
function ds(label,color,pts,o){ return Object.assign({label,borderColor:color,backgroundColor:color,data:pts,
  borderWidth:1.6,pointRadius:pts.length>40?0:2.2,pointHoverRadius:4,tension:0,spanGaps:true},o||{}); }
function card(parent,title,wide){ const c=document.createElement('div'); c.className='card';
  c.innerHTML='<h3></h3><div class="box"><canvas></canvas></div>'; c.firstChild.textContent=title;
  parent.appendChild(c); return c.querySelector('canvas'); }
function chart(parent,title,datasets,o){ o=o||{};
  const cv=card(parent,title);
  const legend = o.legend===false?{display:false}:{position:'bottom',align:'start',labels:Object.assign({boxWidth:10,boxHeight:2,padding:6},o.legendLabels||{})};
  new Chart(cv,{type:o.type||'line',data:{datasets},options:{
    interaction:{mode:'x',intersect:false},
    plugins:{legend,tooltip:{itemSort:(a,b)=>b.parsed.y-a.parsed.y}},
    scales:{x:{type:'linear',min:0,suggestedMax:1,offset:o.type==='bar',stacked:o.stacked,title:{display:!!o.xTitle,text:o.xTitle},
        ticks:{precision:0,maxTicksLimit:10,callback:o.xTick||undefined},grid:{display:false}},
      y:{stacked:o.stacked,beginAtZero:!!o.zero,ticks:{maxTicksLimit:6}}}}});
}
const pts = (rows,fn)=> rows.map((r,i)=>({x:r.g,y:fn(r,i)})).map(p=>({x:p.x,y:p.y==null?null:p.y}));
function hline(label,color,rows,fn,o){ return ds(label,color,pts(rows,fn),o); }

// ---- global charts
(function(){
  const G = document.getElementById('global');
  const fit=[], bd=[], md=[];
  DATA.runs.forEach((r,i)=>{ const c=RUNCOL[i%RUNCOL.length];
    fit.push(Object.assign(hline(r.name+' best',c,r.gens,g=>g.best),{runName:r.name,key:'best'}));
    fit.push(Object.assign(hline(r.name+' median',c,r.gens,g=>g.median,{borderDash:[5,3],borderWidth:1.2,pointRadius:0}),{runName:r.name,key:'median'}));
    fit.push(Object.assign(hline(r.name+' default',c,r.gens,g=>g.default,{borderDash:[1.5,3],borderWidth:1.5,pointRadius:0}),{runName:r.name,key:'default'}));
    bd.push(hline(r.name,c,r.gens,g=>g.best!=null&&g.default!=null?g.best-g.default:null));
    md.push(hline(r.name,c,r.gens,g=>g.median!=null&&g.default!=null?g.median-g.default:null,{borderDash:[5,3]}));
  });
  const lg={filter:(it,d)=>d.datasets[it.datasetIndex].key==='best',
    generateLabels:ch=>Chart.defaults.plugins.legend.labels.generateLabels(ch).filter(it=>ch.data.datasets[it.datasetIndex].key==='best')
      .map(it=>Object.assign(it,{text:ch.data.datasets[it.datasetIndex].runName}))};
  chart(G,'Fitness per generation: solid best, dashed median (ran), dotted default',fit,{xTitle:'generation',legendLabels:lg});
  chart(G,'Best − default (more negative is better)',bd,{xTitle:'generation'});
  chart(G,'Median − default (more negative is better)',md,{xTitle:'generation'});
})();

// ---- per-run sections
const RUNS = document.getElementById('runs');
DATA.runs.forEach((r,ri)=>{
  const [c,t] = state(r);
  const det = document.createElement('details'); det.open = true; det.id='run-'+r.name;
  const g = r.gens[r.gens.length-1];
  det.innerHTML = `<summary><span>${r.name}</span><span class="chip ${c}">${t}</span><span class="mute">${r.host} &middot; ${r.path}</span>`
    + `<span class="mute">${r.gens.length} gens, last update ${fmtAge(r.last_update==null?null:now-r.last_update)} ago</span></summary><div class="grid"></div>`;
  RUNS.appendChild(det);
  const grid = det.querySelector('.grid');
  const G = r.gens;
  if(!G.length){ grid.innerHTML='<div class="mute">no generations yet</div>'; return; }
  const withLin = G.filter(x=>x.lin);
  const PC = {bound:'#8a6fd1',lively:'#4fb3d9',persist:'#d9a21b',total:FG};
  chart(grid,'Best candidate penalty (mean over worlds)',
    ['bound','lively','persist','total'].map(k=>hline(k,PC[k],G,x=>x.pen?x.pen[k]:null,k==='total'?{borderWidth:2.2}:{})),{xTitle:'generation',zero:true});
  const lineage=(title,key,zero)=>chart(grid,title,LINS.map(n=>hline(LINSHORT[n],LINCOL[n],G,x=>x.lin?x.lin[n][key]:null)),{xTitle:'generation',zero:zero!==false});
  lineage('Alive at end (best candidate worlds)','alive');
  lineage('Late births','late');
  lineage('Readiness','ready');
  lineage('Moving share','moving');
  lineage('Starved per hour','starved');
  chart(grid,'Screen: candidates per generation',
    [['inside','#3aa66a'],['margin','#d9a21b'],['refused','#d2554f']].map(([k,col])=>({label:k,backgroundColor:col,borderColor:col,
      data:G.map(x=>({x:x.g,y:x.screen[k]})),borderWidth:0})),{type:'bar',stacked:true,xTitle:'generation',zero:true});
  chart(grid,'CMA σ',[hline('σ','#4fb3d9',G,x=>x.sigma,{borderWidth:2})],{xTitle:'generation',legend:false});
  const W = r.windows;
  if(W.length){
    const NW = r.n_windows_per_gen||1;
    const wx = w=>w.g+w.w/NW;
    const wl = (n,key,o)=>ds(LINSHORT[n]+' '+key,LINCOL[n],W.map(w=>({x:wx(w),y:(w.lin[n]||{})[key]??null})),o);
    const wide = document.createElement('div'); wide.className='grid wide'; wide.style.marginTop='8px'; det.appendChild(wide);
    chart(wide,'Lock-step: twin |Δ| (solid) and pair spread (dashed) per window',
      LINS.flatMap(n=>[wl(n,'twin',{pointRadius:0}),wl(n,'pair',{borderDash:[4,3],borderWidth:1.1,pointRadius:0})]).filter(d=>d.data.some(p=>p.y!=null)),
      {xTitle:'generation (windows as fractions)',zero:true});
    chart(wide,'Lock-step: twin / pair ratio (signal-to-noise)',
      LINS.map(n=>wl(n,'ratio',{pointRadius:0})).filter(d=>d.data.some(p=>p.y!=null)),
      {xTitle:'generation (windows as fractions)',zero:true});
    chart(wide,'Lock-step: worlds alive',
      [ds('worlds','#4fb3d9',W.map(w=>({x:wx(w),y:w.worlds})),{pointRadius:0,borderWidth:2})],
      {xTitle:'generation (windows as fractions)',zero:true,legend:false});
  }
});

// keep scroll position and open sections across the 60 s meta refresh
try{
  const open=JSON.parse(sessionStorage.getItem('cd-open')||'{}');
  document.querySelectorAll('details').forEach(d=>{ if(d.id in open) d.open=open[d.id];
    d.addEventListener('toggle',()=>{ open[d.id]=d.open; sessionStorage.setItem('cd-open',JSON.stringify(open)); }); });
  const y=sessionStorage.getItem('cd-y'); if(y) window.scrollTo(0,+y);
  addEventListener('scroll',()=>sessionStorage.setItem('cd-y',scrollY),{passive:true});
}catch(e){}
</script></body></html>
"""


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--sources", default=str(DASH / "sources.txt"))
    ap.add_argument("--no-sync", action="store_true")
    ap.add_argument("--sync-only", action="store_true")
    a = ap.parse_args()
    srcs = read_sources(Path(a.sources))
    CACHE.mkdir(parents=True, exist_ok=True)
    if not a.no_sync:
        with ThreadPoolExecutor(max_workers=max(1, min(8, len(srcs)))) as ex:
            list(ex.map(sync_one, srcs))
    if a.sync_only:
        return
    runs = []
    for s in srcs:
        try:
            meta = json.loads((CACHE / s["name"] / "meta.json").read_text())
        except Exception:
            meta = {"error": "never synced"}
        runs.append(summarise(s, meta))
    print("wrote", render(runs))


if __name__ == "__main__":
    main()
