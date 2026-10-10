"""labd: ingest specs, plan, resolve inputs, preflight, place, launch, reconcile, finish.

The loop is single-threaded except for preflights and host probes (thread pool); all
state lives in sqlite plus each job's output dir, so the daemon can stop and restart at
any point. Tasks are separate systemd units and keep running while it is down; on start
the first poll reconciles them from their rc.json files and unit states.
"""

from concurrent.futures import ThreadPoolExecutor
import fcntl
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
from .hosts import Host, Inventory, Usage, allowed, could_ever_fit, fits, mem_available_gb
from .launch import LAUNCHERS, LocalTransport, SshTransport, systemd_user_works, write_file
from .store import TERMINAL, Events, Store, atomic_write, new_id
from .ec2 import Ec2Pool, estimate
from . import providers

ACTIVE = ("submitted", "inputs", "preflight", "running", "aborting")
TOOLS = ("taskwrap.py", "snaprun.py")


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
        self.probes = {}
        self.timers = {}
        self.transports = {}
        self.launchers = {}
        self.tools = set()
        self.stopping = False
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

    def local_load(self):
        for host in self.inv.hosts.values():
            if host.kind == "local" and host.external_load:
                mine = sum(Task.from_dict(json.loads(r["spec"])).threads
                           for r in self.store.q("SELECT spec FROM tasks WHERE state='running' AND host=?", host.name))
                host.external_threads = max(0.0, os.getloadavg()[0] - mine)

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
        spec = kind.validate(dict(spec), self.ctx)
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
                if self.ctx.artifact(key):
                    continue
                missing.append(key)
                provider = self.store.one("SELECT job FROM providers WHERE key=?", key)
                if provider:
                    state = self.store.one("SELECT state, reason FROM jobs WHERE id=?", provider["job"])
                    if state and state["state"] in TERMINAL:
                        problem = f"input {key}: provider job {provider['job']} ended {state['state']} ({state['reason']})"
                        self.store.x("DELETE FROM providers WHERE key=?", key)
                        break
                    continue
                recipe = self.store.one("SELECT spec FROM recipes WHERE key=?", key)
                if not recipe:
                    problem = f"no artifact and no recipe for input {key}"
                    break
                try:
                    spec = providers.spec_for(key, json.loads(recipe["spec"]), self.ctx)
                    child = self.insert_job(new_id(spec["kind"], spec.get("name")), spec, parent=job.id)
                except Exception as error:  # noqa: BLE001
                    problem = f"input {key}: cannot submit provider: {error}"
                    break
                self.store.x("INSERT OR REPLACE INTO providers(key, job) VALUES(?, ?)", key, child)
                self.events.emit("input-wait", job=job.id, key=key, provider=child)
            if problem:
                self.abort(job.id, problem)
            elif not missing:
                if row["started"]:
                    self.store.update("jobs", dict(id=job.id), state="running")
                    continue
                job.inputs = {k: str(self.ctx.artifact(k)) for k in keys}
                self.store.update("jobs", dict(id=job.id), state="preflight")
                self.preflights[job.id] = (self.pool.submit(self.call, job, "preflight"), job)

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

    def task_ended(self, row, rc):
        job = self.job(row["job"])
        task = Task.from_dict(json.loads(row["spec"]))
        view = job.tasks[row["name"]]
        host = self.host(row["host"])
        reason = None
        if host and host.kind != "local":
            try:
                self.transports[host.name].get(self.remote_out(host, job, view), view.out)
            except Exception as error:  # noqa: BLE001
                reason = f"fetch failed: {error}"
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
        if reason is None and state != "done":
            reason = f"exit {code}"
        self.store.update("tasks", dict(job=row["job"], name=row["name"]), state=state, rc=code,
                          finished=rc.get("ended", self.clock()), reason=reason)
        self.events.emit("task-done" if state == "done" else "task-failed", job=job.id, task=row["name"],
                         host=row["host"], rc=code, reason=reason)
        self.after_task(job.id, row["name"])

    def task_lost(self, row, reason):
        task = Task.from_dict(json.loads(row["spec"]))
        if row["attempts"] <= task.retries:
            if task.resume_argv:
                task.argv = task.resume_argv
            self.store.update("tasks", dict(job=row["job"], name=row["name"]), state="pending",
                              spec=json.dumps(task.to_dict()), reason=f"requeued: {reason}")
            self.events.emit("requeued", job=row["job"], task=row["name"], host=row["host"], reason=reason)
            return
        self.store.update("tasks", dict(job=row["job"], name=row["name"]), state="lost",
                          finished=self.clock(), reason=reason)
        self.events.emit("task-lost", job=row["job"], task=row["name"], host=row["host"], reason=reason)
        self.after_task(row["job"], row["name"])

    def after_task(self, job_id, name):
        job = self.job(job_id)
        if job.status not in ("running", "inputs", "preflight"):
            return
        self.handle(job, self.call(job, "progress", job.tasks[name]))

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
                self.handle(job, self.call(job, "progress", None))

    def check_liveness(self, job, view):
        row = self.store.one("SELECT * FROM tasks WHERE job=? AND name=?", job.id, view.name)
        spec = view.task.liveness
        host = self.host(row["host"])
        out = view.out
        if host and host.kind != "local":
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
            "SELECT t.*, j.priority AS jp FROM tasks t JOIN jobs j ON t.job=j.id "
            "WHERE t.state='pending' AND j.state='running' ORDER BY j.priority DESC, j.submitted, t.seq")
        states = {}
        for row in rows:
            task = Task.from_dict(json.loads(row["spec"]))
            if row["job"] not in states:
                states[row["job"]] = {r["name"]: r["state"] for r in self.store.q(
                    "SELECT name, state FROM tasks WHERE job=?", row["job"])}
            if any(states[row["job"]].get(d) != "done" for d in task.deps):
                continue
            for host in self.hosts():
                group = fits(task, host, usage[host.name], live_mem if host.kind == "local" else None)
                if group is None:
                    continue
                if self.launch(row, task, host, group):
                    usage[host.name].add(task, group)
                break

    def ensure_tools(self, host, transport):
        if host.name in self.tools:
            return
        for name in TOOLS:
            transport.put(self.ctx.lab_dir / name, f"{host.root}/tools/{name}")
        transport.put(self.ctx.run_study, f"{host.root}/tools/run-study.py")
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
                if row["attempts"] and any(local_out.iterdir()):
                    transport.put_dir(local_out, out)  # resume: earlier outputs, checkpoints first
            lab = f"{jobdir}/.lab/{task.name}"

            def subst(value):
                value = str(value).replace("{out}", out).replace("{job}", jobdir).replace("{lab}", tools)
                value = value.replace("{python}", launcher.python)
                return re.sub(r"\{input:([^}]+)\}", lambda m: inputs[m[1]], value)

            env = {}
            if host.kind == "local":
                env = {k: os.environ[k] for k in ("PATH", "HOME", "USER", "LANG", "CARGO_HOME", "RUSTUP_HOME")
                       if k in os.environ}
            env.update({k: subst(v) for k, v in task.env.items()})
            spec = dict(argv=[subst(a) for a in task.argv], cwd=subst(task.cwd) if task.cwd else out,
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
                              attempts=row["attempts"] + 1, live=json.dumps(dict(since=self.clock())))
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
            busy = self.store.one("SELECT COUNT(*) AS n FROM tasks WHERE job=? AND state IN ('pending', 'running')",
                                  row["id"])["n"]
            if busy:
                continue
            job = self.job(row["id"])
            summary = self.call(job, "finish")
            if isinstance(summary, Abort):
                summary = summary.reason
            summary = str(summary or "")
            if row["state"] == "aborting":
                final = json.loads(self.store.one("SELECT summary FROM jobs WHERE id=?", job.id)["summary"]
                                   or "{}").get("final", "aborted")
                reason = job.reason if final == "cancelled" else f"aborted: {job.reason}"
            else:
                final, reason = "done", "ok: " + (summary.splitlines()[0] if summary else "finished")
            table = "\n".join(f"{v.name}\t{v.state}\t{v.host or '-'}\t{v.rc if v.rc is not None else '-'}\t{v.reason or ''}"
                              for v in job.tasks.values())
            atomic_write(job.out / "summary.txt", f"{reason}\n\n{summary}\n\ntask\tstate\thost\trc\treason\n{table}\n")
            atomic_write(job.out / "DONE", reason + "\n")
            self.store.update("jobs", dict(id=job.id), state=final, finished=self.clock(), reason=reason,
                              summary=summary)
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
