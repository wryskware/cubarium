#!/usr/bin/env python3
"""Cubarium co-training progress dashboard.

Reads runs/dashboard/sources.txt (one run per line: `name host path log`), copies each
run's generations.jsonl and its `w ...` window log lines into runs/dashboard/cache/<name>/
(remote hosts are only ever READ: ssh stat / ssh cat), and writes one self-contained
runs/dashboard/index.html (data embedded, auto-refresh 60 s, works from file://).

  scripts/cotrain_dashboard.py              runs sync + host probe, then render
  scripts/cotrain_dashboard.py --runs-only  runs sync (every 120 s in the loop), then render
  scripts/cotrain_dashboard.py --hosts-only host probe (every 30 s in the loop), then render
  scripts/cotrain_dashboard.py --no-sync    render from the cache only

Hosts (runs/dashboard/hosts.txt) are probed read-only: ssh running cat /proc/*, stat, ps, tail,
grep, date; plus `scripts/aws-graviton.sh status` and `aws ec2 describe-spot-price-history`
(both read-only). Nothing is ever started, stopped or modified.

A source that fails to sync keeps its cached copy and shows as stale.
"""
import argparse, datetime, json, math, os, re, shlex, subprocess, sys, time, urllib.request
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
        extra = dict(t.split("=", 1) for t in parts[4:] if "=" in t)
        out.append(dict(name=name, host=host, path=rpath.rstrip("/"), log=log, extra=extra))
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
    if r.returncode == 255:
        raise RuntimeError("ssh failed: " + r.stderr.decode(errors="replace").strip()[-200:])
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
        meta["absent"] = not (d / "generations.jsonl").exists()
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


def parse_run(src):
    """Heavy part: parse the cached jsonl and w-lines of one run."""
    d = CACHE / src["name"]
    gens, windows, nwin = [], {}, 0
    jp = d / "generations.jsonl"
    if jp.exists():
        with open(jp, "r", errors="replace") as f:
            for line in f:
                try:
                    g = json.loads(line)
                except ValueError:
                    continue  # partial last line while the run is writing
                gens.append(summarise_gen(g))
                nwin = max(nwin, len(g.get("windows") or []))
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
    return dict(gens=gens, windows=wins,
                min_per_gen=(sum(walls) / len(walls) / 60 if walls else None),
                n_windows_per_gen=nwin or ((max((w["w"] for w in wins), default=-1) + 1) if wins else None))


def parse_cached(src, meta):
    sig = [meta.get("jsonl"), meta.get("log")]
    sp = CACHE / "parsed.json"
    try:
        store = json.loads(sp.read_text())
    except Exception:
        store = {}
    ent = store.get(src["name"])
    if ent and ent.get("sig") == sig:
        return ent["parsed"]
    parsed = parse_run(src)
    store[src["name"]] = dict(sig=sig, parsed=parsed)
    atomic_write(sp, json.dumps(store).encode())
    return parsed


def load_meta(src):
    try:
        return json.loads((CACHE / src["name"] / "meta.json").read_text())
    except Exception:
        return {"error": "never synced"}


def summarise(src, meta):
    parsed = parse_cached(src, meta)
    mt = [m["mtime"] for m in (meta.get("jsonl"), meta.get("log")) if m]
    extra = src.get("extra", {})
    return dict(
        name=src["name"], host=src["host"], path=src["path"], **parsed,
        last_update=max(mt) if mt else None,
        jsonl_mtime=(meta.get("jsonl") or {}).get("mtime"),
        synced=meta.get("synced"), error=meta.get("error"),
        absent=bool(meta.get("absent")) and not parsed["gens"] and not parsed["windows"],
        continued_by=extra.get("continued_by"),
    )


def summarise_all(srcs):
    runs = [summarise(s, load_meta(s)) for s in srcs]
    by = {r["name"]: r for r in runs}
    for r in runs:
        if r["continued_by"] in by:
            by[r["continued_by"]]["continues"] = r["name"]
    return runs


# ---------------------------------------------------------------- hosts (live machine status)
HOSTS_TXT = DASH / "hosts.txt"
PRICE_FALLBACK = 0.70
AWS_ENV = dict(os.environ, AWS_PAGER="")


def read_hosts(path):
    out = []
    if not path.exists():
        return out
    for raw in path.read_text().splitlines():
        line = raw.split("#", 1)[0].strip()
        if not line:
            continue
        parts = line.split()
        if len(parts) < 2:
            continue
        kv = dict(t.split("=", 1) for t in parts[2:] if "=" in t)
        h = dict(label=parts[0], host=parts[1], id=kv.get("id"), expect=kv.get("expect", "yes") != "no",
                 ttl_h=float(kv["ttl_h"]) if "ttl_h" in kv else None, launch=None)
        if kv.get("launch"):
            h["launch"] = iso_epoch(kv["launch"])
        out.append(h)
    return out


def iso_epoch(t):
    try:
        return datetime.datetime.fromisoformat(t.replace("Z", "+00:00")).timestamp()
    except Exception:
        return None


def spot_price():
    """c7g.16xlarge spot price, cached for an hour; falls back to $0.70/h."""
    p = CACHE / "spotprice.json"
    try:
        d = json.loads(p.read_text())
    except Exception:
        d = None
    now = time.time()
    if d and now - d.get("fetched", 0) < 3600:
        return d
    try:
        r = subprocess.run(["aws", "--region", "us-east-1", "ec2", "describe-spot-price-history",
                            "--instance-types", "c7g.16xlarge", "--product-descriptions", "Linux/UNIX",
                            "--max-items", "1", "--output", "json"],
                           capture_output=True, timeout=60, env=AWS_ENV)
        price = float(json.loads(r.stdout)["SpotPriceHistory"][0]["SpotPrice"])
        d = dict(price=price, fetched=now, source="aws")
    except Exception as e:
        print(f"spot price lookup failed ({e}); using " + ("cached" if d else "fallback"), file=sys.stderr)
        old = d["price"] if d else PRICE_FALLBACK
        d = dict(price=old, fetched=now - 3300, source="cached" if d else "fallback")  # retry in 5 min
    atomic_write(p, json.dumps(d).encode())
    return d


def aws_status():
    """scripts/aws-graviton.sh status (read-only): {instance id: row}, or None on failure."""
    try:
        r = subprocess.run([str(ROOT / "scripts" / "aws-graviton.sh"), "status"], capture_output=True,
                           timeout=90, env=AWS_ENV, cwd=ROOT)
        if r.returncode != 0:
            raise RuntimeError(r.stderr.decode(errors="replace").strip()[-200:])
        rows = {}
        for line in r.stdout.decode().splitlines():
            f = line.split()
            if len(f) >= 7:
                rows[f[1]] = dict(id=f[1], ip=f[2], state=f[3], market=f[4], launch=iso_epoch(f[5]), type=f[6])
        return rows
    except Exception as e:
        print(f"aws status failed: {e}", file=sys.stderr)
        return None


def probe_script(h, srcs):
    local = h["host"] == "local"
    q = shlex.quote
    parts = ["echo @@t", "date +%s", "echo @@stat", "cat /proc/stat", "echo @@load", "cat /proc/loadavg",
             "echo @@up", "cat /proc/uptime", "echo @@mem", "cat /proc/meminfo", "echo @@ps",
             "ps -C cubarium-search -o pid=,etimes=,rss=,args="]
    for s in srcs:
        lp = str(local_resolve(s["log"])) if local else rpath(s["log"])
        jp = (str(local_resolve(s["path"])) if local else rpath(s["path"])) + "/generations.jsonl"
        parts += [f"echo {q('@@run ' + s['name'])}",
                  f"stat -c '%n %Y %s' -- {q(lp)} {q(jp)} 2>/dev/null",
                  "echo @@w",
                  f"tail -c 200000 {q(lp)} 2>/dev/null | grep -E '^( *w |gen )' | tail -n 16"]
    return "; ".join(parts)


def run_probe(h, srcs):
    script = probe_script(h, srcs)
    t0 = time.time()
    try:
        if h["host"] == "local":
            r = subprocess.run(["bash", "-c", script], capture_output=True, timeout=60)
        else:
            r = subprocess.run(ssh_cmd(h["host"]) + [script], capture_output=True, timeout=60)
    except subprocess.TimeoutExpired:
        return dict(ok=False, error="probe timed out")
    if r.returncode == 255 or not r.stdout:
        return dict(ok=False, error="ssh: " + r.stderr.decode(errors="replace").strip()[-160:])
    out = dict(ok=True, local_now=time.time(), runs={}, ps=[], stat=[], mem={})
    sec, run = None, None
    for line in r.stdout.decode(errors="replace").splitlines():
        if line.startswith("@@"):
            tag = line[2:]
            if tag.startswith("run "):
                run = tag[4:]
                out["runs"][run] = dict(files={}, lines=[])
                sec = "runhead"
            else:
                sec = tag
            continue
        if sec == "t":
            out["remote_now"] = float(line)
        elif sec == "stat":
            out["stat"].append(line)
        elif sec == "load":
            out["load"] = [float(x) for x in line.split()[:3]]
        elif sec == "up":
            out["uptime_s"] = float(line.split()[0])
        elif sec == "mem":
            m = re.match(r"(\w+):\s+(\d+)", line)
            if m:
                out["mem"][m.group(1)] = int(m.group(2))
        elif sec == "ps":
            f = line.split(None, 3)
            if len(f) >= 3:
                out["ps"].append(dict(pid=int(f[0]), etime=int(f[1]), rss_kb=int(f[2]), args=f[3] if len(f) > 3 else ""))
        elif sec == "runhead" and run:
            bits = line.rsplit(" ", 2)
            if len(bits) == 3:
                out["runs"][run]["files"][bits[0]] = (int(bits[1]), int(bits[2]))
        elif sec == "w" and run:
            out["runs"][run]["lines"].append(line)
    out["rtt"] = time.time() - t0
    return out


def cpu_sample(stat_lines):
    total = idle = None
    cores = 0
    for l in stat_lines:
        f = l.split()
        if f and f[0] == "cpu":
            v = [int(x) for x in f[1:9]]
            total, idle = sum(v), v[3] + v[4]
        elif f and re.fullmatch(r"cpu\d+", f[0]):
            cores += 1
    return total, idle, cores


def run_record(src, pr, offset, summ, now):
    """Progress of one run as seen by the host probe (all times are local epoch seconds)."""
    info = pr["runs"].get(src["name"], dict(files={}, lines=[]))
    lp = str(local_resolve(src["log"])) if src["host"] == "local" else rpath(src["log"])
    jp = (str(local_resolve(src["path"])) if src["host"] == "local" else rpath(src["path"])) + "/generations.jsonl"
    log_m = info["files"].get(lp, (None,))[0]
    jsonl_m = info["files"].get(jp, (None,))[0]
    rec = dict(name=src["name"], exists=log_m is not None or jsonl_m is not None,
               log_mtime=log_m - offset if log_m else None, jsonl_mtime=jsonl_m - offset if jsonl_m else None,
               phase=None, cur_gen=None, w=None, nw=None, win_s=None, eta=None)
    if not rec["exists"]:
        return rec
    wl = [x for x in (parse_window_line(l) for l in info["lines"]) if x]
    gens = (summ or {}).get("gens") or []
    cur_gen = (gens[-1]["g"] + 1) if gens else 0
    nw = (summ or {}).get("n_windows_per_gen") or 12
    mean_gen_s = ((summ or {}).get("min_per_gen") or 0) * 60 or None
    rec.update(cur_gen=cur_gen, nw=nw)
    if wl and wl[-1]["g"] >= cur_gen:
        last = wl[-1]
        secs = [x["wall"] for x in wl if x["g"] == last["g"]][-nw:]
        win_s = sum(secs) / len(secs)
        rec.update(phase="windows", cur_gen=last["g"], w=last["w"], win_s=win_s)
        t_ref = rec["log_mtime"] or now  # nothing but w-lines is written between windows
        rec["eta"] = t_ref + max(0, nw - 1 - last["w"]) * win_s
    else:
        rec["phase"] = "setup" if wl else "evaluating"
        if wl:
            secs = [x["wall"] for x in wl][-nw:]
            rec["win_s"] = sum(secs) / len(secs)
        if rec["jsonl_mtime"] and mean_gen_s:
            rec["eta"] = rec["jsonl_mtime"] + mean_gen_s
    return rec


def host_cycle(srcs):
    hosts = read_hosts(HOSTS_TXT)
    if not hosts:
        return
    try:
        old = json.loads((CACHE / "hosts.json").read_text()).get("hosts", {})
    except Exception:
        old = {}
    now = time.time()
    is_aws = any(h["host"].startswith("aws:") for h in hosts)
    aws = aws_status() if is_aws else None
    price = spot_price() if is_aws else None
    summaries = {r["name"]: r for r in summarise_all(srcs)}
    mine = {h["label"]: [s for s in srcs if s["host"] == h["host"]] for h in hosts}
    with ThreadPoolExecutor(max_workers=len(hosts)) as ex:
        probes = list(ex.map(lambda h: run_probe(h, mine[h["label"]]), hosts))
    out = []
    for h, pr in zip(hosts, probes):
        prev = old.get(h["label"], {})
        rec = dict(label=h["label"], host=h["host"], id=h["id"], expect=h["expect"],
                   ip=h["host"][4:] if h["host"].startswith("aws:") else None,
                   is_aws=h["host"].startswith("aws:"), probed=now, cpu_hist=prev.get("cpu_hist", []),
                   cpu_prev=prev.get("cpu_prev"), reachable=pr["ok"], error=pr.get("error"))
        if rec["is_aws"]:
            row = (aws or {}).get(h["id"]) if aws is not None else None
            launch = (row or {}).get("launch") or h["launch"] or prev.get("launch")
            rec.update(aws_known=aws is not None, aws_state=(row["state"] if row else ("gone" if aws is not None else prev.get("aws_state"))),
                       launch=launch, market=(row or {}).get("market"), itype=(row or {}).get("type"),
                       terminate=(launch + h["ttl_h"] * 3600) if launch and h["ttl_h"] else None,
                       price=price["price"], price_src=price["source"])
        if pr["ok"]:
            offset = pr["remote_now"] - pr["local_now"]
            total, idle, cores = cpu_sample(pr["stat"])
            cp = prev.get("cpu_prev")
            cpu = None
            if total is not None and cp and now - cp["t"] < 180 and total > cp["total"]:
                cpu = max(0.0, min(100.0, 100.0 * (1 - (idle - cp["idle"]) / (total - cp["total"]))))
                rec["cpu_hist"] = (rec["cpu_hist"] + [round(cpu, 1)])[-60:]
            rec["cpu_prev"] = dict(t=now, total=total, idle=idle)
            m = pr["mem"]
            ps = pr["ps"]
            rec.update(cpu=cpu, cores=cores, load=pr.get("load"), os_uptime_s=pr.get("uptime_s"),
                       mem_total_kb=m.get("MemTotal"), mem_avail_kb=m.get("MemAvailable"),
                       proc=dict(running=bool(ps), n=len(ps), etime=max((p["etime"] for p in ps), default=None),
                                 rss_kb=sum(p["rss_kb"] for p in ps), args=(ps[0]["args"][:200] if ps else "")),
                       runs=[run_record(s, pr, offset, summaries.get(s["name"]), now) for s in mine[h["label"]]])
            existing = [r for r in rec["runs"] if r["exists"]]
            pick = max(existing, key=lambda r: (r["log_mtime"] or 0, r["jsonl_mtime"] or 0)) if existing else (rec["runs"][0] if rec["runs"] else None)
            rec["run"] = pick
            rec["n_runs"] = len(rec["runs"])
            rec["last_ok"] = now
        else:
            rec.update(last_ok=prev.get("last_ok"), cpu=None, runs=[], run=prev.get("run"),
                       proc=prev.get("proc"), cores=prev.get("cores"), load=prev.get("load"),
                       mem_total_kb=prev.get("mem_total_kb"), mem_avail_kb=prev.get("mem_avail_kb"),
                       os_uptime_s=None, n_runs=prev.get("n_runs"))
        out.append(rec)
    atomic_write(CACHE / "hosts.json", json.dumps(dict(updated=now, price=price, hosts={r["label"]: r for r in out}, order=[r["label"] for r in out]), allow_nan=False).encode())


def load_hosts():
    try:
        d = json.loads((CACHE / "hosts.json").read_text())
        return dict(updated=d["updated"], price=d.get("price"), hosts=[d["hosts"][k] for k in d["order"]])
    except Exception:
        return None


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
    data = json.dumps(dict(generated=int(time.time()), runs=runs, hosts=load_hosts()), allow_nan=False)
    data = data.replace("</", "<\\/")
    html = TEMPLATE.replace("__CHARTJS__", tag).replace("__DATA__", data)
    out = DASH / "index.html"
    atomic_write(out, html.encode())
    return out


TEMPLATE = r"""<!doctype html>
<html lang="en"><head><meta charset="utf-8">
<meta http-equiv="refresh" content="20">
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
.done{color:var(--mute)}
.sub{display:block;color:var(--mute);font-size:11px}
.flag{display:inline-block;padding:0 7px;margin:1px 2px 1px 0;border-radius:9px;font-size:11px;font-weight:600;color:#fff;background:var(--bad)}
.flag.w{background:var(--warn)}
tr.red td:first-child{box-shadow:inset 3px 0 0 var(--bad)}
.hosts td{vertical-align:top;line-height:1.35}
.hosts td.l{white-space:normal}
.spark{vertical-align:middle;margin-left:4px}
.tot{font-size:12px;color:var(--mute);margin:0 0 6px}
.explain{margin:2px 0 10px;color:var(--mute)}
</style></head><body>
<header><h1>Cubarium co-training</h1><span class="mute" id="stamp"></span>
<span class="mute">lower fitness is better &middot; best/median over candidates that ran, default = shipped defaults</span></header>
<p class="explain"><b>Hosts:</b> live machine status (probed every 30 s). &nbsp; <b>Runs:</b> search progress per generation (synced every 120 s).</p>
<h2>Hosts</h2>
<div class="tot" id="hosttot"></div>
<div style="overflow-x:auto"><table id="hosts" class="hosts"></table></div>
<h2>Runs</h2>
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
  if(!r.gens.length && !r.windows.length) return ['done',r.error?'not started (host down?)':'not started',r.error||''];
  if(r.error) return ['stale','stale (sync failed)',r.error];
  if(age==null) return ['dead','no data',''];
  if(age>900) return r.continued_by ? ['done','handed off','continued on '+r.continued_by] : ['stale','stale','no new output for '+fmtAge(age)];
  return ['live','live','']; }

const stampEl=document.getElementById('stamp'); stampEl.textContent = 'page built ' + new Date(DATA.generated*1000).toLocaleTimeString();


// ---- hosts
function fmtDur(s){ if(s==null||!isFinite(s)) return '–'; const neg=s<0; s=Math.abs(Math.round(s));
  const h=Math.floor(s/3600), m=Math.floor((s%3600)/60);
  const t = h>0 ? h+'h '+String(m).padStart(2,'0')+'m' : (m>0 ? m+'m '+String(s%60).padStart(2,'0')+'s' : s+'s');
  return (neg?'-':'')+t; }
function spark(v){ if(!v||v.length<2) return ''; const w=84,h=22; 
  const pts=v.map((y,i)=>((i/(v.length-1))*(w-2)+1).toFixed(1)+','+((1-Math.min(100,Math.max(0,y))/100)*(h-3)+1.5).toFixed(1)).join(' ');
  return `<svg class="spark" width="${w}" height="${h}" viewBox="0 0 ${w} ${h}"><rect width="${w}" height="${h}" fill="none" stroke="${GRID}"/><polyline points="${pts}" fill="none" stroke="#4fb3d9" stroke-width="1.4"/></svg>`; }
const clock = e => new Date(e*1000).toLocaleTimeString([], {hour:'2-digit',minute:'2-digit'});
(function(){
  const HS = DATA.hosts, tb = document.getElementById('hosts');
  if(!HS || !HS.hosts.length){ tb.innerHTML=''; document.getElementById('hosttot').textContent='no hosts configured (runs/dashboard/hosts.txt)'; return; }
  let h = '<tr><th class="l">Host</th><th class="l">AWS</th><th class="l">Uptime / left</th><th>Cost</th><th class="l">CPU</th><th class="l">Load / cores</th><th class="l">Memory</th><th class="l">Search</th><th class="l">Run</th><th class="l">Flags</th></tr>';
  let total=0, nAws=0, price=HS.price;
  for(const x of HS.hosts){
    const fl=[]; const red=(t)=>fl.push(['',t]), amb=(t)=>fl.push(['w',t]);
    const pAge = now-x.probed, dataAge = HS.updated? now-HS.updated : null;
    if(dataAge!=null && dataAge>180) amb('host data '+fmtAge(dataAge)+' old');
    if(!x.reachable) red('unreachable');
    let awsCell='–', upCell='–', cost='–';
    if(x.is_aws){
      const st = x.aws_state||'unknown'; nAws++;
      awsCell = `<span class="chip ${st==='running'?'live':'dead'}">${st}</span><span class="sub">${x.itype||'c7g.16xlarge'} ${x.market||'spot'}</span>`;
      if(st!=='running') red('instance '+st);
      if(x.launch){ const up=now-x.launch, left = x.terminate? x.terminate-now : null;
        upCell = fmtDur(up) + (left!=null? `<span class="sub">${left>0?fmtDur(left)+' left':'terminated'} &middot; ends ${clock(x.terminate)}</span>`:'');
        if(left!=null && left<2700) red(left>0?'terminates in '+fmtDur(left):'past auto-terminate');
        const hrs=Math.max(0,Math.min(now,x.terminate||now)-x.launch)/3600; const c=hrs*(x.price||0.7); total+=c;
        cost = '$'+c.toFixed(2)+`<span class="sub">$${(x.price||0.7).toFixed(3)}/h${x.price_src!=='aws'?' ('+x.price_src+')':''}</span>`; }
    } else if(x.os_uptime_s!=null){ upCell = fmtDur(x.os_uptime_s+pAge)+'<span class="sub">OS uptime</span>'; }
    const cpu = x.cpu==null? '–' : Math.round(x.cpu)+'%';
    const cpuCell = cpu + spark(x.cpu_hist);
    const loadCell = x.load? `${x.load[0].toFixed(1)} <span class="mute">${x.load[1].toFixed(1)} ${x.load[2].toFixed(1)}</span><span class="sub">${x.cores||'?'} cores</span>` : '–';
    const used = x.mem_total_kb&&x.mem_avail_kb!=null? (x.mem_total_kb-x.mem_avail_kb)/1048576 : null;
    const memCell = used==null?'–':`${used.toFixed(1)} / ${(x.mem_total_kb/1048576).toFixed(0)} GB<span class="sub">${Math.round(100*used/(x.mem_total_kb/1048576))}% used</span>`;
    const p=x.proc, run=x.run;
    let procCell = !x.reachable?'–': (p&&p.running ? `<span class="live">running</span> ${fmtDur(p.etime+pAge)}<span class="sub">RSS ${(p.rss_kb/1048576).toFixed(1)} GB${p.n>1?' &middot; '+p.n+' procs':''}</span>` : '<span class="dead">not running</span>');
    let runCell='–';
    if(run){
      const logAge = run.log_mtime? now-run.log_mtime : null;
      if(!run.exists){ runCell = `${run.name}<span class="sub">not started</span>`;
        if(x.expect && x.reachable) amb('run not started'); }
      else {
        const more = x.n_runs>1? ` <span class="mute">(+${x.n_runs-1} other)</span>`:'';
        let l2;
        if(run.phase==='windows') l2 = `gen ${run.cur_gen} &middot; ${run.w+1}/${run.nw} windows`;
        else if(run.phase==='setup') l2 = `gen ${run.cur_gen} setting up worlds`;
        else l2 = `gen ${run.cur_gen} (ecology only, no w-lines)`;
        const win = run.win_s? ` &middot; ${Math.round(run.win_s)} s/win`:'';
        const eta = run.eta==null? '' : (run.eta<now? ' &middot; gen end due' : ` &middot; gen end ~${fmtDur(run.eta-now)} (${clock(run.eta)})`);
        runCell = `${run.name}${more}<span class="sub">${l2}${win}${eta}</span><span class="sub">log changed ${fmtDur(logAge)} ago</span>`;
        if(x.expect && x.reachable){
          if(p && !p.running) red('search not running');
          if(logAge!=null && logAge>600) red('log quiet '+fmtDur(logAge));
        }
      }
    } else if(x.expect && x.reachable && p && !p.running) red('search not running');
    const flags = fl.map(([c,t])=>`<span class="flag ${c}">${t}</span>`).join('') || '<span class="mute">ok</span>';
    h += `<tr class="${fl.some(f=>f[0]==='')?'red':''}"><td class="l"><b>${x.label}</b><span class="sub">${x.ip?x.ip+' &middot; ':''}${x.id||x.host}</span></td>`
      + `<td class="l">${awsCell}</td><td class="l">${upCell}</td><td>${cost}</td><td class="l">${cpuCell}</td><td class="l">${loadCell}</td><td class="l">${memCell}</td>`
      + `<td class="l">${procCell}</td><td class="l">${runCell}</td><td class="l">${flags}</td></tr>`;
  }
  tb.innerHTML = h;
  document.getElementById('hosttot').innerHTML = (nAws? `Spot cost so far across ${nAws} AWS hosts: <b>$${total.toFixed(2)}</b>` + (price?` at $${price.price.toFixed(3)}/h (${price.source})`:'') + ' &middot; ':'') + 'probed ' + (HS.updated? fmtAge(now-HS.updated)+' ago':'never');
})();

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
    h += `<tr><td class="l"><a href="#run-${r.name}"><b>${r.name}</b></a>${r.continued_by?`<span class="sub">earlier segment &rarr; ${r.continued_by}</span>`:''}${r.continues?`<span class="sub">continues ${r.continues}</span>`:''}</td><td class="l mute">${r.host}</td><td>${done}</td><td>${prog}</td>`
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
    ap.add_argument("--no-sync", action="store_true", help="render from the cache only")
    ap.add_argument("--runs-only", action="store_true", help="sync runs (no host probe)")
    ap.add_argument("--hosts-only", action="store_true", help="probe hosts (no run sync)")
    a = ap.parse_args()
    srcs = read_sources(Path(a.sources))
    CACHE.mkdir(parents=True, exist_ok=True)
    if not a.no_sync:
        if not a.hosts_only:
            with ThreadPoolExecutor(max_workers=max(1, min(8, len(srcs)))) as ex:
                list(ex.map(sync_one, srcs))
        if not a.runs_only:
            host_cycle(srcs)
    print("wrote", render(summarise_all(srcs)))


if __name__ == "__main__":
    main()
