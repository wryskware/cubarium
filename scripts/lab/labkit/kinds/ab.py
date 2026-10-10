"""ab: guarded main-versus-branch effect check, on scripts/studies/ab.py's own guards.

    kind = "ab"
    config_path = "config/desktop/terrarium-wide.toml"
    expect = ["fauna.founders.frondgrazer.core.reproduction.birth_interval_s"]
    expect_metric = ["founders_frondgrazer", "fauna_births"]
    seeds = "1-4"       hours = 2       threads = 1      early_minute = 20
    preflight_only = false              est_h = 1.0  # optional wall estimate per run
    [arms.main]                         # first arm is the baseline
    tree = "/home/wrysk/wryskware/cubarium"
    commit = "HEAD"                     # omitted / "working": the working tree (dirty: snapshot)
    [arms.branch]
    tree = ".claude/worktrees/x"
    bin = "runs/bins/voxel_census.8dd1f576"   # optional; needs commit; else built and cached
    commit = "8dd1f576"

Each arm is pinned to an exact commit at submit. Arms without `bin` get a cached build of
that commit (binary:<commit>:<arch>:voxel_census). Preflight reuses ab.py: configs only
from the arm's own tree at its commit, the resolved expand=/params= diff, --expect keys,
identical-arm refusal. The early stage runs every seed to early_minute; ab.py's early_read
is the identical-results tripwire; continuations load only their own checkpoints.
"""

import argparse
import contextlib
import io
from pathlib import Path
import subprocess
import sys
import threading

from ..core import Abort, Kind, Spawn, SpecError, Task

STUDIES = Path(__file__).resolve().parents[4] / "scripts/studies"
if str(STUDIES) not in sys.path:
    sys.path.insert(0, str(STUDIES))
import ab as abtool  # noqa: E402
from batch import seed_list  # noqa: E402
from .staging import pinned_config

RESERVED = {"build", "early", "continuation", "full", "DONE"}
PRINT_LOCK = threading.Lock()


@contextlib.contextmanager
def logged(path):
    """ab.py reports with print(); capture it into the job's log (daemon logs use __stdout__)."""
    buffer = io.StringIO()
    with PRINT_LOCK, contextlib.redirect_stdout(buffer):
        try:
            yield buffer
        finally:
            with open(path, "a") as log:
                log.write(buffer.getvalue())


def listed(value):
    if value is None:
        return []
    return abtool.names([value] if isinstance(value, str) else [str(v) for v in value])


class AB(Kind):
    name = "ab"
    description = "guarded main-versus-branch effect check (ab.py guards, staged, tripwire)"

    def validate(self, spec, ctx):
        arms = ctx.require(spec, "arms", dict)
        if len(arms) < 2:
            raise SpecError("ab needs at least two arms; the first is the baseline")
        for name, arm in arms.items():
            if not abtool.re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_-]*", name) or name in RESERVED:
                raise SpecError(f"arm name {name!r} is unsafe or reserved")
            if not isinstance(arm, dict) or "tree" not in arm:
                raise SpecError(f"arm {name} needs a tree")
            if arm.get("pinned"):
                continue
            tree = ctx.path(spec, arm["tree"])
            if arm.get("bin"):
                if not arm.get("commit") or arm["commit"] == "working":
                    raise SpecError(f"arm {name}: a supplied bin needs commit = the binary's source commit")
                arm["bin"] = str(ctx.path(spec, arm["bin"]).resolve(strict=True))
            commit = arm.get("commit")
            pinned = ctx.resolve_commit(tree, None if commit in (None, "working") else commit)
            arm.update(tree=pinned["tree"], commit=pinned["commit"], dirty=pinned["dirty"], pinned=True)
            if arm.get("config"):
                arm["config"] = str(ctx.path(spec, arm["config"]))
        defaults = dict(config_path="config/desktop/terrarium-wide.toml", seeds="1-8", hours=2, threads=1,
                        early_minute=20, early_read=True, segment_min=15, preflight_only=False, slot_base=0,
                        stall_min=30, mem_gb=3)
        for key, value in defaults.items():
            spec.setdefault(key, value)
        spec["expect"] = listed(spec.get("expect"))
        spec["expect_metric"] = listed(spec.get("expect_metric"))
        spec["seed_list"] = seed_list(str(spec["seeds"]))
        if spec["threads"] < 1 or spec["segment_min"] < 1 or spec["early_minute"] < 1 or spec["hours"] <= 0:
            raise SpecError("require positive threads, minutes and hours")
        if not spec["preflight_only"] and spec["early_read"] and spec["expect_metric"] \
                and spec["hours"] * 60 < spec["early_minute"]:
            raise SpecError("hours ends before early_minute")
        return spec

    def keys(self, job, ctx):
        return {name: ctx.binary(arm["tree"], arm["commit"], "voxel_census", dirty=arm.get("dirty", False))
                for name, arm in job.spec["arms"].items() if not arm.get("bin")}

    def inputs(self, job, ctx):
        return list(self.keys(job, ctx).values())

    def stage_tasks(self, job, ctx, stage, hours, binaries, starts=None):
        spec, tasks = job.spec, []
        for seed in spec["seed_list"]:
            for name in spec["arms"]:
                root = job.out / stage / name / f"s{seed}"
                arm = spec["arms"][name]
                relative = Path(arm["config"]).relative_to(arm["tree"]) if arm.get("config") else Path(spec["config_path"])
                config = job.out / name / "source" / relative
                files = {str(job.out / name): "{job}/" + name}
                if starts:
                    checkpoint = starts[name][str(seed)]
                    files[checkpoint] = "{job}/" + Path(checkpoint).relative_to(job.out).as_posix()
                    source = [f"load={checkpoint}"]
                else:
                    source = [f"config={config}", f"seed={seed}"]
                if arm.get("bin"):
                    files[arm["bin"]] = "{job}/inputs/" + name + "/binary"
                argv = abtool.census_argv(binaries[name], hours, source, root, spec["threads"], spec["segment_min"])
                tasks.append(Task(name=f"{stage}-{name}-s{seed}", argv=argv, threads=spec["threads"],
                                  mem_gb=spec["mem_gb"], l3=True, arch=ctx.arch, hosts=spec.get("hosts"), files=files,
                                  outputs=str(root),
                                  stdout=str(root / "census.csv"), stderr=str(root / "err.log"),
                                  inputs=[binaries[name][7:-1]] if binaries[name].startswith("{input:") else [],
                                  liveness=dict(minute_file="run/status", stall_min=spec["stall_min"]),
                                  est_h=spec.get("est_h"), meta=dict(stage=stage, arm=name, seed=seed)))
        return tasks

    def binaries(self, job, ctx, placeholders=True):
        keys = self.keys(job, ctx)
        result = {}
        for name, arm in job.spec["arms"].items():
            if arm.get("bin"):
                result[name] = arm["bin"]
            elif placeholders:
                result[name] = "{input:" + keys[name] + "}"
            else:
                result[name] = job.inputs.get(keys[name]) or str(ctx.artifact(keys[name]))
        return result

    def horizons(self, spec):
        early = spec["early_read"] and bool(spec["expect_metric"])
        return early, (spec["early_minute"] / 60 if early else spec["hours"])

    def plan(self, job, ctx):
        if job.spec["preflight_only"]:
            return []
        early, hours = self.horizons(job.spec)
        return self.stage_tasks(job, ctx, "early" if early else "full", hours, self.binaries(job, ctx))

    def preflight(self, job, ctx):
        spec = job.spec
        binaries = self.binaries(job, ctx, placeholders=False)
        arms = spec["arms"]
        args = argparse.Namespace(
            arm=[f"{n}={a['tree']}" for n, a in arms.items()], bin=[f"{n}={binaries[n]}" for n in arms],
            commit=[f"{n}={a['commit']}" for n, a in arms.items()],
            config=[f"{n}={a['config']}" for n, a in arms.items() if a.get("config")],
            config_path=spec["config_path"], preflight_only=True, pinned=True,
            dirty={n: a.get("dirty", False) for n, a in arms.items()})
        with logged(job.out / "preflight.log") as log:
            try:
                resolved = abtool.arm_inputs(args, job.out)
                for name, arm in resolved.items():
                    relative = Path(arm["config_path"]).relative_to(arm["tree"])
                    config = pinned_config(arm["tree"], arm["commit"], relative, job.out / name / "source")
                    arm.update(config=str(config), config_sha256=abtool.sha256(config))
                manifest = dict(arms=resolved, seeds=spec["seed_list"], expect=spec["expect"],
                                expect_metric=spec["expect_metric"], args={k: v for k, v in spec.items()
                                                                            if not k.startswith("_")}, lab_job=job.id)
                abtool.save_manifest(job.out, manifest)
                manifest["diff"] = abtool.preflight(job.out, resolved, spec["expect"], ctx.run_study,
                                                    spec["slot_base"])
                abtool.save_manifest(job.out, manifest)
            except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as error:
                return Abort(str(error))
        job.state["arms"] = resolved
        job.state["diff_keys"] = {n: len(d) for n, d in manifest["diff"].items()}
        job.state["preflight_log"] = log.getvalue()[-2000:]
        return None

    def progress(self, job, task, ctx):
        if task is None:
            return None
        stage = task.task.meta.get("stage")
        if task.state != "done":
            return Abort(f"{stage}: {task.name} {task.state} ({task.reason})")
        if any(v.state != "done" for v in job.tasks.values() if v.task.meta.get("stage") == stage):
            return None
        if stage != "early":
            return None
        spec = job.spec
        arms = job.state["arms"]
        with logged(job.out / "early.log"):
            try:
                different = abtool.early_read(job.out, job.out / "early", list(arms), spec["seed_list"],
                                              spec["expect_metric"], spec["early_minute"])
            except (OSError, ValueError) as error:
                return Abort(f"early read: {error}")
        if not different:
            return Abort("early-identical: every expected metric identical across all seeds")
        rest = spec["hours"] - spec["early_minute"] / 60
        if rest <= 0:
            return None
        starts = {}
        for view in job.tasks.values():
            if view.task.meta.get("stage") != "early":
                continue
            checkpoint = view.out / "run/ckpt" / f"m{spec['early_minute']:06d}"
            if not (checkpoint / "minute").is_file():
                return Abort(f"missing early checkpoint: {checkpoint}")
            starts.setdefault(view.task.meta["arm"], {})[str(view.task.meta["seed"])] = str(checkpoint)
        try:
            for arm in arms.values():
                abtool.unchanged(arm)
        except ValueError as error:
            return Abort(str(error))
        binaries = self.binaries(job, ctx)
        return Spawn(self.stage_tasks(job, ctx, "continuation", rest, binaries, starts))

    def finish(self, job, ctx):
        if job.spec["preflight_only"]:
            head = "preflight-only: passed; no study launched" if job.state.get("arms") else "preflight did not pass"
        else:
            stages = {}
            for view in job.tasks.values():
                stage = view.task.meta.get("stage")
                stages.setdefault(stage, {}).setdefault(view.state, 0)
                stages[stage][view.state] += 1
            head = "stages: " + "; ".join(f"{s} " + ", ".join(f"{n} {k}" for k, n in c.items()) for s, c in stages.items())
        diffs = ", ".join(f"{n}: {k} changed keys" for n, k in job.state.get("diff_keys", {}).items())
        return f"{head}\npreflight diff: {diffs or '-'}\n{job.state.get('preflight_log', '')}"
