"""Host inventory (config/lab/hosts.toml), capacity and placement.

Capacity model, per host:
  usable  = min(floor(cpus * max_cpu_fraction), cpus - reserve_cpus)   (threads)
  group g = floor(len(g) * usable / cpus)                              (threads)
A task that needs one L3 group is charged to that group and gets the group's CPUs as its
affinity (run-study.py semantics: the whole group, so the scheduler can balance inside it).
Other tasks float over the host and are charged to every group in proportion to its size,
as is load from processes labd does not own (loadavg minus labd's own threads, when
external_load is on). Memory is charged against max_mem_gb, and on the local host also
against live MemAvailable minus reserve_mem_gb.

Lanes: build and cargo-test tasks may use the host's build_overcommit_threads above usable,
and quick tasks (Task.quick) its quick_overcommit_threads. The two pools are separate: each
lane's overflow above usable is capped by its own pool (and by its own running threads), so a
build never takes a quick task's extra threads or the reverse. Normal tasks get neither and
still need every charged thread to fit inside usable. Memory and L3 limits apply to all.
"""

from dataclasses import dataclass, field
import math
import os
from pathlib import Path
import tomllib


@dataclass
class Host:
    name: str
    kind: str = "local"              # local | ssh | ec2
    arch: str = "x86_64"
    cpus: int = 1
    groups: list | None = None       # list of CPU-id lists; None = unknown (probe first)
    prefer_groups: list = field(default_factory=list)
    mem_gb: float = 8
    max_mem_gb: float = 8
    max_cpu_fraction: float = 0.9
    reserve_cpus: int = 1
    reserve_mem_gb: float = 0
    idle: bool = True
    external_load: bool = False
    launcher: str = "systemd"        # systemd | setsid | auto
    address: str | None = None
    root: str | None = None          # remote working root (tasks, artifacts, tools)
    python: str = "python3"
    ssh_options: list = field(default_factory=list)
    speed: float = 1.0
    enabled: bool = True
    up: bool = True
    external_threads: float = 0.0    # last measured load not owned by labd
    price_usd_h: float = 0.0
    build_overcommit_threads: int = 0
    quick_overcommit_threads: int = 0

    @property
    def usable(self):
        return max(0, min(math.floor(self.cpus * self.max_cpu_fraction), self.cpus - self.reserve_cpus))

    def group_cap(self, index):
        return math.floor(len(self.groups[index]) * self.usable / self.cpus)

    def pool(self, which):
        if which == "build":
            return max(0, self.build_overcommit_threads)
        if which == "quick":
            return max(0, self.quick_overcommit_threads)
        return 0

    def thread_limit(self, task):
        return self.usable + self.pool(lane(task))

    def threads_fit(self, task, usage, external=0):
        """Charge every running task and outside load; a lane may overflow usable only into its
        own pool, by at most its own running threads (outside load may fill an open pool)."""
        total = usage.threads + task.threads + external
        which = lane(task)
        if which is None:
            return total <= self.usable
        build = usage.build + (task.threads if which == "build" else 0)
        quick = usage.quick + (task.threads if which == "quick" else 0)
        build_pool = self.pool("build") if build else 0
        quick_pool = self.pool("quick") if quick else 0
        overflow = min(build_pool + quick_pool, min(build, build_pool) + min(quick, quick_pool) + external)
        return total <= self.usable + overflow

    def thread_label(self, used, build=None, quick=0):
        """used/usable, then threads above usable by pool. Without a split, overflow is build's."""
        build = used - quick if build is None else build
        extra = max(0, used - self.usable)
        build_extra = min(extra, build, self.pool("build")) if quick else extra
        quick_extra = min(extra - build_extra, quick)
        other = extra - build_extra - quick_extra
        label = f"{min(used, self.usable):g}/{self.usable}"
        for amount, name in ((build_extra, "build"), (quick_extra, "quick"), (other, "over")):
            if amount:
                label += f" +{amount:g} {name}"
        return label

    def order(self):
        indices = list(range(len(self.groups or [])))
        preferred = [g for g in self.prefer_groups if g in indices]
        return preferred + [g for g in indices if g not in preferred]


BUILD_KINDS = ("build", "cargo-test")


def lane(task):
    """'build', 'quick' or None (normal). Accepts a Task or its dict. Build kinds keep the
    build lane even when marked quick; quick's other rules (no reservation wait, local
    first, the time cap) still apply to them."""
    meta = task.get("meta", {}) if isinstance(task, dict) else task.meta
    quick = task.get("quick", False) if isinstance(task, dict) else task.quick
    if meta.get("kind") in BUILD_KINDS:
        return "build"
    return "quick" if quick else None


def prefers_local(task):
    """Try local hosts first: quick tasks, and tasks whose kind marked local inputs
    (meta.local_inputs: the command kind sets it when argv names files in the local checkout or
    home, or the spec gives a literal cwd). Everything else keeps the most-free-threads order."""
    return bool(task.quick or task.meta.get("local_inputs"))


def quick_timeout_h(task, limit_min):
    """The effective timeout of a quick task: its own timeout_h, capped at the quick limit."""
    limit = float(limit_min) / 60
    return limit if not task.timeout_h or task.timeout_h > limit else task.timeout_h


@dataclass
class Usage:
    threads: float = 0
    floating: float = 0
    mem: float = 0
    l3: dict = field(default_factory=dict)
    build: float = 0                 # threads of running build-lane tasks
    quick: float = 0                 # threads of running quick-lane tasks

    def add(self, task, group):
        self.threads += task.threads
        which = lane(task)
        if which:
            setattr(self, which, getattr(self, which) + task.threads)
        self.mem += task.mem_gb
        if group is None or group < 0:
            self.floating += task.threads
        else:
            self.l3[group] = self.l3.get(group, 0) + task.threads


def mem_available_gb():
    try:
        for line in Path("/proc/meminfo").read_text().splitlines():
            if line.startswith("MemAvailable:"):
                return int(line.split()[1]) / 1024**2
    except OSError:
        pass
    return math.inf


def allowed(task, host):
    """Static constraints only (arch, names, local_only)."""
    if not host.enabled:
        return False
    if task.arch and task.arch != host.arch:
        return False
    if task.local_only and host.kind != "local":
        return False
    names = task.hosts
    if names is None:
        return host.kind != "ec2"     # EC2 costs money: a job opts in by naming it
    return host.name in names or host.kind in names


def fits(task, host, usage, live_mem=None):
    """Return the L3 group index (-1 for floating) or None when the task must wait."""
    if not host.up or not allowed(task, host):
        return None
    if usage.mem + task.mem_gb > host.max_mem_gb:
        return None
    if live_mem is not None and task.mem_gb > live_mem - host.reserve_mem_gb:
        return None
    external = host.external_threads if host.external_load else 0
    # Charge every running task: each pool is shared across its lane on the host, not per task.
    if not host.threads_fit(task, usage, external):
        return None
    if not task.l3:
        return -1
    if not host.groups:
        return None
    for g in host.order():
        share = (usage.floating + external) * len(host.groups[g]) / host.cpus
        if usage.l3.get(g, 0) + task.threads + share <= host.group_cap(g):
            return g
    return None


def could_ever_fit(task, host):
    """Would the task fit on this host with nothing else running? (plan-time check)"""
    if not allowed(task, host) or task.mem_gb > host.max_mem_gb or task.threads > host.thread_limit(task):
        return False
    if task.l3 and host.groups is not None:
        return any(task.threads <= host.group_cap(g) for g in range(len(host.groups)))
    return True


def local_groups(run_study):
    import importlib.util
    spec = importlib.util.spec_from_file_location("lab_run_study", run_study)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return [list(g) for g in module.l3_groups(os.sched_getaffinity(0))]


class Inventory:
    def __init__(self, data, run_study=None):
        self.data = data
        self.lab = data.get("lab", {})
        self.build = data.get("build", {})
        self.ec2 = data.get("ec2", {})
        self.hosts = {}
        for name, item in data.get("hosts", {}).items():
            item = dict(item)
            groups = item.pop("groups", None)
            host = Host(name=name, **{k: v for k, v in item.items() if k in Host.__dataclass_fields__})
            if host.kind == "local" and run_study and groups in (None, "auto", "live"):
                host.groups = local_groups(run_study)
            elif isinstance(groups, list):
                host.groups = [list(range(g)) if isinstance(g, int) else list(g) for g in groups]
            if host.kind == "local" and host.groups and item.get("cpus") is None:
                host.cpus = sum(map(len, host.groups))
            if host.kind != "local":
                host.up = False       # until a probe says otherwise
            self.hosts[name] = host

    @classmethod
    def load(cls, path, run_study=None):
        with open(path, "rb") as stream:
            return cls(tomllib.load(stream), run_study)

    def get(self, key, default):
        return self.lab.get(key, default)

    def instance_type(self, name):
        return dict(self.ec2.get("instance_types", {}).get(name, {}))
