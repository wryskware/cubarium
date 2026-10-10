"""Host inventory (config/lab/hosts.toml), capacity and placement.

Capacity model, per host:
  usable  = min(floor(cpus * max_cpu_fraction), cpus - reserve_cpus)   (threads)
  group g = floor(len(g) * usable / cpus)                              (threads)
A task that needs one L3 group is charged to that group and gets the group's CPUs as its
affinity (run-study.py semantics: the whole group, so the scheduler can balance inside it).
Other tasks float over the host and are charged to every group in proportion to its size,
as is load from processes labd does not own (loadavg minus labd's own threads, when
external_load is on). Memory is charged against max_mem_gb, and on the local host also
against live MemAvailable minus reserve_mem_gb. Build and cargo-test tasks may use
the host's build_overcommit_threads above usable; memory and L3 limits still apply.
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

    @property
    def usable(self):
        return max(0, min(math.floor(self.cpus * self.max_cpu_fraction), self.cpus - self.reserve_cpus))

    def group_cap(self, index):
        return math.floor(len(self.groups[index]) * self.usable / self.cpus)

    def thread_limit(self, task):
        extra = self.build_overcommit_threads if task.meta.get("kind") in ("build", "cargo-test") else 0
        return self.usable + max(0, extra)

    def thread_label(self, used):
        extra = max(0, used - self.usable)
        return f"{min(used, self.usable):g}/{self.usable}" + (f" +{extra:g} build" if extra else "")

    def order(self):
        indices = list(range(len(self.groups or [])))
        preferred = [g for g in self.prefer_groups if g in indices]
        return preferred + [g for g in indices if g not in preferred]


@dataclass
class Usage:
    threads: float = 0
    floating: float = 0
    mem: float = 0
    l3: dict = field(default_factory=dict)

    def add(self, task, group):
        self.threads += task.threads
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
    # Charge every running task: the allowance is shared across the host, not per build.
    if usage.threads + task.threads + external > host.thread_limit(task):
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
