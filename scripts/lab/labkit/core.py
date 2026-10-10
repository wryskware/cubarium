"""The generic job interface: Task, Kind, the kind registry and the shared Context.

A kind is one module defining a Kind subclass with a unique `name`. The core calls:

  validate(spec, ctx) -> spec        schema check, defaults filled, commits pinned (submit time)
  plan(job, ctx) -> [Task]           expand the job into tasks
  inputs(job, ctx) -> [key]          job-level artifact keys needed before preflight (optional)
  preflight(job, ctx) -> None|Abort  seconds-long, after inputs exist, before any task starts
  progress(job, task, ctx) -> None|Abort|Spawn
                                     on every task end (task set) and on a cadence (task None)
  finish(job, ctx) -> str            summary text; the core writes summary.txt and DONE

Nothing here is simulation-specific.
"""

from dataclasses import asdict, dataclass, field, fields
import importlib.util
import inspect
import json
import os
from pathlib import Path
import pkgutil
import platform
import re
import subprocess
import sys

ARCH = platform.machine()
SAFE = re.compile(r"[A-Za-z0-9][A-Za-z0-9_.-]*")


@dataclass
class Task:
    name: str
    argv: list
    threads: int = 1
    mem_gb: float = 1.0
    l3: bool = False                 # must run inside one shared-L3 group
    arch: str | None = None          # None: any; else x86_64 / aarch64
    hosts: list | None = None        # allowed host names (None: local hosts; "ec2" opts in)
    local_only: bool = False
    inputs: list = field(default_factory=list)      # artifact keys; argv may use {input:KEY}
    outputs: str | None = None       # dir; default <job>/<name>; argv may use {out}
    done_marker: str | None = None   # relative to outputs; required for success when set
    deps: list = field(default_factory=list)        # task names in the same job
    priority: int = 0
    cwd: str | None = None
    env: dict = field(default_factory=dict)
    stdout: str | None = None        # default {out}/stdout.log
    stderr: str | None = None        # default {out}/stderr.log
    hidden: bool = False             # wrap in scripts/hidden.sh (private Xvfb)
    liveness: dict | None = None     # {"stall_min": N, "paths": [globs]} or {"minute_file": ...}
    timeout_h: float | None = None
    est_h: float | None = None       # wall estimate for ETAs and EC2 planning
    resume_argv: list | None = None  # used when the task is retried after host loss
    retries: int = 0
    produces: dict = field(default_factory=dict)    # artifact key -> path relative to outputs
    rlimit_as_gb: float | None = None
    meta: dict = field(default_factory=dict)

    def __post_init__(self):
        if not SAFE.fullmatch(self.name):
            raise ValueError(f"task name {self.name!r} is not a safe identifier")
        if not self.argv or not all(isinstance(a, str) for a in self.argv):
            raise ValueError(f"task {self.name}: argv must be a nonempty list of strings")
        if self.threads < 1 or self.mem_gb < 0:
            raise ValueError(f"task {self.name}: threads >= 1 and mem_gb >= 0")

    def to_dict(self):
        return asdict(self)

    @classmethod
    def from_dict(cls, data):
        known = {f.name for f in fields(cls)}
        return cls(**{k: v for k, v in data.items() if k in known})


@dataclass
class Abort:
    reason: str


@dataclass
class Spawn:
    tasks: list


class SpecError(ValueError):
    pass


class Kind:
    name = None
    description = ""

    def validate(self, spec, ctx):
        return spec

    def plan(self, job, ctx):
        return []

    def inputs(self, job, ctx):
        return []

    def preflight(self, job, ctx):
        return None

    def progress(self, job, task, ctx):
        if task is not None and task.state not in ("done",):
            return Abort(f"task {task.name} {task.state}: {task.reason or 'exit ' + str(task.rc)}")
        return None

    def finish(self, job, ctx):
        counts = {}
        for task in job.tasks.values():
            counts[task.state] = counts.get(task.state, 0) + 1
        return "tasks: " + (", ".join(f"{n} {s}" for s, n in sorted(counts.items())) or "none")


_REGISTRY = {}


def register(cls):
    """Register a Kind subclass (decorator). Discovery calls this for every kind module."""
    if not (inspect.isclass(cls) and issubclass(cls, Kind) and cls.name):
        raise TypeError("register needs a Kind subclass with a name")
    _REGISTRY[cls.name] = cls
    return cls


def _scan(module):
    for value in vars(module).values():
        if inspect.isclass(value) and issubclass(value, Kind) and value is not Kind and value.name \
                and value.__module__ == module.__name__:
            register(value)


def load_kinds(extra_paths=()):
    """Built-in kinds (labkit/kinds/*.py) plus every *.py in extra plugin directories."""
    from . import kinds as package
    for info in pkgutil.iter_modules(package.__path__):
        _scan(importlib.import_module(f"{package.__name__}.{info.name}"))
    paths = list(extra_paths) + [p for p in os.environ.get("LAB_KIND_PATH", "").split(os.pathsep) if p]
    for directory in paths:
        for file in sorted(Path(directory).expanduser().glob("*.py")):
            spec = importlib.util.spec_from_file_location(f"lab_kind_{file.stem}", file)
            module = importlib.util.module_from_spec(spec)
            sys.modules[spec.name] = module
            spec.loader.exec_module(module)
            _scan(module)
    return {name: cls() for name, cls in _REGISTRY.items()}


class TaskView:
    def __init__(self, row, job_out):
        self.name = row["name"]
        self.task = Task.from_dict(json.loads(row["spec"]))
        self.state = row["state"]
        self.rc = row["rc"]
        self.reason = row["reason"]
        self.host = row["host"]
        self.started = row["started"]
        self.finished = row["finished"]
        self.attempts = row["attempts"]
        self.out = Path(self.task.outputs) if self.task.outputs else Path(job_out) / self.name


class Job:
    """What a kind sees: id, spec, out dir, kind-private persistent state, tasks."""

    def __init__(self, row, task_rows, inputs=None):
        self.id = row["id"]
        self.kind = row["kind"]
        self.name = row["name"]
        self.status = row["state"]
        self.spec = json.loads(row["spec"])
        self.state = json.loads(row["kstate"] or "{}")
        self.out = Path(row["out"])
        self.reason = row["reason"]
        self.tasks = {r["name"]: TaskView(r, self.out) for r in task_rows}
        self.inputs = inputs or {}


def git(tree, *args, timeout=20):
    return subprocess.run(["git", "-C", str(tree), *args], capture_output=True, text=True,
                          timeout=timeout, check=True, stdin=subprocess.DEVNULL).stdout.strip()


def resolve_commit(tree, ref=None):
    """Pin a tree to an exact commit. A dirty tree becomes a `git stash create` commit:
    that writes an object without touching the shared stash stack. A ref under
    refs/labd/snap keeps it from garbage collection. Untracked files are not included."""
    top = Path(git(tree, "rev-parse", "--show-toplevel")).resolve()
    if ref:
        return dict(tree=str(top), commit=git(top, "rev-parse", "--verify", f"{ref}^{{commit}}"), dirty=False)
    head = git(top, "rev-parse", "HEAD")
    if not git(top, "status", "--porcelain", "--untracked-files=no"):
        return dict(tree=str(top), commit=head, dirty=False)
    snapshot = git(top, "stash", "create", "labd snapshot")
    if not snapshot:
        return dict(tree=str(top), commit=head, dirty=False)
    git(top, "update-ref", f"refs/labd/snap/{snapshot}", snapshot)
    return dict(tree=str(top), commit=snapshot, dirty=True, base=head)


def binary_key(commit, arch, name):
    return f"binary:{commit}:{arch}:{name}"


class Context:
    """Shared services every kind gets. Thread-safe for preflight (store is locked)."""

    def __init__(self, paths, store=None, events=None, inventory=None):
        self.paths = paths
        self.repo = paths.repo
        self.store = store
        self.events = events
        self.inventory = inventory
        self.arch = ARCH
        self.python = sys.executable
        self.lab_dir = Path(__file__).resolve().parent
        self.run_study = self.repo / "scripts/run-study.py"

    # ---- paths and specs
    def path(self, spec, value):
        """Resolve a spec path: absolute stays, relative is from the submitter's cwd."""
        value = Path(str(value)).expanduser()
        return value if value.is_absolute() else Path(spec.get("_cwd") or self.repo) / value

    def require(self, spec, key, kind=None):
        if key not in spec:
            raise SpecError(f"missing required field {key!r}")
        if kind and not isinstance(spec[key], kind):
            raise SpecError(f"field {key!r} must be {kind}")
        return spec[key]

    # ---- trees, commits, artifacts
    def resolve_commit(self, tree, ref=None):
        return resolve_commit(tree, ref)

    def binary(self, tree, commit, name="voxel_census", arch=None, package=None, example=None,
               bin=None, features=None, dirty=False):
        """Key for a cached build of (commit, arch, name); records how to build it."""
        arch = arch or self.arch
        key = binary_key(commit, arch, name)
        recipe = dict(provider="binary", repo=str(tree), commit=commit, arch=arch, name=name,
                      package=package, example=example, bin=bin, features=features or [], dirty=dirty)
        if self.store:
            self.store.x("INSERT INTO recipes(key, spec) VALUES(?, ?) ON CONFLICT(key) DO NOTHING",
                         key, json.dumps(recipe))
        return key

    def artifact(self, key):
        if not self.store:
            return None
        row = self.store.one("SELECT path FROM artifacts WHERE key=?", key)
        return Path(row["path"]) if row and Path(row["path"]).exists() else None

    # ---- events and placement helpers
    def event(self, name, **fields):
        if self.events:
            return self.events.emit(name, **fields)

    def probe_prefix(self, slot=0):
        """Seconds-long in-daemon probes: run-study slot plus idle priority."""
        return [self.python, str(self.run_study), "--slot", str(slot), "--",
                "chrt", "-i", "0", "nice", "-n", "19"]

    def l3_groups(self):
        spec = importlib.util.spec_from_file_location("lab_run_study", self.run_study)
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        return module.l3_groups(os.sched_getaffinity(0))

    @staticmethod
    def output_signature(paths):
        """(file count, total bytes, newest mtime) over globs: liveness by output growth."""
        count = size = newest = 0
        for pattern in paths:
            for file in Path("/").glob(str(pattern).lstrip("/")) if str(pattern).startswith("/") else Path().glob(pattern):
                try:
                    stat = file.stat()
                except OSError:
                    continue
                count += 1
                size += stat.st_size
                newest = max(newest, stat.st_mtime)
        return [count, size, round(newest, 3)]
