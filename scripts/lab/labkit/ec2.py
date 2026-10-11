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
`up` also gets --hours ceil(max_hours), so the host's own shutdown timer is a second guard.
"""

import json
import math
import os
import re
import secrets
import subprocess
import time
from types import SimpleNamespace

from .hosts import Host, allowed
from .core import Task
from . import providers
from .kinds.build import Build

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

    def type_info(self, name=None):
        name = name or (self.cfg.get("types") or ["c7g.16xlarge"])[0]
        info = self.d.inv.instance_type(name)
        return name, info

    def potential_hosts(self):
        if not self.cfg:
            return []
        hosts = []
        for name in self.cfg.get("types", []):
            info = self.d.inv.instance_type(name)
            hosts.append(Host(name="ec2", kind="ec2", arch=info.get("arch", "aarch64"), cpus=info.get("cpus", 64),
                              groups=None, mem_gb=info.get("mem_gb", 128),
                              max_mem_gb=min(info.get("mem_gb", 128), self.cfg.get("max_mem_gb", 64)),
                              max_cpu_fraction=self.cfg.get("max_cpu_fraction", 0.9),
                              reserve_cpus=self.cfg.get("reserve_cpus", 1), speed=info.get("speed", 1.0),
                              price_usd_h=info.get("spot_usd_h", 0.0)))
        return hosts

    def make_host(self, ip):
        name, info = self.type_info(self.s.get("type"))
        template = next((h for h in self.potential_hosts()), None)
        host = Host(name="ec2", kind="ec2", arch=info.get("arch", "aarch64"), cpus=info.get("cpus", 64),
                    groups=None, mem_gb=info.get("mem_gb", 128),
                    max_mem_gb=template.max_mem_gb if template else 64,
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
        return self.d.store.q("SELECT * FROM tasks WHERE state='running' AND host='ec2'")

    def pending_builds(self, tasks):
        """Include required builds before the provider jobs have been planned, once per key."""
        builds = {t.meta["key"]: t for t in tasks if t.meta.get("kind") == "build" and t.meta.get("key")}
        hosts = self.potential_hosts()[:1]
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
        name, info = self.type_info()
        local_h, _ = self.d.eta()
        hosts = self.potential_hosts()[:1]
        builds = self.pending_builds(tasks)
        simulations = [t for t in tasks if t.meta.get("kind") != "build"]
        ec2_h, unknown = estimate(simulations, [], hosts)
        # Builds precede sims, so add their time rather than treating both as parallel work.
        # Serializing distinct commits is conservative even if a large host overlaps builds.
        build_h = sum(t.est_h or 0 for t in builds) / max((h.speed for h in hosts), default=1.0)
        unknown += sum(t.est_h is None for t in builds)
        setup = float(self.cfg.get("setup_h", 0.25))
        ec2_total = None if ec2_h is None else ec2_h + build_h + setup
        estimate_info = dict(instance_type=name, price_usd_h=info.get("spot_usd_h"), local_eta_h=local_h,
                             ec2_eta_h=ec2_total, unknown_estimates=unknown,
                             est_usd=None if ec2_total is None else round(ec2_total * info.get("spot_usd_h", 0), 2),
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
        done = self.script("up", "-n", "1", "-t", self.s["type"], "--hours", str(math.ceil(self.s["max_hours"])))
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

    def cost(self):
        elapsed_h = (self.d.clock() - self.s.get("launched", self.d.clock())) / 3600
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
        for row in self.on_ec2():
            self.d.task_lost(row, f"ec2 down: {reason}")
        self.host = None
        self.d.events.emit("host-down", host="ec2", reason=reason, instance=self.s.get("instance"),
                           hours=round(hours, 3), est_usd=round(usd, 2))
        self.s = dict(state="idle", last=dict(reason=reason, hours=hours, usd=usd))
        self.save()
        return True

    def open_request(self, details):
        request_id = f"ec2-{secrets.token_hex(3)}"
        self.d.store.x("INSERT INTO approvals(id, kind, state, request, created) VALUES(?, 'ec2', 'open', ?, ?)",
                       request_id, json.dumps(details), self.d.clock())
        self.s.update(state="approval-needed", approval=request_id)
        self.d.events.emit("approval-needed", approval=request_id,
                           command=f"lab approve ec2 {request_id} --max-hours H --max-usd D", **details)
        self.save()

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
            self.d.events.emit("host-down", host="ec2", reason="instance gone while labd was down")
            for row in self.on_ec2():
                self.d.task_lost(row, "ec2 instance gone")
            self.s = dict(state="idle")
            self.save()
        elif self.host and self.s["state"] == "up":
            self.probe_host()
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
            elif self.d.clock() - self.s.get("launched", 0) > float(self.cfg.get("launch_timeout_s", 900)):
                self.down("instance never became ready")
        elif state == "up":
            if self.host and self.d._due("probe:ec2", self.d.inv.get("probe_s", 300)):
                self.probe_host()
            if not tasks and not self.on_ec2():
                self.down("queue drained")
                return
            if self.d._due("ec2-status", float(self.cfg.get("status_s", 60))):
                rows, error = self.status()
                if rows is not None and self.s.get("instance") not in rows:
                    self.d.events.emit("host-down", host="ec2", reason="spot loss", instance=self.s.get("instance"))
                    for row in self.on_ec2():
                        self.d.task_lost(row, "spot loss")
                    self.host = None
                    self.s = dict(state="idle")
                    self.save()
