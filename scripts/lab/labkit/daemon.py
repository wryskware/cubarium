"""labd: ingest specs, plan, resolve inputs, preflight, place, launch, reconcile, finish.

The loop is single-threaded except for preflights and host probes (thread pool); all
state lives in sqlite plus each job's output dir, so the daemon can stop and restart at
any point. Tasks are separate systemd units and keep running while it is down; on start
the first poll reconciles them from their rc.json files and unit states.
"""

from concurrent.futures import ThreadPoolExecutor
import fcntl
import glob
import hashlib
import json
import os
from pathlib import Path
import re
import shlex
import shutil
import signal
import subprocess
import sys
import time
import traceback

from .core import Abort, Context, Job, Spawn, SpecError, Task, TaskView, load_kinds
from .hosts import (Host, Inventory, Usage, allowed, could_ever_fit, fits, mem_available_gb, prefers_local,
                    quick_timeout_h)
from .launch import LAUNCHERS, LocalTransport, SshTransport, systemd_user_works, write_file
from .store import TERMINAL, Events, Store, atomic_write, new_id
from . import budget
from .ec2 import Ec2Pool, estimate
from . import providers
from .stops import evaluate, run_hook, validate_rules

ACTIVE = ("submitted", "inputs", "preflight", "running", "aborting")
QUICK_LIMIT = "exceeded the quick limit"
QUICK_SLACK_S = 5    # an end this close to the cap counts as the cap (systemd RuntimeMaxSec vs our tick)


def quick_limit_reason(task, elapsed_s):
    return (f"{QUICK_LIMIT}: ran {elapsed_s / 60:.1f} min against a {task.timeout_h * 60:g} min cap "
            f"([lab] quick_timeout_min or the task's timeout_h); this was not a quick task, "
            f"resubmit it without quick")
TOOLS = ("taskwrap.py", "snaprun.py", "census_resume.py")


def unit_name(job, task):
    return re.sub(r"[^A-Za-z0-9_.-]", "-", f"lab-{job}-{task}")[:200]


def safe_key(key):
    return re.sub(r"[^A-Za-z0-9_.-]", "_", key)[:180]


class Daemon:
    def __init__(self, paths, inventory=None, kinds=None, clock=time.time, log=None):
        self.paths = paths.ensure()
        self.store = Store(paths)
        self.events = Events(paths, self.store)
        run_study = paths.repo / "scripts/run-study.py"
        self.inv = inventory or Inventory.load(paths.hosts, run_study)
        self.kinds = kinds if kinds is not None else load_kinds(self.inv.get("kind_paths", []))
        self.ctx = Context(paths, self.store, self.events, self.inv)
        self.clock = clock
        self.log = log or (lambda message: print(message, file=sys.__stdout__, flush=True))
        self.pool = ThreadPoolExecutor(max_workers=4, thread_name_prefix="labd")
        self.preflights = {}
        self.stop_hooks = {}
        self.probes = {}
        self.timers = {}
        self.transports = {}
        self.launchers = {}
        self.tools = set()
        self.stopping = False
        budget.migrate(self.store, paths.events)  # past ec2 spend from the event log, once
        self.ec2 = Ec2Pool(self)
        for host in self.inv.hosts.values():
            self._connect(host)

    # ------------------------------------------------------------------ hosts
    def hosts(self):
        hosts = list(self.inv.hosts.values())
        if self.ec2.host:
            hosts.append(self.ec2.host)
        return hosts

    def host(self, name):
        if self.ec2.host and name == self.ec2.host.name:
            return self.ec2.host
        return self.inv.hosts.get(name)

    def _connect(self, host, transport=None):
        if transport is None:
            local = host.kind == "local" or host.address in (None, "local")
            transport = LocalTransport() if local else SshTransport(host.address, host.ssh_options)
        self.transports[host.name] = transport
        python = self.ctx.python if host.kind == "local" else host.python
        choice = host.launcher if host.launcher != "auto" else "setsid"
        self.launchers[host.name] = LAUNCHERS[choice](transport, python)

    def _due(self, name, interval):
        now = self.clock()
        if now - self.timers.get(name, -1e18) >= interval:
            self.timers[name] = now
            return True
        return False

    def probe(self, host):
        """Seconds-long: reachability, home, load, L3 groups (this host's own sysfs), linger."""
        transport = self.transports[host.name]
        script = ('echo "home $HOME"; echo "nproc $(nproc)"; '
                  'echo "load $(cut -d" " -f1 /proc/loadavg)"; '
                  'echo "mem $(awk \'/MemAvailable/ {print $2}\' /proc/meminfo)"; '
                  'for f in /sys/devices/system/cpu/cpu*/cache/index3/shared_cpu_list; do echo "l3 $(cat $f)"; done | sort -u\n')
        done = transport.run(script, timeout=20)
        if done.returncode:
            return dict(up=False, error=done.stderr.strip()[-300:])
        info = dict(up=True, groups=[])
        for line in done.stdout.splitlines():
            word, _, value = line.partition(" ")
            if word == "l3":
                cpus = set()
                for part in value.split(","):
                    a, _, b = part.partition("-")
                    cpus.update(range(int(a), int(b or a) + 1))
                info["groups"].append(sorted(cpus))
            elif word in ("nproc", "mem"):
                info[word] = int(value or 0)
            elif word == "load":
                info[word] = float(value or 0)
            else:
                info[word] = value
        if host.launcher == "auto":
            works, detail = systemd_user_works(transport)
            info["launcher"] = "systemd" if works else "setsid"
            info["launcher_detail"] = detail
        return info

    def apply_probe(self, host, info):
        was = host.up
        host.up = info.get("up", False)
        if host.up:
            if info.get("groups"):
                host.groups = sorted(info["groups"])
                host.cpus = sum(map(len, host.groups))
            if host.root and host.root.startswith("~") and info.get("home"):
                host.root = info["home"] + host.root[1:]
            mine = sum(Task.from_dict(json.loads(r["spec"])).threads
                       for r in self.store.q("SELECT spec FROM tasks WHERE state='running' AND host=?", host.name))
            host.external_threads = max(0.0, info.get("load", 0) - mine)
            if "launcher" in info:
                launcher = self.launchers[host.name]
                if launcher.name != info["launcher"]:
                    self.launchers[host.name] = LAUNCHERS[info["launcher"]](launcher.transport, launcher.python)
        self.store.kv_set(f"host:{host.name}", dict(info, checked=self.clock()))
        if was != host.up or "announced" not in info:
            self.events.emit("host-up" if host.up else "host-down", host=host.name,
                             launcher=info.get("launcher"), detail=info.get("launcher_detail") or info.get("error"))

    def probe_hosts(self, force=False):
        for host in self.inv.hosts.values():
            if host.kind == "local" or not host.enabled:
                continue
            future = self.probes.get(host.name)
            if future and future.done():
                try:
                    info = future.result()
                except Exception as error:  # noqa: BLE001 - a probe never crashes the daemon
                    info = dict(up=False, error=str(error))
                previous = self.store.kv_get(f"host:{host.name}", {})
                if previous.get("up") == info.get("up") and previous.get("launcher") == info.get("launcher"):
                    info["announced"] = True
                self.apply_probe(host, info)
                del self.probes[host.name]
            elif not future and (force or self._due(f"probe:{host.name}", self.inv.get("probe_s", 300))):
                self.probes[host.name] = self.pool.submit(self.probe, host)

    PROC_STAT = "/proc/stat"
    LAB_CGROUPS = "/sys/fs/cgroup/user.slice/user-{uid}.slice/user@{uid}.service/*/lab-*.service/cpu.stat"

    def local_load(self):
        for host in self.inv.hosts.values():
            if host.kind == "local" and host.external_load:
                external = self.external_cpu()
                if external is None:
                    # No CPU sample yet (or no cgroups): loadavg minus labd's declared threads.
                    mine = sum(Task.from_dict(json.loads(r["spec"])).threads
                               for r in self.store.q("SELECT spec FROM tasks WHERE state='running' AND host=?",
                                                     host.name))
                    external = max(0.0, os.getloadavg()[0] - mine)
                host.external_threads = external

    def external_cpu(self):
        """Cores busy outside labd's task units, smoothed: total busy CPU (/proc/stat) minus the CPU
        the `lab-*` units used (their cgroups' cpu.stat). Loadavg counts runnable threads, so idle-class
        sims queueing for CPU read as outside load and starved every build (2026-10-10)."""
        try:
            with open(self.PROC_STAT) as f:
                fields = [int(x) for x in f.readline().split()[1:]]
            busy = sum(fields[:3]) + sum(fields[5:8])  # user nice system, irq softirq steal
            units = {}
            for path in glob.glob(self.LAB_CGROUPS.format(uid=os.getuid())):
                with open(path) as f:
                    for line in f:
                        if line.startswith("usage_usec"):
                            units[path] = int(line.split()[1])
        except (OSError, ValueError, IndexError):
            return None
        now = time.monotonic()
        prev = getattr(self, "_cpu_sample", None)
        self._cpu_sample = (now, busy, units)
        if prev is None or now <= prev[0]:
            return getattr(self, "_cpu_external", None)
        dt = now - prev[0]
        busy_cores = (busy - prev[1]) / os.sysconf("SC_CLK_TCK") / dt
        # A unit first seen in this interval counts its whole usage; one that ended drops out.
        lab_usec = sum(u - prev[2].get(p, 0) for p, u in units.items() if u >= prev[2].get(p, 0))
        sample = max(0.0, busy_cores - lab_usec / 1e6 / dt)
        last = getattr(self, "_cpu_external", None)
        self._cpu_external = sample if last is None else 0.8 * last + 0.2 * sample
        return self._cpu_external

    # ------------------------------------------------------------------ jobs
    def job(self, job_id, inputs=None):
        row = self.store.one("SELECT * FROM jobs WHERE id=?", job_id)
        return Job(row, self.store.q("SELECT * FROM tasks WHERE job=? ORDER BY seq", job_id), inputs) if row else None

    def save_kstate(self, job):
        self.store.update("jobs", dict(id=job.id), kstate=json.dumps(job.state))

    def insert_job(self, job_id, spec, submitted=None, parent=None):
        kind = self.kinds.get(spec.get("kind"))
        if kind is None:
            raise SpecError(f"unknown kind {spec.get('kind')!r}; known: {', '.join(sorted(self.kinds))}")
        spec = validate_rules(kind.validate(dict(spec), self.ctx), self.ctx, kind)
        out = Path(spec["out"]) if spec.get("out") else self.paths.runs / job_id
        self.store.x("INSERT INTO jobs(id, kind, name, state, spec, out, priority, submitted, parent) "
                     "VALUES(?, ?, ?, 'submitted', ?, ?, ?, ?, ?)", job_id, spec["kind"], spec.get("name"),
                     json.dumps(spec), str(out), int(spec.get("priority", 0)), submitted or self.clock(), parent)
        self.events.emit("submitted", job=job_id, kind=spec["kind"], name=spec.get("name"), parent=parent)
        return job_id

    def ingest(self):
        incoming = self.paths.state / "incoming"
        for file in sorted(incoming.iterdir()):
            if file.name.startswith(".") or file.suffix not in (".json", ".toml"):
                continue
            try:
                if file.suffix == ".toml":
                    import tomllib
                    payload = dict(spec=tomllib.loads(file.read_text()))
                    payload["spec"].setdefault("_cwd", str(self.paths.repo))
                else:
                    payload = json.loads(file.read_text())
                    if "spec" not in payload:
                        payload = dict(spec=payload)
                spec = payload["spec"]
                job_id = payload.get("id") or new_id(spec.get("kind", "job"), spec.get("name"))
            except (OSError, ValueError) as error:
                file.rename(self.paths.state / "rejected" / file.name)
                self.events.emit("rejected", file=file.name, reason=f"unreadable spec: {error}")
                continue
            if self.store.one("SELECT id FROM jobs WHERE id=?", job_id):
                file.unlink()
                continue
            try:
                self.insert_job(job_id, spec, payload.get("submitted"))
            except Exception as error:  # noqa: BLE001 - a bad spec is rejected, never fatal
                self.store.x("INSERT INTO jobs(id, kind, name, state, spec, out, submitted, finished, reason) "
                             "VALUES(?, ?, ?, 'rejected', ?, '', ?, ?, ?)", job_id, str(spec.get("kind")),
                             spec.get("name"), json.dumps(spec, default=str), self.clock(), self.clock(), str(error))
                self.events.emit("rejected", job=job_id, reason=str(error))
                file.rename(self.paths.state / "rejected" / file.name)
                continue
            file.unlink()

    def controls(self):
        control = self.paths.state / "control"
        for file in sorted(control.iterdir()):
            if file.name.startswith(".") or file.suffix != ".json":
                continue
            try:
                command = json.loads(file.read_text())
                if command["op"] == "cancel":
                    self.cancel(command["job"])
                elif command["op"] == "grant":
                    budget.grant(self.store, float(command["usd"]), command.get("note"),
                                 float(command.get("at") or self.clock()))
                    self.events.emit("budget-granted", usd=float(command["usd"]), note=command.get("note"),
                                     budget_left_usd=round(self.ec2.budget()["remaining"], 2))
                elif command["op"] == "approve" and command.get("what") == "ec2":
                    self.ec2.approve(command["id"], float(command["max_hours"]), float(command["max_usd"]),
                                     command.get("type"))
                else:
                    raise ValueError(f"unknown control {command}")
            except Exception as error:  # noqa: BLE001
                self.events.emit("control-error", file=file.name, reason=str(error))
            file.unlink(missing_ok=True)

    def cancel(self, job_id):
        row = self.store.one("SELECT state FROM jobs WHERE id=?", job_id)
        if not row:
            raise ValueError(f"no job {job_id}")
        if row["state"] in TERMINAL:
            return
        self.abort(job_id, "cancelled", final="cancelled")

    def abort(self, job_id, reason, final="aborted"):
        self.store.update("jobs", dict(id=job_id), state="aborting", reason=reason,
                          summary=json.dumps(dict(final=final)))
        self.preflights.pop(job_id, None)
        for row in self.store.q("SELECT * FROM tasks WHERE job=? AND state IN ('pending', 'running')", job_id):
            if row["state"] == "running":
                self.stop_task(row, "cancelled", reason)
            else:
                self.store.update("tasks", dict(job=job_id, name=row["name"]), state="skipped", reason=reason)
        self.events.emit("aborting", job=job_id, reason=reason)

    def stop_task(self, row, state, reason):
        launcher = self.launchers.get(row["host"])
        if launcher:
            try:
                launcher.stop(row["unit"], self.lab_dir(row))
            except Exception as error:  # noqa: BLE001
                reason = f"{reason}; stop failed: {error}"
        self.store.update("tasks", dict(job=row["job"], name=row["name"]), state=state,
                          reason=reason, finished=self.clock())

    def lab_dir(self, row):
        host = self.host(row["host"])
        if host and host.kind != "local" and host.root:
            return f"{host.root}/jobs/{row['job']}/.lab/{row['name']}"
        job = self.store.one("SELECT out FROM jobs WHERE id=?", row["job"])
        return f"{job['out']}/.lab/{row['name']}"

    def call(self, job, method, *args):
        """Call a kind hook; a kind error aborts its job, never the daemon."""
        try:
            return getattr(self.kinds[job.kind], method)(job, *args, self.ctx)
        except Exception as error:  # noqa: BLE001
            Path(job.out).mkdir(parents=True, exist_ok=True)
            with (Path(job.out) / "lab-error.log").open("a") as log:
                log.write(traceback.format_exc())
            return Abort(f"kind {job.kind}.{method} error: {error}")

    def placeable(self, task):
        hosts = list(self.inv.hosts.values()) + self.ec2.potential_hosts()
        return any(could_ever_fit(task, h) for h in hosts)

    def add_tasks(self, job, tasks):
        existing = self.store.q("SELECT name FROM tasks WHERE job=?", job.id)
        names = {r["name"] for r in existing}
        seq = len(existing)
        for task in tasks:
            if task.name in names:
                raise ValueError(f"duplicate task name {task.name}")
            if not self.placeable(task):
                raise ValueError(f"task {task.name} can never be placed ({task.threads} threads, "
                                 f"{task.mem_gb} GB, l3={task.l3}, arch={task.arch}, hosts={task.hosts})")
            names.add(task.name)
            self.store.x("INSERT INTO tasks(job, name, seq, state, spec) VALUES(?, ?, ?, 'pending', ?)",
                         job.id, task.name, seq, json.dumps(task.to_dict()))
            seq += 1

    def plan_jobs(self):
        for row in self.store.q("SELECT id FROM jobs WHERE state='submitted' ORDER BY submitted"):
            job = self.job(row["id"])
            job.out.mkdir(parents=True, exist_ok=True)
            tasks = self.call(job, "plan")
            if isinstance(tasks, Abort):
                self.save_kstate(job)
                self.abort(job.id, tasks.reason)
                continue
            try:
                self.add_tasks(job, tasks or [])
            except ValueError as error:
                self.save_kstate(job)
                self.abort(job.id, f"plan: {error}")
                continue
            self.save_kstate(job)
            self.store.update("jobs", dict(id=job.id), state="inputs")
            self.events.emit("planned", job=job.id, tasks=len(tasks or []))

    def needed_inputs(self, job):
        keys = list(self.call(job, "inputs") or [])
        if isinstance(keys, Abort):
            return keys
        for view in job.tasks.values():
            if view.state == "pending":
                keys += [k for k in view.task.inputs if k not in keys]
        return keys

    def resolve_inputs(self):
        for row in self.store.q("SELECT id, started FROM jobs WHERE state='inputs' ORDER BY submitted"):
            job = self.job(row["id"])
            keys = self.needed_inputs(job)
            if isinstance(keys, Abort):
                self.abort(job.id, keys.reason)
                continue
            missing, problem = [], None
            for key in keys:
                try:
                    if not self.ensure_input(key, job.id):
                        missing.append(key)
                except Exception as error:  # noqa: BLE001
                    problem = str(error)
                    break
            if problem:
                self.abort(job.id, problem)
            elif not missing:
                if row["started"]:
                    self.store.update("jobs", dict(id=job.id), state="running")
                    continue
                job.inputs = {k: str(self.ctx.artifact(k)) for k in keys}
                self.store.update("jobs", dict(id=job.id), state="preflight")
                self.preflights[job.id] = (self.pool.submit(self.call, job, "preflight"), job)

    def ensure_input(self, key, parent):
        """Use the normal provider path; return whether the artifact is ready now."""
        if self.ctx.artifact(key):
            return True
        provider = self.store.one("SELECT job FROM providers WHERE key=?", key)
        if provider:
            state = self.store.one("SELECT state, reason FROM jobs WHERE id=?", provider["job"])
            if state and state["state"] in TERMINAL:
                self.store.x("DELETE FROM providers WHERE key=?", key)
                raise SpecError(f"input {key}: provider job {provider['job']} ended {state['state']} ({state['reason']})")
            return False
        recipe = self.store.one("SELECT spec FROM recipes WHERE key=?", key)
        if not recipe:
            raise SpecError(f"no artifact and no recipe for input {key}")
        spec = providers.spec_for(key, json.loads(recipe["spec"]), self.ctx)
        child = self.insert_job(new_id(spec["kind"], spec.get("name")), spec, parent=parent)
        self.store.x("INSERT OR REPLACE INTO providers(key, job) VALUES(?, ?)", key, child)
        self.events.emit("input-wait", job=parent, key=key, provider=child)
        return False

    def home_task(self, task):
        """Retarget only task placement and binaries, preserving each input's source commit."""
        keys = {}
        for key in task.inputs:
            if not key.startswith("binary:"):
                continue
            row = self.store.one("SELECT spec FROM recipes WHERE key=?", key)
            if not row:
                return None
            recipe = json.loads(row["spec"])
            keys[key] = self.ctx.binary(recipe["repo"], recipe["commit"], recipe["name"], arch="x86_64",
                                        **{k: recipe[k] for k in ("package", "example", "bin", "features", "dirty")
                                           if k in recipe})
        if not keys:
            return None
        encoded = json.dumps(task.to_dict())
        for old, new in keys.items():
            encoded = encoded.replace(old, new)
        home = Task.from_dict(json.loads(encoded))
        home.arch = "x86_64"
        home.hosts = [name for name in (task.hosts or []) if name != "ec2"] or None
        home.remote_argv = None
        home.meta["moved_binaries"] = keys
        return home

    def collect_preflights(self):
        for job_id, (future, job) in list(self.preflights.items()):
            if not future.done():
                continue
            del self.preflights[job_id]
            result = future.result()
            self.save_kstate(job)
            if isinstance(result, Abort):
                self.events.emit("preflight-abort", job=job_id, reason=result.reason)
                self.abort(job_id, f"preflight: {result.reason}")
            else:
                self.store.update("jobs", dict(id=job_id), state="running", started=self.clock())
                self.events.emit("preflight-ok", job=job_id)

    # ------------------------------------------------------------------ tasks
    def poll_tasks(self, force=False):
        rows = self.store.q("SELECT * FROM tasks WHERE state='running'")
        by_host = {}
        for row in rows:
            by_host.setdefault(row["host"], []).append(row)
        for name, items in by_host.items():
            host = self.host(name)
            if host is None or not host.up:
                continue
            if host.kind != "local" and not (force or self._due(f"poll:{name}", self.inv.get("remote_poll_s", 30))):
                continue
            launcher = self.launchers[name]
            try:
                states = launcher.poll([(r["unit"], self.lab_dir(r)) for r in items])
            except Exception as error:  # noqa: BLE001
                self.log(f"poll {name}: {error}")
                continue
            for row in items:
                rc, alive = states.get(row["unit"], (None, True))
                if rc is not None:
                    self.task_ended(row, rc)
                elif not alive:
                    self.task_lost(row, "unit/process gone without rc.json")
                elif host.kind != "local":
                    try:
                        self.fetch_task(row)
                    except Exception as error:  # noqa: BLE001
                        self.log(f"progress fetch {row['job']}/{row['name']}: {error}")

    def task_ended(self, row, rc):
        job = self.job(row["job"])
        task = Task.from_dict(json.loads(row["spec"]))
        view = job.tasks[row["name"]]
        host = self.host(row["host"])
        reason = None
        if host and host.kind != "local":
            try:
                self.fetch_task(row, final=True)
            except Exception as error:  # noqa: BLE001
                self.log(f"fetch {job.id}/{view.name}: {error}; will retry")
                return  # Keep the rc and remote outputs until a complete fetch succeeds.
        code = rc.get("rc", 1)
        state = "done" if code == 0 else ("cancelled" if rc.get("cancelled") else "failed")
        if state == "done" and task.done_marker and not (view.out / task.done_marker).exists():
            state, reason = "failed", f"exit 0 but no done marker {task.done_marker}"
        if state == "done":
            for key, relative in task.produces.items():
                try:
                    self.register_artifact(key, view.out / relative, job.id)
                except OSError as error:
                    state, reason = "failed", f"artifact {key}: {error}"
        elapsed = rc.get("ended", self.clock()) - (row["started"] or self.clock())
        if task.quick and state != "done" and task.timeout_h \
                and elapsed >= task.timeout_h * 3600 - QUICK_SLACK_S:
            state, reason = "failed", quick_limit_reason(task, elapsed)  # systemd's RuntimeMaxSec got there first
        if reason is None and state != "done":
            reason = f"exit {code}"
        self.store.update("tasks", dict(job=row["job"], name=row["name"]), state=state, rc=code,
                          finished=rc.get("ended", self.clock()), reason=reason)
        if host and host.kind != "local":
            self.cleanup_task(row)
        # Measured peak beside the ask, so kind defaults (`[build] mem_gb`) can follow real use.
        self.events.emit("task-done" if state == "done" else "task-failed", job=job.id, task=row["name"],
                         host=row["host"], rc=code, reason=reason, kind=job.kind,
                         mem_gb=task.mem_gb, mem_peak_gb=rc.get("mem_peak_gb"))
        self.after_task(job.id, row["name"])

    def task_lost(self, row, reason, *, replacement=None):
        moving, fetched = replacement is not None, True
        task = Task.from_dict(json.loads(row["spec"]))
        host = self.host(row["host"])
        if moving:
            try:
                self.launchers[row["host"]].stop(row["unit"], self.lab_dir(row))
            except Exception as error:  # noqa: BLE001
                self.log(f"tail stop {row['job']}/{row['name']}: {error}")
                return False
        if host and host.kind != "local" and host.up:
            try:
                self.fetch_task(row)  # Retain the latest checkpoint before a retry changes hosts.
                self.cleanup_task(row)
            except Exception as error:  # noqa: BLE001
                self.log(f"lost task fetch {row['job']}/{row['name']}: {error}")
                fetched = False
        if moving and fetched:
            task = replacement
        if moving or row["attempts"] <= task.retries:
            if task.resume_argv:
                task.argv = task.resume_argv
            changes = dict(attempts=max(0, row["attempts"] - 1), finished=None) if moving else {}
            self.store.update("tasks", dict(job=row["job"], name=row["name"]), state="pending",
                              spec=json.dumps(task.to_dict()), reason=f"requeued: {reason}", **changes)
            self.events.emit("requeued", job=row["job"], task=row["name"], host=row["host"], reason=reason)
            return fetched
        self.store.update("tasks", dict(job=row["job"], name=row["name"]), state="lost",
                          finished=self.clock(), reason=reason)
        self.events.emit("task-lost", job=row["job"], task=row["name"], host=row["host"], reason=reason)
        self.after_task(row["job"], row["name"])
        return False

    def after_task(self, job_id, name):
        job = self.job(job_id)
        if job.status not in ("running", "inputs", "preflight"):
            return
        if self.check_stops(job):
            return
        self.handle(job, self.call(job, "progress", job.tasks[name]))

    def fire_stop(self, job, result):
        job.state.setdefault("_lab_stop", {})["fired"] = result
        self.save_kstate(job)
        self.events.emit("stopped", job=job.id, **result)
        self.abort(job.id, result["reason"], final="done" if result["action"] == "finish" else "aborted")

    def check_stops(self, job):
        """Evaluate mirrored census output, and schedule due local hooks off the loop."""
        if not job.spec.get("stop"):
            return False
        try:
            result = (evaluate(job, self.kinds[job.kind].census_runs(job, self.ctx))
                      if any("when" in rule for rule in job.spec["stop"]) else None)
        except Exception as error:  # noqa: BLE001 - bad conditions are visible job errors
            reason = f"stop rule error: {error}"
            self.log(f"{job.id}: {reason}")
            self.events.emit("stop-error", job=job.id, reason=reason)
            self.abort(job.id, reason)
            return True
        if result:
            self.fire_stop(job, result)
            return True
        state = job.state.setdefault("_lab_stop", {}).setdefault("hooks", {})
        now = self.clock()
        for index, rule in enumerate(job.spec["stop"]):
            key = (job.id, index)
            if "hook" not in rule or key in self.stop_hooks:
                continue
            if str(index) in state and now - state[str(index)] < rule["every_min"] * 60:
                continue
            state[str(index)] = now
            self.stop_hooks[key] = self.pool.submit(run_hook, rule, job.out)
        self.save_kstate(job)
        return False

    def collect_stop_hooks(self):
        for (job_id, index), future in list(self.stop_hooks.items()):
            if not future.done():
                continue
            del self.stop_hooks[job_id, index]
            job = self.job(job_id)
            try:
                result = future.result()
            except Exception as error:  # noqa: BLE001 - a bad hook means continue
                reason = f"stop hook {index}: {error}"
                self.log(f"{job_id}: {reason}")
                self.events.emit("stop-hook-error", job=job_id, rule=index, reason=reason)
                continue
            if result and job.status == "running":
                self.fire_stop(job, dict(rule=index, action=result["action"], reason=result["reason"]))

    def handle(self, job, result):
        self.save_kstate(job)
        if isinstance(result, Abort):
            self.abort(job.id, result.reason)
        elif isinstance(result, Spawn):
            try:
                self.add_tasks(job, result.tasks)
            except ValueError as error:
                self.abort(job.id, f"spawn: {error}")
                return
            self.events.emit("spawned", job=job.id, tasks=[t.name for t in result.tasks])
            if any(not self.ctx.artifact(k) for t in result.tasks for k in t.inputs):
                self.store.update("jobs", dict(id=job.id), state="inputs")

    def register_artifact(self, key, source, job_id):
        from .core import binary_key  # noqa: F401 - keys are opaque here
        if not source.is_file():
            raise OSError(f"missing {source}")
        destination = self.paths.cache / "artifacts" / safe_key(key) / source.name
        destination.parent.mkdir(parents=True, exist_ok=True)
        temporary = destination.with_name(f".{source.name}.tmp")
        subprocess.run(["cp", "--reflink=auto", "-p", str(source), str(temporary)], check=True)
        temporary.replace(destination)
        meta = {}
        if (source.parent / "meta.json").is_file():
            meta = json.loads((source.parent / "meta.json").read_text())
        from .snaprun import sha256
        self.store.x("INSERT OR REPLACE INTO artifacts(key, path, sha256, meta, created) VALUES(?, ?, ?, ?, ?)",
                     key, str(destination), sha256(destination), json.dumps(dict(meta, job=job_id)), self.clock())
        self.events.emit("artifact", job=job_id, key=key, path=str(destination))

    def cadence(self):
        interval = self.inv.get("progress_s", 60)
        now = self.clock()
        for row in self.store.q("SELECT id, last_progress FROM jobs WHERE state='running'"):
            if now - (row["last_progress"] or 0) < interval:
                continue
            self.store.update("jobs", dict(id=row["id"]), last_progress=now)
            job = self.job(row["id"])
            for view in job.tasks.values():
                if view.state == "running" and view.task.liveness:
                    self.check_liveness(job, view)
            job = self.job(row["id"])
            if job.status == "running":
                if not self.check_stops(job):
                    self.handle(job, self.call(job, "progress", None))

    def quick_timeouts(self):
        """Quick tasks end at their capped timeout on every launcher (setsid has no RuntimeMaxSec)."""
        now = self.clock()
        for row in self.store.q("SELECT * FROM tasks WHERE state='running'"):
            task = Task.from_dict(json.loads(row["spec"]))
            if not task.quick or not task.timeout_h or row["started"] is None:
                continue
            elapsed = now - row["started"]
            if elapsed < task.timeout_h * 3600:
                continue
            reason = quick_limit_reason(task, elapsed)
            self.stop_task(row, "failed", reason)
            self.events.emit("task-failed", job=row["job"], task=row["name"], host=row["host"], reason=reason)
            self.after_task(row["job"], row["name"])

    def check_liveness(self, job, view):
        row = self.store.one("SELECT * FROM tasks WHERE job=? AND name=?", job.id, view.name)
        spec = view.task.liveness
        host = self.host(row["host"])
        out = view.out
        if host and host.kind != "local" and not spec.get("minute_file"):
            script = (f"find {shlex.quote(self.remote_out(host, job, view))} -type f -printf '%s %T@\\n' 2>/dev/null"
                      " | awk '{n++; s+=$1; if ($2>m) m=$2} END {print n+0, s+0, m+0}'\n")
            done = self.transports[host.name].run(script, timeout=30)
            signature = done.stdout.split() if done.returncode == 0 else None
        elif spec.get("minute_file"):
            try:
                match = re.search(r"\bminute=(\d+)", (out / spec["minute_file"]).read_text())
                signature = int(match[1]) if match else None
            except OSError:
                signature = None
        else:
            signature = Context.output_signature([str(out / p) for p in spec.get("paths", ["**/*"])])
        live = json.loads(row["live"] or "{}")
        now = self.clock()
        if signature is not None and signature != live.get("sig"):
            live = dict(sig=signature, since=now)
            self.store.update("tasks", dict(job=job.id, name=view.name), live=json.dumps(live))
            return
        since = live.get("since") or row["started"] or now
        if now - since >= float(spec.get("stall_min", 30)) * 60:
            reason = f"stalled: no output growth for {spec.get('stall_min', 30)} min"
            self.stop_task(row, "stalled", reason)
            self.events.emit("task-stalled", job=job.id, task=view.name, reason=reason)
            self.after_task(job.id, view.name)

    def remote_out(self, host, job, view):
        relative = view.out.relative_to(job.out)
        return f"{host.root}/jobs/{job.id}/{relative.as_posix()}"

    def fetch_task(self, row, final=False):
        """Mirror outputs before hooks read them; rsync transfers only changed files."""
        host = self.host(row["host"])
        if not host or host.kind == "local":
            return
        job = self.job(row["job"])
        view = job.tasks[row["name"]]
        remote = self.remote_out(host, job, view)
        transport = self.transports[host.name]
        transport.get(remote, view.out)
        if view.task.meta.get("batch") and (view.out / "batch/batch.json").is_file():
            manifest_path = view.out / "batch/batch.json"
            manifest = json.loads(manifest_path.read_text())
            manifest["lab_paths"] = view.task.meta.get("remote_paths", {})
            manifest_path.write_text(json.dumps(manifest, indent=2) + "\n")
        if final:
            lab = self.lab_dir(row)
            transport.get(lab, job.out / ".lab" / view.name)

    def cleanup_task(self, row):
        host = self.host(row["host"])
        job = self.job(row["job"])
        remote = self.remote_out(host, job, job.tasks[row["name"]])
        try:
            done = self.transports[host.name].run(
                f"rm -rf -- {shlex.quote(remote)} {shlex.quote(self.lab_dir(row))}\n")
            if done.returncode:
                self.log(f"remote cleanup {job.id}/{row['name']}: {done.stderr.strip()}")
        except Exception as error:  # noqa: BLE001
            self.log(f"remote cleanup {job.id}/{row['name']}: {error}")

    def usage(self):
        usage = {h.name: Usage() for h in self.hosts()}
        for row in self.store.q("SELECT host, grp, spec FROM tasks WHERE state='running'"):
            if row["host"] in usage:
                usage[row["host"]].add(Task.from_dict(json.loads(row["spec"])), row["grp"])
        return usage

    def schedule(self):
        self.local_load()
        usage = self.usage()
        live_mem = mem_available_gb()
        rows = self.store.q(
            "SELECT t.*, j.priority AS jp, j.kind AS jk, j.spec AS js FROM tasks t JOIN jobs j ON t.job=j.id "
            "WHERE t.state='pending' AND j.state='running' ORDER BY j.submitted, t.seq, j.id")
        # Stable sorting preserves FIFO when job + task priorities are equal.
        rows = [(row, Task.from_dict(json.loads(row["spec"]))) for row in rows]
        # Tasks planned before kinds marked themselves (build lane) take their job's kind.
        quick_min = self.inv.get("quick_timeout_min", 15)
        for row, task in rows:
            task.meta.setdefault("kind", row["jk"])
            # quick = true on the job spec applies to all its tasks; quick tasks get the time cap.
            task.quick = bool(task.quick or json.loads(row["js"] or "{}").get("quick"))
            if task.quick:
                task.timeout_h = quick_timeout_h(task, quick_min)
        rows.sort(key=lambda item: item[0]["jp"] + item[1].priority, reverse=True)
        states = {}
        reservations = {}
        now = self.clock()
        for row, task in rows:
            priority = row["jp"] + task.priority
            key = dict(job=row["job"], name=row["name"])
            self.store.update("tasks", key, reason=None)
            if row["job"] not in states:
                states[row["job"]] = {r["name"]: r["state"] for r in self.store.q(
                    "SELECT name, state FROM tasks WHERE job=?", row["job"])}
            if any(states[row["job"]].get(d) != "done" for d in task.deps):
                continue
            # Most free threads first; quick tasks and tasks with local inputs try local hosts first.
            local_first = prefers_local(task)
            candidates = sorted(self.hosts(), key=lambda h: (
                local_first and h.kind != "local",
                -(h.usable - usage[h.name].threads - (h.external_threads if h.external_load else 0))))
            for host in candidates:
                group = fits(task, host, usage[host.name], live_mem if host.kind == "local" else None)
                if group is None:
                    continue
                # A quick task never waits behind a reservation (its own pool is above usable).
                if host.name in reservations and not task.quick:
                    if any(priority >= reserved_priority or deadline is None or task.est_h is None
                           or task.est_h <= 0 or now + task.est_h * 3600 > deadline
                           for reserved_priority, deadline in reservations[host.name]):
                        continue
                if self.launch(row, task, host, group):
                    usage[host.name].add(task, group)
                break
            else:
                eligible = [h for h in candidates if h.up and could_ever_fit(task, h)
                            and (not task.l3 or h.groups)]
                if eligible and task.quick:
                    # Quick tasks hold no reservation: their pool turns over within the time cap.
                    self.store.update("tasks", key, reason=f"waiting for {task.threads} threads on "
                                      f"{eligible[0].name} (quick pool)")
                elif eligible:
                    host = eligible[0]
                    reservations.setdefault(host.name, []).append(
                        (priority, self.reservation_start(task, host, live_mem, now)))
                    self.store.update("tasks", key,
                                      reason=f"waiting for {task.threads} threads on {host.name} (reserved)")

    def reservation_start(self, task, host, live_mem, now):
        """Earliest estimated fit; unestimated/overdue tasks and external load stay charged."""
        running = []
        for row in self.store.q("SELECT spec, grp, started FROM tasks WHERE state='running' AND host=?", host.name):
            other = Task.from_dict(json.loads(row["spec"]))
            end = row["started"] + other.est_h * 3600 if row["started"] is not None and other.est_h else None
            running.append((other, row["grp"], end if end is not None and end > now else None))
        for end in sorted({end for _, _, end in running if end is not None}):
            remaining = Usage()
            freed_mem = 0
            for other, group, finish in running:
                if finish is None or finish > end:
                    remaining.add(other, group)
                else:
                    freed_mem += other.mem_gb
            memory = live_mem + freed_mem if host.kind == "local" else None
            if fits(task, host, remaining, memory) is not None:
                return end
        return None

    def ensure_tools(self, host, transport):
        if host.name in self.tools:
            return
        for name in TOOLS:
            transport.put(self.ctx.lab_dir / name, f"{host.root}/tools/{name}")
        transport.put(self.ctx.run_study, f"{host.root}/tools/run-study.py")
        for name in ("batch.py", "study_contract.py"):
            transport.put(self.ctx.repo / "scripts/studies" / name, f"{host.root}/tools/studies/{name}")
        self.tools.add(host.name)

    def launch(self, row, task, host, group):
        job = self.job(row["job"])
        view = job.tasks[row["name"]]
        transport = self.transports[host.name]
        launcher = self.launchers[host.name]
        try:
            local_out = view.out
            local_out.mkdir(parents=True, exist_ok=True)
            if host.kind == "local":
                out, jobdir, tools = str(local_out), str(job.out), str(self.ctx.lab_dir)
                run_study = str(self.ctx.run_study)
                inputs = {k: str(self.ctx.artifact(k)) for k in task.inputs}
            else:
                if not host.root:
                    raise RuntimeError(f"host {host.name} has no root")
                self.ensure_tools(host, transport)
                out, jobdir = self.remote_out(host, job, view), f"{host.root}/jobs/{job.id}"
                tools = run_study = f"{host.root}/tools"
                run_study += "/run-study.py"
                inputs = {}
                for key in task.inputs:
                    source = self.ctx.artifact(key)
                    remote = f"{host.root}/artifacts/{safe_key(key)}/{source.name}"
                    mark = f"uploaded:{host.name}:{key}"
                    if self.store.kv_get(mark) != host.root:
                        transport.put(source, remote)
                        self.store.kv_set(mark, host.root)
                    inputs[key] = remote
                if any(local_out.iterdir()):
                    transport.put_dir(local_out, out)  # retries and an explicit resume both carry checkpoints
            lab = f"{jobdir}/.lab/{task.name}"

            replacements = {}
            if host.kind != "local":
                replacements[str(job.out)] = jobdir
                replacements[str(self.ctx.repo / "scripts/studies/batch.py")] = f"{tools}/studies/batch.py"
                replacements.update({str(self.ctx.artifact(k)): v for k, v in inputs.items()})

            def subst(value):
                value = str(value)
                for source in sorted(replacements, key=len, reverse=True):
                    value = value.replace(source, replacements[source])
                value = str(value).replace("{out}", out).replace("{job}", jobdir).replace("{lab}", tools)
                value = value.replace("{python}", launcher.python)
                return re.sub(r"\{input:([^}]+)\}", lambda m: inputs[m[1]], value)

            if host.kind != "local":
                for source, destination in task.files.items():
                    remote = os.path.normpath(subst(destination))
                    if not Path(remote).is_relative_to(Path(jobdir)):
                        raise ValueError(f"staged input escapes remote job root: {destination}")
                    if Path(source).is_dir():
                        transport.put_dir(source, remote)
                    else:
                        transport.put(source, remote)
                    replacements[str(source)] = remote
            # Saved batch commands/identities contain absolute paths from the previous host.
            manifest_path = local_out / "batch/batch.json"
            current_paths = dict(replacements)
            for previous_key, current_key in task.meta.get("moved_binaries", {}).items():
                previous_binary = self.ctx.artifact(previous_key)
                if previous_binary:
                    current_paths[str(previous_binary)] = inputs[current_key]
            # Staged census arguments carry vitals/config paths from the previous host.
            for source, previous in task.meta.get("remote_paths", {}).items():
                replacements[previous] = current_paths.get(source, source)
            replacements.update(current_paths)
            if task.resume_argv:
                argfiles = [local_out / "run/args", *local_out.glob("batch/*/s*/run/args")]
                for argfile in argfiles:
                    if argfile.is_file():
                        relocated = subst(argfile.read_text())
                        write_file(transport, f"{out}/{argfile.relative_to(local_out)}", relocated)
            if task.meta.get("batch") and manifest_path.is_file():
                manifest = json.loads(manifest_path.read_text())
                for source, previous in manifest.get("lab_paths", task.meta.get("remote_paths", {})).items():
                    replacements[previous] = current_paths.get(source, source)

                def relocate(value):
                    if isinstance(value, str):
                        return subst(value)
                    if isinstance(value, list):
                        return [relocate(v) for v in value]
                    if isinstance(value, dict):
                        return {subst(k): relocate(v) for k, v in value.items()}
                    return value

                manifest = relocate(manifest)
                if task.meta.get("moved_binaries"):
                    # Only home_task can authorize this: the exact source commit is unchanged.
                    binary = self.ctx.artifact(next(k for k in task.inputs if k.startswith("binary:")))
                    manifest["binary_sha256"] = hashlib.sha256(binary.read_bytes()).hexdigest()
                manifest["lab_paths"] = current_paths
                write_file(transport, f"{out}/batch/batch.json", json.dumps(manifest, indent=2))
            if task.resume_argv:
                task.meta["remote_paths"] = current_paths

            env = {}
            if host.kind == "local":
                env = {k: os.environ[k] for k in ("PATH", "HOME", "USER", "LANG", "CARGO_HOME", "RUSTUP_HOME")
                       if k in os.environ}
            env.update({k: subst(v) for k, v in task.env.items()})
            argv = task.remote_argv if host.kind != "local" and task.remote_argv else task.argv
            spec = dict(argv=[subst(a) for a in argv], cwd=subst(task.cwd) if task.cwd else out,
                        env=env, stdout=subst(task.stdout or "{out}/stdout.log"),
                        stderr=subst(task.stderr or "{out}/stderr.log"), lab_dir=lab,
                        group=group if group >= 0 else None,
                        cpus=host.groups[group] if host.kind == "local" and group >= 0 else None,
                        idle=host.idle, rlimit_as_gb=task.rlimit_as_gb, run_study=run_study,
                        hidden_script=str(self.paths.repo / "scripts/hidden.sh") if task.hidden else None)
            if task.hidden and host.kind != "local":
                raise RuntimeError("hidden tasks run only on the local host")
            write_file(transport, f"{lab}/launch.json", json.dumps(spec, indent=1))
            transport.run(f"mkdir -p {shlex.quote(spec['cwd'])} {shlex.quote(out)}\n")
            unit = unit_name(job.id, task.name)
            self.store.update("tasks", dict(job=job.id, name=task.name), state="running", host=host.name,
                              grp=group, unit=unit, launcher=launcher.name, started=self.clock(),
                              attempts=row["attempts"] + 1, live=json.dumps(dict(since=self.clock())),
                              spec=json.dumps(task.to_dict()))
            launcher.start(unit, f"{lab}/launch.json", lab, spec["cwd"], mem_gb=host.max_mem_gb,
                           timeout_h=task.timeout_h, taskwrap=f"{tools}/taskwrap.py")
        except Exception as error:  # noqa: BLE001
            self.store.update("tasks", dict(job=job.id, name=task.name), state="failed", host=host.name,
                              reason=f"launch: {error}", finished=self.clock())
            self.events.emit("task-failed", job=job.id, task=task.name, host=host.name, reason=f"launch: {error}")
            self.after_task(job.id, task.name)
            return False
        self.events.emit("started", job=job.id, task=task.name, host=host.name,
                         group=group if group >= 0 else None, unit=unit, launcher=launcher.name)
        return True

    def finish_jobs(self):
        for row in self.store.q("SELECT id, state FROM jobs WHERE state IN ('running', 'aborting')"):
            if row["id"] in self.preflights:
                continue
            if row["state"] == "running" and any(key[0] == row["id"] for key in self.stop_hooks):
                continue
            busy = self.store.one("SELECT COUNT(*) AS n FROM tasks WHERE job=? AND state IN ('pending', 'running')",
                                  row["id"])["n"]
            if busy:
                continue
            job = self.job(row["id"])
            fetching = False
            for view in job.tasks.values():
                host = self.host(view.host)
                stopped = self.store.one("SELECT * FROM tasks WHERE job=? AND name=?", job.id, view.name)
                killed = view.state in ("cancelled", "stalled") or (
                    view.state == "failed" and (view.reason or "").startswith(QUICK_LIMIT))
                if not host or host.kind == "local" or not host.up or not stopped["unit"] \
                        or not killed or view.rc is not None:
                    continue
                try:
                    rc, alive = self.launchers[host.name].poll([(stopped["unit"], self.lab_dir(stopped))])[stopped["unit"]]
                    if rc is None and alive:
                        fetching = True
                        continue
                    self.fetch_task(stopped, final=True)
                    self.store.update("tasks", dict(job=job.id, name=view.name), rc=(rc or {}).get("rc", 1))
                    self.cleanup_task(stopped)
                except Exception as error:  # noqa: BLE001
                    self.log(f"stopped task fetch {job.id}/{view.name}: {error}")
                    fetching = True
            if fetching:
                continue
            job = self.job(row["id"])
            summary = self.call(job, "finish")
            self.save_kstate(job)
            if isinstance(summary, Abort):
                summary = summary.reason
            summary = str(summary or "")
            if row["state"] == "aborting":
                final = json.loads(self.store.one("SELECT summary FROM jobs WHERE id=?", job.id)["summary"]
                                   or "{}").get("final", "aborted")
                reason = (f"stopped (finish): {job.reason}" if final == "done" else
                          job.reason if final == "cancelled" else f"aborted: {job.reason}")
            else:
                final, reason = "done", "ok: " + (summary.splitlines()[0] if summary else "finished")
            table = "\n".join(f"{v.name}\t{v.state}\t{v.host or '-'}\t{v.rc if v.rc is not None else '-'}\t{v.reason or ''}"
                              for v in job.tasks.values())
            atomic_write(job.out / "summary.txt", f"{reason}\n\n{summary}\n\ntask\tstate\thost\trc\treason\n{table}\n")
            atomic_write(job.out / "DONE", reason + "\n")
            self.store.update("jobs", dict(id=job.id), state=final, finished=self.clock(), reason=reason,
                              summary=reason + "\n" + summary if job.state.get("_lab_stop", {}).get("fired") else summary)
            for name in {v.host for v in job.tasks.values() if v.host}:
                host = self.host(name)
                views = [v for v in job.tasks.values() if v.host == name]
                if host and host.kind != "local" and all(v.rc is not None for v in views):
                    try:
                        # Shared config/checkpoint inputs outlive individual tasks, until all hooks finish.
                        self.transports[name].run(f"rm -rf -- {shlex.quote(host.root + '/jobs/' + job.id)}\n")
                    except Exception as error:  # noqa: BLE001
                        self.log(f"remote job cleanup {job.id}: {error}")
            self.events.emit(final, job=job.id, kind=job.kind, reason=reason, out=str(job.out))

    # ------------------------------------------------------------------ janitor
    def janitor(self):
        trees = self.paths.cache / "trees"
        keep = int(self.inv.build.get("keep_trees", 3))
        if not trees.is_dir() or not self._due("janitor", 600):
            return
        busy = {json.loads(r["spec"]).get("meta", {}).get("tree")
                for r in self.store.q("SELECT spec FROM tasks WHERE state IN ('running', 'pending')")}
        candidates = sorted((t for t in trees.iterdir() if t.is_dir() and not t.name.endswith(".partial")),
                            key=lambda t: t.stat().st_mtime, reverse=True)
        for tree in candidates[keep:]:
            if str(tree) in busy:
                continue
            lock = trees / f"{tree.name}.lock"
            with open(lock, "w") as stream:
                try:
                    fcntl.flock(stream, fcntl.LOCK_EX | fcntl.LOCK_NB)
                except BlockingIOError:
                    continue
                done = subprocess.run(["git", "-C", str(tree), "worktree", "remove", "--force", str(tree)],
                                      capture_output=True, timeout=600)
                if done.returncode:
                    shutil.rmtree(tree, ignore_errors=True)
            self.events.emit("tree-pruned", path=str(tree))

    # ------------------------------------------------------------------ loop
    def step(self):
        self.ingest()
        self.controls()
        self.plan_jobs()
        self.resolve_inputs()
        self.collect_preflights()
        self.probe_hosts()
        self.poll_tasks()
        self.quick_timeouts()
        self.collect_stop_hooks()
        self.cadence()
        self.schedule()
        self.ec2.tick()
        self.finish_jobs()
        self.janitor()

    def start(self):
        running = self.store.one("SELECT COUNT(*) AS n FROM tasks WHERE state='running'")["n"]
        self.events.emit("daemon-start", pid=os.getpid(), running_tasks=running)
        # Jobs left mid-preflight by a previous daemon preflight again (seconds-long).
        self.store.x("UPDATE jobs SET state='inputs' WHERE state='preflight'")
        self.probe_hosts(force=True)
        for future in list(self.probes.values()):
            try:
                future.result(timeout=30)
            except Exception:  # noqa: BLE001
                pass
        self.probe_hosts()
        self.ec2.reconcile()
        self.poll_tasks(force=True)

    def run(self, until=None, tick=None):
        tick = tick if tick is not None else float(self.inv.get("tick_s", 2))
        lock = open(self.paths.state / "labd.lock", "w")
        try:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            raise SystemExit("labd: another daemon owns this state dir")
        signal.signal(signal.SIGTERM, lambda *_: setattr(self, "stopping", True))
        signal.signal(signal.SIGINT, lambda *_: setattr(self, "stopping", True))
        self.start()
        try:
            while not self.stopping and (until is None or not until()):
                try:
                    self.step()
                except Exception:  # noqa: BLE001 - log, keep owning the jobs
                    self.log(traceback.format_exc())
                time.sleep(tick)
        finally:
            self.events.emit("daemon-stop", pid=os.getpid())
            self.pool.shutdown(wait=False, cancel_futures=True)
            lock.close()

    def eta(self):
        pending = [Task.from_dict(json.loads(r["spec"])) for r in self.store.q(
            "SELECT t.spec FROM tasks t JOIN jobs j ON t.job=j.id WHERE t.state='pending' AND j.state IN "
            "('running','inputs','preflight')")]
        running = [(Task.from_dict(json.loads(r["spec"])), self.clock() - (r["started"] or self.clock()))
                   for r in self.store.q("SELECT spec, started FROM tasks WHERE state='running'")]
        local = [h for h in self.inv.hosts.values() if h.up and h.kind != "ec2"]
        return estimate(pending, running, local)
