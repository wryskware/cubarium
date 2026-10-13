"""lab: submit, quick, status, top, hosts, cancel, approve, budget, wait, events, kinds, install, daemon."""

import argparse
import json
import os
from pathlib import Path
import re
import shlex
import subprocess
import sys
import time
import tomllib

from . import budget
from .core import Context, SpecError, load_kinds
from .top import cmd_top
from .ec2 import fill_label
from .hosts import Inventory, lane
from .store import TERMINAL, Paths, Store, new_id, read_events, spool
from .stops import rule_labels, validate_rules

SAFE_NAME = re.compile(r"[^A-Za-z0-9_.-]+")
WAKE = set(TERMINAL) | {"approval-needed", "approval-refused", "ec2-auth-expired", "ec2-down-failed", "ec2-limit"}


def read_spec(path):
    text = Path(path).read_text()
    return tomllib.loads(text) if str(path).endswith(".toml") else json.loads(text)


def cmd_submit(args, paths):
    spec = read_spec(args.spec)
    if args.name:
        spec["name"] = args.name
    spec.setdefault("_cwd", os.getcwd())
    spec["_spec_file"] = str(Path(args.spec).resolve())
    print(submit_spec(spec, paths))


def quick_spec(argv, cwd, name=None, threads=1, mem_gb=2.0, out=None):
    """The `lab quick` job: one command task in the quick lane, run in cwd."""
    if argv and argv[0] == "--":
        argv = argv[1:]
    if not argv:
        raise SpecError("lab quick needs a command after --")
    spec = dict(kind="command", name=name or SAFE_NAME.sub("-", Path(argv[0]).name)[:40].strip("-.") or "quick",
                argv=list(argv), cwd=str(cwd), _cwd=str(cwd), quick=True, threads=threads, mem_gb=mem_gb)
    if out:
        spec["out"] = str(Path(cwd, out).resolve())
    return spec


def cmd_quick(args, paths):
    spec = quick_spec(args.argv, os.getcwd(), args.name, args.threads, args.mem_gb, args.out)
    job_id = submit_spec(spec, paths)
    print(job_id)
    print(f"out {spec.get('out') or paths.runs / job_id}")


def submit_spec(spec, paths):
    """Validate (pinning commits at submit time), spool for labd; return the job id."""
    kinds = load_kinds()
    kind = kinds.get(spec.get("kind"))
    if kind is None:
        raise SpecError(f"unknown kind {spec.get('kind')!r}; known: {', '.join(sorted(kinds))}")
    try:
        inventory = Inventory.load(paths.hosts, paths.repo / "scripts/run-study.py")
    except OSError:
        inventory = None
    # Validate now: commits are pinned at submit time, so later commits never leak in.
    ctx = Context(paths, inventory=inventory)
    spec = validate_rules(kind.validate(spec, ctx), ctx, kind)
    job_id = new_id(spec["kind"], spec.get("name"))
    spool(paths, "incoming", dict(id=job_id, spec=spec, submitted=time.time()), job_id)
    return job_id


def fmt_h(hours):
    if hours is None:
        return "?"
    return f"{hours * 60:.0f}m" if hours < 1 else f"{hours:.1f}h"


def cmd_status(args, paths):
    store = Store(paths, readonly=True)
    rows = store.q("SELECT * FROM jobs ORDER BY submitted DESC LIMIT ?", args.limit)
    if not args.all:
        rows = [r for r in rows if r["state"] not in TERMINAL] + \
               [r for r in rows if r["state"] in TERMINAL][:args.recent]
    if args.json:
        print(json.dumps([dict(r) for r in rows], default=str, indent=1))
        return
    spooled = sorted(p.stem for p in (paths.state / "incoming").glob("*.json")) if (paths.state / "incoming").exists() else []
    print(f"{'JOB':34} {'KIND':10} {'STATE':10} {'RUN/PEND/DONE/BAD':17} {'AGE':>6}  REASON")
    for row in rows:
        counts = {s: 0 for s in ("running", "pending", "done", "bad")}
        for task in store.q("SELECT state FROM tasks WHERE job=?", row["id"]):
            key = task["state"] if task["state"] in counts else "bad" if task["state"] not in ("skipped",) else None
            if key:
                counts[key] += 1
        age = fmt_h((time.time() - (row["submitted"] or time.time())) / 3600)
        tasks = f"{counts['running']}/{counts['pending']}/{counts['done']}/{counts['bad']}"
        print(f"{row['id'][:34]:34} {row['kind'][:10]:10} {row['state'][:10]:10} {tasks:17} {age:>6}  {(row['reason'] or '')[:70]}")
        for label in rule_labels(json.loads(row['spec'])):
            print(f"  {label}")
    for stem in spooled:
        print(f"{stem[:34]:34} {'?':10} {'spooled':10} {'-':17} {'':>6}  waiting for labd")
    running = store.q("SELECT job, name, host, grp, started, spec FROM tasks WHERE state='running' ORDER BY started")
    if running:
        print("\nrunning tasks:")
        for row in running:
            est = json.loads(row["spec"]).get("est_h")
            elapsed = (time.time() - (row["started"] or time.time())) / 3600
            eta = fmt_h(max(0.0, est - elapsed)) if est else "?"
            print(f"  {row['job'][:30]}/{row['name'][:28]:28} {row['host']}:g{row['grp'] if row['grp'] is not None and row['grp'] >= 0 else '-'}"
                  f"  up {fmt_h(elapsed)}  left {eta}")
    waiting = store.q("SELECT job, name, reason FROM tasks WHERE state='pending' AND reason IS NOT NULL "
                      "ORDER BY job, seq")
    if waiting:
        print("\nwaiting tasks:")
        for row in waiting:
            print(f"  {row['job']}/{row['name']}: {row['reason']}")
    print_hosts(store, paths, brief=True)
    ec2 = store.kv_get("ec2", dict(state="idle"))
    left = budget_summary(store, paths)
    line = f"ec2: {ec2.get('state')}"
    if ec2.get("approval"):
        line += f" (request {ec2['approval']})"
    open_requests = store.q("SELECT id, request FROM approvals WHERE state='open'")
    for request in open_requests:
        details = json.loads(request["request"])
        line += (f"\n  approval needed {request['id']}: {details.get('eligible_tasks')} tasks, local ETA "
                 f"{fmt_h(details.get('local_eta_h'))}, {details.get('instance_type')} ETA {fmt_h(details.get('ec2_eta_h'))}"
                 f" ~${details.get('est_usd')}  {fill_label(details)}"
                 f"  ->  lab approve ec2 {request['id']} --max-hours H --max-usd D"
                 f" (ec2 budget left ${left['remaining']:.2f})")
    print(line)
    print(budget.status_line(left))
    tail = store.kv_get("ec2-tail", {})
    if tail:
        print(f"  EC2 tail: {tail['moved']} tasks moved home; ~{tail['est_h_saved']:.2f} task h saved, "
              f"~${tail['usd_saved']:.2f} instance cost saved")


def budget_summary(store, paths):
    """The EC2 budget as labd counts it (read-only; spend comes from the store or the event log)."""
    try:
        inventory = Inventory.load(paths.hosts, paths.repo / "scripts/run-study.py")
        price_of = lambda name: inventory.instance_type(name).get("spot_usd_h") if name else None  # noqa: E731
    except (OSError, ValueError, KeyError):
        price_of = lambda name: None  # noqa: E731
    return budget.summary(store, store.kv_get("ec2", dict(state="idle")), price_of, time.time(), paths.events)


def cmd_budget(args, paths):
    if args.action is None:
        if args.usd is not None or args.note:
            raise SpecError("use `lab budget grant USD --note TEXT`")
        print(budget.report(budget_summary(Store(paths, readonly=True), paths)))
        return
    if args.usd is None:
        raise SpecError("lab budget grant needs USD")
    budget.check_grant(args.usd, args.note)
    at = budget.parse_at(args.at, time.time())
    spool(paths, "control", dict(op="grant", usd=args.usd, note=args.note.strip(), at=at),
          f"{time.time():.6f}-grant")
    print(f"budget grant queued: ${args.usd:.2f} \"{args.note.strip()}\" (labd records it on its next tick)")


def print_hosts(store, paths, brief=False):
    try:
        inventory = Inventory.load(paths.hosts, paths.repo / "scripts/run-study.py")
    except OSError as error:
        print(f"hosts: cannot read {paths.hosts}: {error}")
        return
    running = store.q("SELECT host, grp, spec FROM tasks WHERE state='running'")
    print("\nhosts:" if brief else f"{'HOST':10} {'KIND':6} {'ARCH':8} {'STATE':6} {'THREADS':>12} {'MEM GB':>10}  GROUPS / NOTES")
    for name, host in inventory.hosts.items():
        probe = store.kv_get(f"host:{name}", {})
        up = host.kind == "local" or probe.get("up")
        mine = [r for r in running if r["host"] == name]
        threads = sum(json.loads(r["spec"])["threads"] for r in mine)
        build, quick = (sum(json.loads(r["spec"])["threads"] for r in mine if lane(json.loads(r["spec"])) == which)
                        for which in ("build", "quick"))
        mem = sum(json.loads(r["spec"])["mem_gb"] for r in mine)
        groups = probe.get("groups") or host.groups or []
        per = []
        for index, group in enumerate(groups):
            used = sum(json.loads(r["spec"])["threads"] for r in mine if r["grp"] == index)
            cap = int(len(group) * host.usable / max(1, sum(map(len, groups))))
            per.append(f"g{index} {used}/{cap}")
        notes = " ".join(per) or "groups unknown (probe)"
        if probe.get("launcher"):
            notes += f"; launcher {probe['launcher']}"
        if probe.get("load") is not None:
            notes += f"; load {probe['load']}"
        if host.kind == "local":
            notes += f"; load {os.getloadavg()[0]:.1f}"
        if not host.enabled:
            notes += "; disabled"
        state = "up" if up else "down"
        thread_label = host.thread_label(threads, build, quick)
        if brief:
            print(f"  {name:9} {state:5} {thread_label} threads  {mem:.0f}/{host.max_mem_gb:.0f} GB  {notes}")
        else:
            print(f"{name:10} {host.kind:6} {host.arch:8} {state:6} {thread_label:>12} "
                  f"{f'{mem:.0f}/{host.max_mem_gb:.0f}':>10}  {notes}")
    if not brief:
        for name in inventory.ec2.get("types", []):
            info = inventory.instance_type(name)
            fallback = f"; fallback {info['fallback']}" if info.get("fallback") else ""
            print(f"{'ec2':10} {'ec2':6} {info.get('arch', '?'):8} {'-':6} {info.get('cpus', '?'):>12} "
                  f"{info.get('mem_gb', '?'):>10}  {name}: ~${info.get('spot_usd_h')}/h spot (assumed), approval per launch{fallback}")


def cmd_hosts(args, paths):
    print_hosts(Store(paths, readonly=True), paths)


def cmd_cancel(args, paths):
    spool(paths, "control", dict(op="cancel", job=args.job), f"{time.time():.6f}-cancel")
    print(f"cancel queued for {args.job}")


def cmd_approve(args, paths):
    if args.what != "ec2":
        raise SpecError("only `lab approve ec2 ID` exists")
    # The daemon checks again when it acts; this refuses early with the same message.
    budget.check(budget_summary(Store(paths, readonly=True), paths), args.max_usd)
    spool(paths, "control", dict(op="approve", what="ec2", id=args.id, max_hours=args.max_hours,
                                 max_usd=args.max_usd, type=args.type), f"{time.time():.6f}-approve")
    print(f"approval queued: {args.id} type={args.type or 'requested'} max {args.max_hours} h / ${args.max_usd}")


def job_final(paths, job_id):
    if not paths.db.exists():
        return None
    row = Store(paths, readonly=True).one("SELECT state, reason FROM jobs WHERE id=?", job_id)
    return dict(row) if row and row["state"] in TERMINAL else None


def cmd_wait(args, paths):
    """The one agent waiter: blocks (sleep loop, no busy CPU) until the job or any job ends."""
    if not args.job and not args.any_new:
        raise SpecError("wait needs --job ID or --any-new")
    offset = paths.events.stat().st_size if paths.events.exists() and args.any_new else 0
    deadline = time.monotonic() + args.timeout if args.timeout else None
    if args.job:
        final = job_final(paths, args.job)
        if final:
            print(json.dumps(dict(event=final["state"], job=args.job, reason=final["reason"])))
            return 0 if final["state"] == "done" else 2
    while True:
        records, offset = read_events(paths.events, offset)
        for record in records:
            if args.job and record.get("job") == args.job and record["event"] in TERMINAL:
                print(json.dumps(record))
                return 0 if record["event"] == "done" else 2
            if args.any_new and record["event"] in WAKE:
                print(json.dumps(record))
                return 0
        if deadline and time.monotonic() >= deadline:
            print("timeout", file=sys.stderr)
            return 124
        time.sleep(args.interval)


def cmd_events(args, paths):
    records, _ = read_events(paths.events)
    for record in records[-args.n:]:
        if args.job and record.get("job") != args.job:
            continue
        print(json.dumps(record))


def cmd_kinds(args, paths):
    for name, kind in sorted(load_kinds().items()):
        print(f"{name:12} {kind.description}")


UNIT = """[Unit]
Description=Cubarium lab daemon: owns sim, test and build jobs (scripts/lab)
After=default.target

[Service]
Type=simple
ExecStart={python} {labd}
Environment={env}
Restart=on-failure
RestartSec=10
# Tasks are their own transient units; stopping labd never stops them.
KillMode=mixed
TimeoutStopSec=20

[Install]
WantedBy=default.target
"""


def cmd_install(args, paths):
    unit = Path("~/.config/systemd/user/labd.service").expanduser()
    env = " ".join(shlex.quote(f"{k}={v}") for k, v in (("PATH", os.environ.get("PATH", "/usr/bin")),
                                                       *(("LABD_" + k.upper(), str(getattr(paths, k)))
                                                         for k in ("state", "cache", "runs", "hosts"))))
    text = UNIT.format(python=sys.executable, labd=paths.repo / "scripts/lab/labd", env=env)
    if args.print:
        print(text)
        return
    unit.parent.mkdir(parents=True, exist_ok=True)
    unit.write_text(text)
    print(f"wrote {unit}")
    if not args.no_start:
        subprocess.run(["systemctl", "--user", "daemon-reload"], check=True)
        subprocess.run(["systemctl", "--user", "enable", "--now", "labd.service"], check=True)
        print("labd.service enabled and started")


def cmd_daemon(args, paths):
    from .daemon import Daemon
    daemon = Daemon(paths)
    daemon.run(tick=args.tick)


def main(argv=None):
    parser = argparse.ArgumentParser(prog="lab", description=__doc__)
    parser.add_argument("--state", help="state dir (default ~/.local/state/labd or $LABD_STATE)")
    parser.add_argument("--cache")
    parser.add_argument("--runs")
    parser.add_argument("--hosts")
    sub = parser.add_subparsers(dest="command", required=True)
    p = sub.add_parser("submit", help="validate SPEC (toml/json), pin commits, spool it; prints the job id")
    p.add_argument("spec")
    p.add_argument("--name")
    p = sub.add_parser("quick", help="lab quick [--name N] [--mem-gb G] [--threads T] [--out DIR] -- CMD...: "
                       "one command task in the quick lane, run here; prints the job id and output dir")
    p.add_argument("--name")
    p.add_argument("--threads", type=int, default=1)
    p.add_argument("--mem-gb", type=float, default=2.0)
    p.add_argument("--out", help="output dir (default: the runs dir / job id)")
    p.add_argument("argv", nargs=argparse.REMAINDER)
    p = sub.add_parser("status")
    p.add_argument("--all", action="store_true")
    p.add_argument("--json", action="store_true")
    p.add_argument("--limit", type=int, default=200)
    p.add_argument("--recent", type=int, default=8)
    p = sub.add_parser("top", help="live read-only dashboard (q quits)")
    p.add_argument("--once", action="store_true", help="print one plain-text frame and exit")
    sub.add_parser("hosts")
    p = sub.add_parser("cancel")
    p.add_argument("job")
    p = sub.add_parser("approve", help="lab approve ec2 ID --max-hours H --max-usd D [--type T]")
    p.add_argument("what")
    p.add_argument("id")
    p.add_argument("--max-hours", type=float, required=True)
    p.add_argument("--max-usd", type=float, required=True)
    p.add_argument("--type")
    p = sub.add_parser("budget", help="lab budget: EC2 granted/spent/committed/remaining; "
                       "lab budget grant USD --note TEXT [--at ISO]: record Wrysk's authorization")
    p.add_argument("action", nargs="?", choices=["grant"])
    p.add_argument("usd", nargs="?", type=float)
    p.add_argument("--note", help="his words authorizing it (required for a grant)")
    p.add_argument("--at", help="when he authorized it, ISO date/time (default now)")
    p = sub.add_parser("wait", help="block until --job ends, or until any job ends / needs approval")
    p.add_argument("--job")
    p.add_argument("--any-new", action="store_true")
    p.add_argument("--interval", type=float, default=20)
    p.add_argument("--timeout", type=float)
    p = sub.add_parser("events")
    p.add_argument("-n", type=int, default=30)
    p.add_argument("--job")
    sub.add_parser("kinds")
    p = sub.add_parser("install", help="write ~/.config/systemd/user/labd.service and start it")
    p.add_argument("--no-start", action="store_true")
    p.add_argument("--print", action="store_true")
    p = sub.add_parser("daemon", help="run labd in the foreground")
    p.add_argument("--tick", type=float)
    args = parser.parse_args(argv)
    paths = Paths.from_env(args.state, args.cache, args.runs, args.hosts)
    handler = globals()[f"cmd_{args.command}"]
    try:
        return handler(args, paths) or 0
    except (SpecError, ValueError, OSError, subprocess.CalledProcessError) as error:
        print(f"lab: {error}", file=sys.stderr)
        return 1
