"""EC2 offload: estimate, ask, launch only on an explicit approval, always tear down.

States (persisted in kv 'ec2'):
  idle             nothing up; no request open
  approval-needed  eligible tasks exist and EC2 is worth it; event approval-needed sent once
  launching        approved: `aws-graviton.sh up` returned an instance id; waiting for IP/ready
  up               host schedulable; limits enforced every tick
  down-failed      `down` failed (e.g. expired login): reported loudly, retried every tick
  blocked-auth     `up` failed on an expired AWS login: reported, never "fixed"; needs a new approve

One approval = one launch, with instance type, max hours and max dollars. The daemon calls
`down` when the queue drains (no eligible pending and nothing running there), when the hour
or dollar limit is reached, and on spot loss it requeues the lost tasks (they resume from
fetched checkpoints) and opens a new request: a replacement launch needs its own go.
Sustained low fill moves checkpointed tails to free local capacity using the same commit's
x86_64 binary, then the normal drain downs the host. Approval estimates include a greedy
fill profile, mean busy share and idle dollars; these are information, not approval gates.
`up` also gets --hours ceil(max_hours), so the host's own shutdown timer is a second guard.

S3 (optional: `s3_bucket`, `s3_region`, `instance_profile` in [ec2]): `up` attaches the
instance profile when one is named; at host-up labd checks on the instance whether its aws cli
can list the bucket and records that per instance (`ec2-s3` event). Only a capable instance
copies checkpoints to `ckpt/` at each remote probe, pulls them on resume, and uses `bin/`.
"""

import json
import math
import os
from pathlib import Path
import re
import secrets
import subprocess
import time
from types import SimpleNamespace

from .hosts import Host, allowed, could_ever_fit, fits, mem_available_gb
from .core import SpecError, Task
from .census_resume import latest_checkpoint
from . import budget as budget_mod
from . import providers
from .kinds.build import Build
from . import transfer

AUTH = re.compile(r"ExpiredToken|expired|Unable to locate credentials|InvalidClientTokenId|AuthFailure|"
                  r"sso|SignatureDoesNotMatch|could not be found in the credential", re.I)


def estimate(pending, running, hosts, speed_of=None):
    """Makespan estimate (hours): max(longest task, thread-hours / capacity). Tasks without
    est_h are counted as unknown. Returns (hours or None, unknown count)."""
    capacity = sum(h.usable * h.speed for h in hosts if h.up)
    unknown = sum(1 for t in pending if t.est_h is None) + sum(1 for t, _ in running if t.est_h is None)
    work, longest = 0.0, 0.0
    for task in pending:
        if task.est_h is not None:
            work += task.threads * task.est_h
            longest = max(longest, task.est_h)
    for task, elapsed in running:
        if task.est_h is not None:
            left = max(0.0, task.est_h - elapsed / 3600)
            work += task.threads * left
            longest = max(longest, left)
    if capacity <= 0:
        return (None if work else 0.0), unknown
    speed = max((h.speed for h in hosts), default=1.0)
    return max(longest / speed, work / capacity), unknown


def fill_profile(tasks, host, builds=(), setup_h=0):
    """Longest-first list schedule, charging usable threads and memory, plus serial builds.

    A missing duration makes the profile unknown; no invented work enters the integral.
    """
    if any(t.est_h is None for t in [*tasks, *builds]) or host.usable <= 0:
        return dict(fill_profile=None, fill_mean=None, idle_usd=None)
    segments = []

    def segment(threads, hours):
        if hours <= 0:
            return
        fraction = threads / host.usable
        if segments and math.isclose(segments[-1]["fraction"], fraction):
            segments[-1]["hours"] += hours
        else:
            segments.append(dict(fraction=fraction, hours=hours))

    segment(0, setup_h)
    for task in builds:
        segment(task.threads, task.est_h / host.speed)
    pending = sorted((t for t in tasks if t.est_h > 0), key=lambda t: -t.est_h)
    active, now = [], 0.0
    while pending or active:
        threads = sum(t.threads for _, t in active)
        memory = sum(t.mem_gb for _, t in active)
        for task in pending[:]:
            if threads + task.threads <= host.usable and memory + task.mem_gb <= host.max_mem_gb:
                active.append((now + task.est_h / host.speed, task))
                pending.remove(task)
                threads += task.threads
                memory += task.mem_gb
        if not active:
            return dict(fill_profile=None, fill_mean=None, idle_usd=None)
        end = min(end for end, _ in active)
        segment(threads, end - now)
        active = [(finish, task) for finish, task in active if finish > end]
        now = end
    paid = sum(s["hours"] for s in segments)
    busy = sum(s["fraction"] * s["hours"] for s in segments)
    return dict(fill_profile=segments, fill_mean=busy / paid if paid else 0,
                idle_usd=(paid - busy) * host.price_usd_h, schedule_h=paid)


def fill_label(details):
    segments = details.get("fill_profile")
    if segments is None:
        return "fill unknown; idle $?; mean ?"
    if len(segments) > 5:
        middle = segments[2:-2]
        hours = sum(s["hours"] for s in middle)
        segments = [*segments[:2], dict(fraction=sum(s["fraction"] * s["hours"] for s in middle) / hours,
                                       hours=hours), *segments[-2:]]
    profile = " | ".join(f"{s['fraction']:.0%} {s['hours']:.2g} h" for s in segments)
    return f"fill {profile}; idle ${details['idle_usd']:.2f}; mean {details['fill_mean']:.0%}"


def tail_progress(task):
    """Work fractions left and replayed, using absolute minutes relative to the stage start."""
    root = Path(task.outputs)
    if task.meta.get("batch"):
        manifest = json.loads((root / "batch/batch.json").read_text())
        pieces = [(root / "batch" / r["arm"] / f"s{r['seed']}",
                   float(manifest.get("load_minutes", {}).get(str(r["seed"]), 0)),
                   float(manifest["args"]["hours"]) * 60) for r in manifest["runs"]]
    else:
        pieces = [(root, task.meta["start_minute"], task.meta["total_minutes"])]
    done, saved, total, minutes = 0.0, 0.0, 0.0, []
    for child, start, work in pieces:
        if not all(math.isfinite(v) for v in (start, work)) or work <= 0:
            return None
        total += work
        run = child / "run"
        if (run / "DONE").exists():
            done += work
            saved += work
            continue
        if not (run / "status").exists() and task.meta.get("batch"):
            continue  # not-yet-started batch child
        fields = dict(token.split("=", 1) for token in (run / "status").read_text().split() if "=" in token)
        current = float(fields["minute"])
        checkpoint, _ = latest_checkpoint(run)
        if checkpoint is None or not (run / "args").is_file():
            return None
        if not all(math.isfinite(v) for v in (start, work, current, checkpoint)) or work <= 0:
            return None
        done += min(work, max(0, current - start))
        saved += min(work, max(0, checkpoint - start))
        minutes.append(checkpoint)
    if total <= 0 or not minutes:
        return None
    return (total - done) / total, max(0, done - saved) / total, min(minutes)


class Ec2Pool:
    def __init__(self, daemon):
        self.d = daemon
        self.cfg = daemon.inv.ec2
        self.s = daemon.store.kv_get("ec2", dict(state="idle"))
        self.host = None
        if self.s.get("state") == "up" and self.s.get("ip"):
            self.host = self.make_host(self.s["ip"])

    # ---------------------------------------------------------------- helpers
    def save(self):
        self.d.store.kv_set("ec2", self.s)

    def budget(self):
        """Granted, spent, committed and remaining EC2 dollars (budget.summary)."""
        return budget_mod.summary(self.d.store, self.s, lambda t: self.type_info(t)[1].get("spot_usd_h"),
                                  self.d.clock(), self.d.paths.events)

    def ended(self, reason, hours, usd):
        """Record a terminated instance's spend (the budget's source of truth)."""
        budget_mod.record_spend(self.d.store, self.s.get("instance"), self.s.get("approval"), self.d.clock(),
                                hours, usd, reason)

    def type_info(self, name=None):
        name = name or (self.cfg.get("types") or ["c7g.16xlarge"])[0]
        info = self.d.inv.instance_type(name)
        return name, info

    def mem_limit(self, info):
        """A dedicated instance may use its memory less `reserve_mem_gb`; `max_mem_gb`, when set,
        is an extra ceiling (the desktop's 64 GB limit halved what a 128 GB host could run)."""
        mem = info.get("mem_gb", 128)
        limit = mem - float(self.cfg.get("reserve_mem_gb", 0))
        if "max_mem_gb" in self.cfg:
            limit = min(limit, self.cfg["max_mem_gb"])
        return max(0, limit)

    def potential_hosts(self):
        if not self.cfg:
            return []
        hosts = []
        for name in self.cfg.get("types", []):
            info = self.d.inv.instance_type(name)
            hosts.append(Host(name="ec2", kind="ec2", arch=info.get("arch", "aarch64"), cpus=info.get("cpus", 64),
                              groups=None, mem_gb=info.get("mem_gb", 128),
                              max_mem_gb=self.mem_limit(info),
                              max_cpu_fraction=self.cfg.get("max_cpu_fraction", 0.9),
                              reserve_cpus=self.cfg.get("reserve_cpus", 1), speed=info.get("speed", 1.0),
                              price_usd_h=info.get("spot_usd_h", 0.0)))
        return hosts

    def s3(self, host_name):
        """(bucket, region) when host_name is this instance and it can list the bucket; else None:
        without a bucket configured or without access, every S3 path is off."""
        bucket = self.cfg.get("s3_bucket")
        if host_name != "ec2" or not bucket or not self.s.get("s3"):
            return None
        return bucket, self.cfg.get("s3_region", "us-east-1")

    def probe_s3(self):
        """At host-up: can this instance's aws cli (its instance role) list the bucket?"""
        bucket = self.cfg.get("s3_bucket")
        if not bucket:
            self.s.pop("s3", None)
            return
        region = self.cfg.get("s3_region", "us-east-1")
        try:
            done = self.d.transports["ec2"].run(transfer.s3_probe_script(bucket, region), timeout=60)
            capable, detail = done.returncode == 0 and "S3-OK" in done.stdout, (done.stdout + done.stderr).strip()
        except (OSError, subprocess.SubprocessError) as error:
            capable, detail = False, str(error)
        self.s["s3"] = capable
        self.save()
        self.d.events.emit("ec2-s3", instance=self.s.get("instance"), bucket=bucket, capable=capable,
                           detail=None if capable else detail[-300:])

    def make_host(self, ip):
        name, info = self.type_info(self.s.get("type"))
        host = Host(name="ec2", kind="ec2", arch=info.get("arch", "aarch64"), cpus=info.get("cpus", 64),
                    groups=None, mem_gb=info.get("mem_gb", 128),
                    max_mem_gb=self.mem_limit(info),
                    max_cpu_fraction=self.cfg.get("max_cpu_fraction", 0.9),
                    reserve_cpus=self.cfg.get("reserve_cpus", 1), idle=False,
                    launcher=self.cfg.get("launcher", "setsid"), root=self.cfg.get("root"),
                    python=self.cfg.get("python", "python3.11"), speed=info.get("speed", 1.0),
                    price_usd_h=info.get("spot_usd_h", 0.0),
                    address=None if self.cfg.get("transport") == "local" else f"{self.cfg.get('user', 'ec2-user')}@{ip}",
                    ssh_options=["-i", os.path.expanduser(self.cfg.get("ssh_key", "~/.ssh/cubarium-aws.pem")),
                                 "-o", "StrictHostKeyChecking=accept-new"])
        self.d._connect(host)
        return host

    def script(self, *args, timeout=300):
        path = self.d.ctx.path({}, self.cfg.get("script", "scripts/aws-graviton.sh"))
        done = subprocess.run([str(path), *args], capture_output=True, text=True, timeout=timeout,
                              cwd=self.d.paths.repo, stdin=subprocess.DEVNULL)
        return done

    def probe_host(self):
        """EC2 is outside the static inventory, so probe its execution-host L3 groups here."""
        try:
            info = self.d.probe(self.host)
        except (OSError, subprocess.SubprocessError) as error:
            info = dict(up=False, error=str(error))
        self.d.apply_probe(self.host, info)
        return self.host.up

    def eligible(self):
        """Pending tasks of live jobs that may run on EC2 (the job named it, or only EC2 fits)."""
        types = self.potential_hosts()
        result = []
        for row in self.d.store.q("SELECT t.spec FROM tasks t JOIN jobs j ON t.job=j.id WHERE t.state='pending' "
                                  "AND j.state IN ('running', 'inputs', 'preflight')"):
            task = Task.from_dict(json.loads(row["spec"]))
            if any(allowed(task, h) for h in types):
                result.append(task)
        return result

    def on_ec2(self):
        return self.d.store.q("SELECT * FROM tasks WHERE host='ec2' AND (state='running' OR "
                              "(state='pending' AND reason LIKE 'start uncertain:%'))")

    def pending_builds(self, tasks):
        """Include required builds before the provider jobs have been planned, once per key."""
        builds = {t.meta["key"]: t for t in tasks if t.meta.get("kind") == "build" and t.meta.get("key")}
        hosts = self.potential_hosts()
        for key in {key for task in tasks for key in task.inputs if key.startswith("binary:")}:
            if key in builds or self.d.ctx.artifact(key):
                continue
            row = self.d.store.one("SELECT spec FROM recipes WHERE key=?", key)
            if not row:
                continue
            spec = providers.binary(key, json.loads(row["spec"]), self.d.ctx)
            spec["pinned"] = True
            kind = Build()
            spec = kind.validate(spec, self.d.ctx)
            # This is the same recipe the provider will execute; planning does no build.
            for task in kind.plan(SimpleNamespace(spec=spec, state={}), self.d.ctx):
                if any(allowed(task, host) for host in hosts):
                    builds[key] = task
        return list(builds.values())

    def worth(self, tasks):
        local = [h for h in self.d.inv.hosts.values() if h.kind != "ec2"]
        only_ec2 = [t for t in tasks if not any(allowed(t, h) for h in local)]
        local_h, _ = self.d.eta()
        builds = self.pending_builds(tasks)
        simulations = [t for t in tasks if t.meta.get("kind") != "build"]
        setup = float(self.cfg.get("setup_h", 0.25))
        memory = sum(t.mem_gb for t in simulations)
        options = []
        for name, host in zip(self.cfg.get("types", []), self.potential_hosts()):
            if not all(could_ever_fit(t, host) for t in [*tasks, *builds]):
                continue
            # Memory limits concurrency as well as individual task placement. Scale only
            # thread capacity: a task's own wall time still uses the type's actual speed.
            fraction = min(1.0, host.max_mem_gb / memory) if memory else 1.0
            effective = SimpleNamespace(usable=host.usable * fraction, speed=host.speed, up=host.up)
            ec2_h, unknown = estimate(simulations, [], [effective])
            # Builds precede sims; distinct commits are conservatively serialized.
            build_h = sum(t.est_h or 0 for t in builds) / host.speed
            unknown += sum(t.est_h is None for t in builds)
            total = None if ec2_h is None else ec2_h + build_h + setup
            profile = fill_profile(simulations, host, builds, setup)
            total = profile.pop("schedule_h", total)
            options.append(dict(type=name, eta_h=total,
                                est_usd=None if total is None else total * host.price_usd_h,
                                cpus=host.cpus, unknown=unknown, **profile))
        if not options:
            return False, {}
        costed = [o for o in options if o["est_usd"] is not None]
        if costed:
            limit = min(o["est_usd"] for o in costed) * (1 + float(self.cfg.get("cost_tolerance", 0.10)))
            choice = min((o for o in costed if o["est_usd"] <= limit), key=lambda o: (o["eta_h"], o["cpus"]))
        else:  # no capacity estimate at all: the smallest type that fits
            choice = options[0]
        name, info = self.type_info(choice["type"])
        ec2_total, unknown = choice["eta_h"], choice["unknown"]
        estimate_info = dict(instance_type=name, price_usd_h=info.get("spot_usd_h"), local_eta_h=local_h,
                             ec2_eta_h=ec2_total, unknown_estimates=unknown,
                             est_usd=None if choice["est_usd"] is None else round(choice["est_usd"], 2), fallback=info.get("fallback"),
                             options=[dict(type=o["type"], eta_h=o["eta_h"],
                                           est_usd=None if o["est_usd"] is None else round(o["est_usd"], 2),
                                           **{k: o[k] for k in ("fill_profile", "fill_mean", "idle_usd")})
                                      for o in options],
                             **{k: choice[k] for k in ("fill_profile", "fill_mean", "idle_usd")},
                             only_ec2_tasks=len(only_ec2), eligible_tasks=len(tasks))
        if only_ec2:
            return True, estimate_info
        if local_h is not None and ec2_total is not None and local_h > float(self.cfg.get("suggest_after_h", 4)) \
                and ec2_total < local_h:
            return True, estimate_info
        return False, estimate_info

    # ---------------------------------------------------------------- transitions
    def approve(self, request_id, max_hours, max_usd, instance_type=None):
        if not (math.isfinite(max_hours) and max_hours > 0 and math.isfinite(max_usd) and max_usd > 0):
            raise ValueError("approval needs positive --max-hours and --max-usd")
        request = self.d.store.one("SELECT * FROM approvals WHERE id=?", request_id)
        if request is None:  # accept a job id named in the open request
            for row in self.d.store.q("SELECT * FROM approvals WHERE state='open'"):
                if request_id in json.loads(row["request"]).get("jobs", []):
                    request = row
        if request is None or request["state"] != "open":
            raise ValueError(f"no open EC2 approval request {request_id}")
        try:  # before anything launches; a running instance's cap is already committed
            budget_mod.check(self.budget(), max_usd)
        except budget_mod.BudgetError as error:
            self.d.events.emit("approval-refused", approval=request["id"], max_usd=max_usd, reason=str(error))
            raise
        details = json.loads(request["request"])
        instance_type = instance_type or details["instance_type"]
        grant = dict(instance_type=instance_type, max_hours=max_hours, max_usd=max_usd)
        self.d.store.update("approvals", dict(id=request["id"]), state="granted", grant_=json.dumps(grant),
                            decided=self.d.clock())
        self.d.events.emit("approval-granted", approval=request["id"], **grant)
        self.s.update(state="approved", approval=request["id"], type=instance_type, max_hours=max_hours,
                      max_usd=max_usd)
        self.launch()

    def launch(self):
        name, info = self.type_info(self.s["type"])
        types = ",".join(filter(None, [name, info.get("fallback")]))
        profile = ["--iam-instance-profile", self.cfg["instance_profile"]] if self.cfg.get("instance_profile") else []
        done = self.script("up", "-n", "1", "-t", types, "--hours", str(math.ceil(self.s["max_hours"])), *profile)
        text = done.stdout + done.stderr
        if done.returncode:
            if AUTH.search(text):
                self.s.update(state="blocked-auth")
                self.d.events.emit("ec2-auth-expired", approval=self.s.get("approval"),
                                   reason="AWS login expired or missing: renew it yourself (e.g. `aws sso login`); "
                                          "labd never touches keys or credentials. Then approve again.",
                                   detail=text.strip()[-300:])
            else:
                self.s.update(state="idle")
                self.d.events.emit("ec2-error", approval=self.s.get("approval"), reason=text.strip()[-300:])
            self.save()
            return
        match = re.search(r"\bi-[0-9a-f]+\b", done.stdout)
        launched_type = re.search(r"\blaunched\s+([^\s;]+);", done.stderr)
        if launched_type:
            self.s["type"] = launched_type[1]
        self.s["price_usd_h"] = float(self.type_info(self.s["type"])[1].get("spot_usd_h", 0))
        self.forget_host()
        self.s.pop("s3", None)  # a new instance proves its own S3 access at host-up
        self.s.update(state="launching", instance=match[0] if match else None, launched=self.d.clock(), ip=None)
        self.d.events.emit("ec2-launching", approval=self.s.get("approval"), instance=self.s["instance"],
                           instance_type=self.s["type"])
        self.save()

    def status(self):
        """(rows by instance id, error text or None)."""
        done = self.script("status", timeout=60)
        if done.returncode:
            return None, (done.stdout + done.stderr).strip()
        rows = {}
        for line in done.stdout.splitlines():
            parts = line.split()
            if len(parts) >= 4 and parts[1].startswith("i-"):
                rows[parts[1]] = dict(ip=parts[2], state=parts[3])
        return rows, None

    def ready(self):
        marker = self.cfg.get("ready_file", "/var/tmp/ready")
        if not marker:
            return True
        done = self.d.transports["ec2"].run(f"test -f {marker} && echo ready\n", timeout=20)
        return "ready" in done.stdout

    def cost(self, cap_h=None):
        elapsed_h = (self.d.clock() - self.s.get("launched", self.d.clock())) / 3600
        if cap_h is not None:
            elapsed_h = min(elapsed_h, cap_h)
        _, info = self.type_info(self.s.get("type"))
        return elapsed_h, elapsed_h * float(info.get("spot_usd_h", 0))

    def down(self, reason):
        # Save what the host has before terminating it (best effort; spot loss can't).
        for row in self.on_ec2():
            try:
                job = self.d.job(row["job"])
                view = job.tasks[row["name"]]
                self.d.transports["ec2"].get(self.d.remote_out(self.host, job, view), view.out, timeout=240)
            except Exception as error:  # noqa: BLE001
                self.d.events.emit("ec2-error", task=row["name"], job=row["job"], reason=f"fetch before down: {error}")
        done = self.script("down", timeout=300)
        hours, usd = self.cost()
        if done.returncode:
            auth = bool(AUTH.search(done.stdout + done.stderr))
            self.s.update(state="down-failed", down_reason=reason)
            self.d.events.emit("ec2-down-failed", reason=reason, auth_expired=auth,
                               detail=(done.stdout + done.stderr).strip()[-300:],
                               action="instance may still be up: renew the AWS login and run scripts/aws-graviton.sh down")
            self.save()
            return False
        self.d.drop_stages(host="ec2", reason=f"ec2 down: {reason}")
        for row in self.on_ec2():
            self.d.task_lost(row, f"ec2 down: {reason}")
        self.forget_host()
        self.host = None
        self.ended(reason, hours, usd)
        self.d.events.emit("host-down", host="ec2", reason=reason, instance=self.s.get("instance"),
                           hours=round(hours, 3), est_usd=round(usd, 2))
        self.s = dict(state="idle", last=dict(reason=reason, hours=hours, usd=usd))
        self.save()
        return True

    def forget_host(self):
        """A new instance has none of the last one's uploads: drop the persistent upload marks
        and the tools cache for `ec2` (launch 2 of 2026-10-10 found no source archive)."""
        self.d.store.x("DELETE FROM kv WHERE key LIKE 'uploaded:ec2:%'")
        self.d.tools.discard("ec2")

    def open_request(self, details):
        request_id = f"ec2-{secrets.token_hex(3)}"
        self.d.store.x("INSERT INTO approvals(id, kind, state, request, created) VALUES(?, 'ec2', 'open', ?, ?)",
                       request_id, json.dumps(details), self.d.clock())
        self.s.update(state="approval-needed", approval=request_id)
        self.d.events.emit("approval-needed", approval=request_id,
                           command=f"lab approve ec2 {request_id} --max-hours H --max-usd D",
                           budget_left_usd=round(self.budget()["remaining"], 2), **details)
        self.save()

    def move_tail(self, pending):
        rows = self.on_ec2()
        charged = sum(Task.from_dict(json.loads(r["spec"])).threads for r in rows)
        if (pending or not rows or not self.host or not self.host.up
                or charged > float(self.cfg.get("tail_fraction", 0.25)) * self.host.usable):
            if self.s.pop("tail_since", None) is not None:
                self.save()
            return
        now = self.d.clock()
        if "tail_since" not in self.s:
            self.s["tail_since"] = now
            self.save()
        if now - self.s["tail_since"] < float(self.cfg.get("tail_hold_min", 10)) * 60:
            return
        # Progress reads the mirror the background fetches keep; the stop below fetches once more.
        if not self.d._due("ec2-tail-check", float(self.cfg.get("tail_check_s", 120))):
            return
        self.d.local_load()
        usage, live_mem = self.d.usage(), mem_available_gb()
        for row in rows:
            row = self.d.store.one("SELECT t.* FROM tasks t JOIN jobs j ON j.id=t.job "
                                   "WHERE t.job=? AND t.name=? AND t.state='running' AND j.state='running'",
                                   row["job"], row["name"])
            if row is None:
                continue
            task = Task.from_dict(json.loads(row["spec"]))
            if not task.resume_argv or not task.est_h or task.est_h <= 0:
                continue
            try:
                progress = tail_progress(task)
            except (OSError, ValueError, KeyError, TypeError, subprocess.SubprocessError) as error:
                self.d.log(f"tail progress {row['job']}/{row['name']}: {error}")
                continue
            if progress is None:
                continue
            left, replay, minute = progress
            saved_h = (left - replay) * task.est_h / self.host.speed
            if saved_h <= float(self.cfg.get("tail_min_remaining_h", 0.5)):
                continue
            home = self.d.home_task(task)
            if home is None:
                continue
            candidates = sorted((h for h in self.d.hosts() if h.kind != "ec2"),
                                key=lambda h: h.usable - usage[h.name].threads -
                                (h.external_threads if h.external_load else 0), reverse=True)
            for host in candidates:
                group = fits(home, host, usage[host.name], live_mem if host.kind == "local" else None)
                if group is None:
                    continue
                try:
                    ready = [self.d.ensure_input(k, row["job"]) for k in home.inputs]
                except SpecError as error:
                    self.d.log(f"tail input {row['job']}/{row['name']}: {error}")
                    break
                if not all(ready):
                    break  # build first; keep computing on EC2 until the binary is cached
                before = self.tail_eta()
                home.hosts = [host.name]
                if not self.d.task_lost(row, "EC2 tail move", replacement=home):
                    break
                # Stopping/fetching may have picked up a newer committed checkpoint.
                latest = tail_progress(home)
                if latest:
                    minute = latest[2]
                    saved_h = max(0, latest[0] - latest[1]) * task.est_h / self.host.speed
                moved = self.d.store.one("SELECT * FROM tasks WHERE job=? AND name=?", row["job"], row["name"])
                if self.d.launch(moved, home, host, group):
                    usage[host.name].add(home, group)
                    if host.kind == "local":
                        live_mem -= home.mem_gb
                    after = self.tail_eta()
                    # Marginal drain time avoids counting the same instance savings twice.
                    saved_usd = (max(0, before - after) * self.host.price_usd_h
                                 if before is not None and after is not None else 0)
                    self.d.events.emit("tail-moved", job=row["job"], task=row["name"], host=host.name,
                                       from_minute=minute, est_h_saved=saved_h, usd_saved=saved_usd)
                    summary = self.d.store.kv_get("ec2-tail", dict(moved=0, est_h_saved=0, usd_saved=0))
                    summary.update(moved=summary["moved"] + 1,
                                   est_h_saved=summary["est_h_saved"] + saved_h,
                                   usd_saved=summary["usd_saved"] + saved_usd)
                    self.d.store.kv_set("ec2-tail", summary)
                break

    def tail_eta(self):
        """Estimated instance drain time; unknown progress cannot justify dollar savings."""
        remaining = []
        for row in self.on_ec2():
            task = Task.from_dict(json.loads(row["spec"]))
            if task.est_h is None:
                return None
            try:
                progress = tail_progress(task)
            except (OSError, ValueError, KeyError, TypeError):
                return None
            if progress is None:
                return None
            remaining.append(progress[0] * task.est_h / self.host.speed)
        return max(remaining, default=0)

    def reconcile(self):
        """At start: a launched instance labd knows about is resumed; unknown tagged hosts
        are reported, not killed (they may be someone's manual session)."""
        if not self.cfg or self.s.get("state") not in ("launching", "up", "down-failed"):
            return
        rows, error = self.status()
        if rows is None:
            self.d.events.emit("ec2-error", reason=f"status at start failed: {error}")
            return
        if self.s.get("instance") not in rows and self.s["state"] != "down-failed":
            # When it went is unknown: charge up to now, at most its own shutdown timer.
            hours, usd = self.cost(cap_h=math.ceil(self.s.get("max_hours") or 0) or None)
            self.ended("instance gone while labd was down", hours, usd)
            self.d.events.emit("host-down", host="ec2", reason="instance gone while labd was down",
                               instance=self.s.get("instance"), hours=round(hours, 3), est_usd=round(usd, 2))
            self.d.drop_stages(host="ec2", reason="ec2 instance gone")
            for row in self.on_ec2():
                self.d.task_lost(row, "ec2 instance gone")
            self.s = dict(state="idle")
            self.save()
        elif self.host and self.s["state"] == "up":
            if self.probe_host() and "s3" not in self.s:
                self.probe_s3()
        for instance in rows:
            if instance != self.s.get("instance"):
                self.d.events.emit("ec2-orphan", instance=instance, reason="tagged host labd did not launch")

    def tick(self):
        if not self.cfg:
            return
        state = self.s.get("state", "idle")
        if state == "down-failed":
            if self.d._due("ec2-down-retry", 60):
                self.down(self.s.get("down_reason", "retry"))
            return
        if state in ("launching", "up"):
            hours, usd = self.cost()
            if hours >= self.s["max_hours"] or usd >= self.s["max_usd"]:
                self.d.events.emit("ec2-limit", hours=round(hours, 3), est_usd=round(usd, 2),
                                   max_hours=self.s["max_hours"], max_usd=self.s["max_usd"])
                self.down("limit reached")
                return
        tasks = self.eligible()
        if state == "idle":
            if tasks:
                worth, details = self.worth(tasks)
                if worth:
                    details["jobs"] = sorted({r["job"] for r in self.d.store.q(
                        "SELECT DISTINCT t.job FROM tasks t JOIN jobs j ON t.job=j.id WHERE t.state='pending' "
                        "AND j.state IN ('running','inputs','preflight')")})
                    self.open_request(details)
        elif state == "approval-needed":
            if not tasks:
                self.d.store.update("approvals", dict(id=self.s["approval"]), state="withdrawn", decided=self.d.clock())
                self.d.events.emit("approval-withdrawn", approval=self.s["approval"], reason="no eligible tasks left")
                self.s = dict(state="idle")
                self.save()
        elif state == "launching":
            if not self.d._due("ec2-status", float(self.cfg.get("status_s", 20))):
                return
            rows, error = self.status()
            row = (rows or {}).get(self.s.get("instance"))
            if row and row["state"] == "running" and row["ip"] not in ("None", ""):
                self.s["ip"] = row["ip"]
                self.host = self.make_host(row["ip"])
                if not self.ready() or not self.probe_host():
                    self.host = None
                    return
                self.s["state"] = "up"
                self.save()
                self.d.events.emit("host-up", host="ec2", instance=self.s.get("instance"), ip=row["ip"])
                self.probe_s3()
            elif self.d.clock() - self.s.get("launched", 0) > float(self.cfg.get("launch_timeout_s", 900)):
                self.down("instance never became ready")
        elif state == "up":
            if self.host and self.d._due("probe:ec2", self.d.inv.get("probe_s", 300)):
                self.probe_host()
            self.move_tail(tasks)
            tasks = self.eligible()
            if not tasks and not self.on_ec2():
                self.down("queue drained")
                return
            if self.d._due("ec2-status", float(self.cfg.get("status_s", 60))):
                rows, error = self.status()
                if rows is not None and self.s.get("instance") not in rows:
                    hours, usd = self.cost()
                    self.ended("spot loss", hours, usd)
                    self.d.events.emit("host-down", host="ec2", reason="spot loss", instance=self.s.get("instance"),
                                       hours=round(hours, 3), est_usd=round(usd, 2))
                    self.d.drop_stages(host="ec2", reason="spot loss")
                    for row in self.on_ec2():
                        self.d.task_lost(row, "spot loss")
                    self.host = None
                    self.s = dict(state="idle")
                    self.save()
