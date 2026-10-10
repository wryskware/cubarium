#!/usr/bin/env python3
r"""Guarded main-versus-branch effect checks (fresh starts, stdlib only).

    python3 scripts/studies/ab.py OUT --arm main=/repo --arm branch=/worktree \
      --config-path config/desktop/terrarium-wide.toml \
      --bin main=/bins/voxel_census.MAIN --commit main=MAIN \
      --bin branch=/bins/voxel_census.BRANCH --commit branch=BRANCH \
      --expect fauna.founders.frondgrazer.core.reproduction.birth_interval_s \
      --expect-metric founders_frondgrazer,fauna_births --seeds 1-4 \
      --hours 2 --slot-base 8 --max-parallel 4 --unit effect-check

Omit an arm's --bin to build its checked-out commit in an isolated target dir.
Supplied binaries require --commit (the caller's source-commit declaration).
Configs default to each tree's --config-path; a historical --commit reads the
config from Git without changing the checkout. --config NAME=PATH overrides it,
but must resolve inside that same Git tree. Inputs and commands go to ab.json.
--preflight-only runs seconds-long probes, prints the diff and launches no study.
Otherwise systemd-run --user --collect owns the entire batch; inspect launch.log,
STATE.tsv in each stage, and DONE (which includes any failure/abort reason).
--early-minute 20 and --no-early-read control the optional metric tripwire.
All seeds finish the early checkpoint stage before any continuation, including
with a small parallel cap. Each continuation loads only its own new checkpoint;
external saved-start libraries are deliberately unsupported. batch.py has no SSH
launcher; remote orchestration, including eidolon upload, is deferred.
"""

import argparse
import csv
from decimal import Decimal, InvalidOperation
import json
import math
import os
from pathlib import Path
import re
import shlex
import subprocess
import sys
import time
import tomllib

from batch import atomic, execute, physical_cores, seed_list, topology
from study_contract import sha256


class GuardError(ValueError):
    pass


def git(tree, *args):
    return subprocess.check_output(["git", "-C", str(tree), *args],
                                   stderr=subprocess.PIPE, timeout=5)


def assignments(values, label):
    result = {}
    for value in values or []:
        name, sep, item = value.partition("=")
        if not sep or not item or not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_-]*", name) or name in result:
            raise GuardError(f"{label} needs unique NAME=VALUE entries")
        result[name] = item
    return result


def names(values):
    result = []
    for value in values or []:
        for key in value.split(","):
            if not key.strip():
                raise GuardError("empty expected key/metric")
            if key.strip() not in result:
                result.append(key.strip())
    return result


def prefix(wrapper, slot):
    return [sys.executable, str(wrapper), "--slot", str(slot), "--",
            "chrt", "-i", "0", "nice", "-n", "19"]


def tree_status(tree, out):
    # A study's own log/build/config artifacts do not make its source dirty.
    paths = ["--", "."]
    if out.is_relative_to(tree):
        paths.append(f":(exclude){out.relative_to(tree).as_posix()}")
    return git(tree, "status", "--porcelain", "--untracked-files=normal", *paths)


def arm_inputs(args, out):
    trees = assignments(args.arm, "--arm")
    binaries = assignments(args.bin, "--bin")
    commits = assignments(args.commit, "--commit")
    configs = assignments(args.config, "--config")
    if len(trees) < 2:
        raise GuardError("need at least two --arm NAME=TREE entries; first is baseline")
    if set(trees) & {"build", "early", "continuation", "full", "DONE"}:
        raise GuardError("arm name is reserved for launcher output")
    if (set(binaries) | set(commits) | set(configs)) - set(trees):
        raise GuardError("bin/commit/config names must name an arm")
    relative = Path(args.config_path)
    if not relative.parts or relative.is_absolute() or ".." in relative.parts or relative.parts[0] != "config":
        raise GuardError("--config-path must be a relative path under config/")
    arms = {}
    for name, value in trees.items():
        tree = Path(value).resolve(strict=True)
        if Path(git(tree, "rev-parse", "--show-toplevel").decode().strip()).resolve() != tree:
            raise GuardError(f"{name}: --arm must name a Git tree root")
        head = git(tree, "rev-parse", "HEAD").decode().strip()
        commit = git(tree, "rev-parse", "--verify", f"{commits.get(name, 'HEAD')}^{{commit}}").decode().strip()
        source_status = tree_status(tree, out)
        dirty = bool(source_status)
        if name in binaries and name not in commits:
            raise GuardError(f"{name}: a supplied binary requires --commit NAME=REF")
        source = Path(configs[name]) if name in configs else tree / relative
        if not source.is_absolute():
            source = tree / source
        source = source.resolve(strict=name in configs or commit == head)
        if not source.is_relative_to(tree) or Path(git(source.parent, "rev-parse", "--show-toplevel").decode().strip()).resolve() != tree:
            raise GuardError(f"{name}: config {source} resolves into a different tree than binary source {tree}")
        config_relative = source.relative_to(tree)
        if commit == head:
            content = source.read_bytes()
            origin = "working tree"
        else:
            content = git(tree, "show", f"{commit}:{config_relative.as_posix()}")
            origin = f"git:{commit}:{config_relative.as_posix()}"
            if name in configs and source.read_bytes() != content:
                raise GuardError(f"{name}: explicit config does not match historical source commit {commit}")
        root = out / name
        root.mkdir()
        config = root / "config.toml"
        config.write_bytes(content)
        if name in binaries:
            binary = Path(binaries[name]).resolve(strict=True)
        else:
            if commit != head:
                raise GuardError(f"{name}: build needs the checked-out commit; supply --bin for historical commits")
            target = out / "build" / name
            # Seed from the tree's own target so the build is incremental; reflink only, so a
            # cross-filesystem output never turns this into a full copy.
            if not target.exists() and (tree / "target").is_dir():
                target.parent.mkdir(parents=True, exist_ok=True)
                seeded = subprocess.run(["cp", "-a", "--reflink=always", str(tree / "target"), str(target)],
                                        stdin=subprocess.DEVNULL, capture_output=True)
                if seeded.returncode:
                    subprocess.run(["rm", "-rf", "--", str(target)], check=False)
            env = os.environ | {"CARGO_TARGET_DIR": str(target)}
            command = ["chrt", "-i", "0", "nice", "-n", "19", "cargo", "build", "--release",
                       "-p", "cubarium", "--example", "voxel_census"]
            print(f"build {name}: {commit} target={target}", flush=True)
            with (root / "build.log").open("w") as log:
                subprocess.run(command, cwd=tree, env=env, stdout=log, stderr=log,
                               stdin=subprocess.DEVNULL, check=True,
                               timeout=240 if args.preflight_only else 3600)
            if git(tree, "rev-parse", "HEAD").decode().strip() != head or tree_status(tree, out) != source_status:
                raise GuardError(f"{name}: source tree changed during build")
            binary = target / "release/examples/voxel_census"
        if not os.access(binary, os.X_OK):
            raise GuardError(f"{name}: binary is not executable: {binary}")
        arms[name] = dict(tree=str(tree), commit=commit, dirty=dirty, binary=str(binary),
                          binary_sha256=sha256(binary), config_path=str(source),
                          config_origin=origin, config=str(config), config_sha256=sha256(config),
                          commands=[])
    return arms


def unchanged(arm):
    for path, digest in ((arm["binary"], arm["binary_sha256"]), (arm["config"], arm["config_sha256"])):
        if sha256(path) != digest:
            raise GuardError(f"input changed after preflight: {path}")


def flatten(value, path=""):
    result = {}
    if isinstance(value, dict):
        for key, item in value.items():
            result.update(flatten(item, f"{path}.{key}" if path else key))
    else:
        result[path] = value
    return result


def enum_names(arm, file, enum):
    """The registry's array aliases are ALL order mapped through enum.name().

    Read the binary's declared source revision, rather than freezing today's
    roster/food order in this launcher. Fail closed if that interface changes.
    """
    tree = Path(arm["tree"])
    head = git(tree, "rev-parse", "HEAD").decode().strip()
    source = (tree / file).read_text() if head == arm["commit"] else git(tree, "show", f'{arm["commit"]}:{file}').decode()
    implementation = source.split(f"impl {enum} {{", 1)[-1]
    order = re.search(r"pub const ALL:[^=]+?=\s*\[(.*?)\];", implementation, re.S)
    name_function = re.search(r"pub (?:const )?fn name\([^)]*\)[^{]+\{(.*?)\n    \}", implementation, re.S)
    if not order or not name_function:
        raise GuardError(f"cannot resolve {enum} array aliases from {file} at {arm['commit']}")
    variants = re.findall(rf"{enum}::(\w+)", order[1])
    aliases = dict(re.findall(rf'{enum}::(\w+)\s*=>\s*"([^"]+)"', name_function[1]))
    if not variants or set(variants) - set(aliases):
        raise GuardError(f"incomplete {enum} array aliases in {file}")
    return [aliases[variant] for variant in variants]


def resolved_values(data, arm):
    founders = data.get("fauna", {}).get("founders")
    if isinstance(founders, list):
        lineages = enum_names(arm, "crates/cubarium-voxel-fauna/src/manifest.rs", "Founder")
        foods = enum_names(arm, "crates/cubarium-voxel-fauna/src/lib.rs", "Food")

        def named(items, aliases):
            if len(items) != len(aliases):
                raise GuardError("parameter array length differs from source registry aliases")
            return dict(zip(aliases, items))

        data["fauna"]["founders"] = named(founders, lineages)
        for founder in founders:
            for container, key, aliases in ((founder, "yield_by_food", foods),
                    (founder.get("gut", {}), "digestion_s", foods),
                    (founder.get("hunter", {}), "prey", lineages)):
                if isinstance(container.get(key), list):
                    container[key] = named(container[key], aliases)
        arm["registry_aliases"] = dict(founders=lineages, foods=foods)
    return flatten(data)


def parameter_probe(command, target, log):
    """params= is written before founding; stop only this owned probe immediately.

    expand= alone exports a genome, not resolved default values. The params dump
    supplies those defaults without waiting for a landscape to found or simulate.
    Wait for the owned binary to close its dump before reading: the file can
    become visible while the single Rust write is still in progress.
    """
    with log.open("w") as stderr:
        process = subprocess.Popen(command, stdin=subprocess.DEVNULL,
                                   stdout=subprocess.DEVNULL, stderr=stderr)
        try:
            deadline = time.monotonic() + 20
            while True:
                if target.exists():
                    try:
                        writing = any(fd.resolve() == target for fd in Path(f"/proc/{process.pid}/fd").glob("*"))
                        if not writing:
                            data = tomllib.loads(target.read_text())
                            if all(key in data for key in ("world", "flora", "fauna")):
                                return data
                    except tomllib.TOMLDecodeError:
                        pass
                if process.poll() is not None:
                    raise GuardError(f"parameter probe exited {process.returncode}: {log.read_text()[-600:]}")
                if time.monotonic() >= deadline:
                    raise GuardError(f"parameter probe timed out: {log}")
                time.sleep(0.01)
        finally:
            if process.poll() is None:
                process.terminate()
                try:
                    process.wait(timeout=2)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait(timeout=2)


def preflight(out, arms, expect, wrapper, slot_base=0):
    values = {}
    for index, (name, arm) in enumerate(arms.items()):
        unchanged(arm)
        root = out / name
        genome = root / "probe-genome.toml"
        genome.write_text("gene = []\n")
        command = prefix(wrapper, slot_base + index) + [arm["binary"], "0",
            f'config={arm["config"]}', f"genome={genome}", f"expand={root / 'expanded-genome.toml'}", "threads=1"]
        arm["commands"].append(dict(kind="expand", argv=command, line=shlex.join(command)))
        done = subprocess.run(command, capture_output=True, text=True,
                              stdin=subprocess.DEVNULL, timeout=20)
        (root / "expand.log").write_text(done.stdout + done.stderr)
        if done.returncode or not (root / "expanded-genome.toml").is_file():
            raise GuardError(f"{name}: expansion failed ({done.returncode}): {done.stderr[-600:]}")
        # No genome or overrides: dump exactly the arm's resolved config.
        target = root / "expanded.toml"
        command = prefix(wrapper, slot_base + index) + [arm["binary"], "0",
            f'config={arm["config"]}', f"params={target}", "threads=1"]
        arm["commands"].append(dict(kind="parameters", argv=command, line=shlex.join(command)))
        values[name] = resolved_values(parameter_probe(command, target, root / "params.log"), arm)
        arm["expanded_config_sha256"] = sha256(target)
    baseline = next(iter(arms))
    differences = {}
    absent = object()
    for name in list(arms)[1:]:
        left, right = values[baseline], values[name]
        changes = {key: [left.get(key, "<absent>"), right.get(key, "<absent>")]
                   for key in sorted(left.keys() | right.keys())
                   if left.get(key, absent) != right.get(key, absent)}
        differences[name] = changes
        print(f"preflight {baseline} -> {name}: {len(changes)} changed keys", flush=True)
        for key, (old, new) in changes.items():
            print(f"  {key}: {old!r} → {new!r}", flush=True)
        if not changes and arms[baseline]["binary_sha256"] == arms[name]["binary_sha256"]:
            raise GuardError(f"{baseline}/{name}: identical binary and expanded config")
        missing = [key for key in expect if key not in changes]
        if missing:
            raise GuardError(f"{baseline}/{name}: expected keys absent from diff: {', '.join(missing)}")
    print("preflight passed", flush=True)
    return differences


def early_read(out, stage, arm_names, seeds, metrics, minute):
    """Read the exact sim-minute, compare numeric values across every paired seed."""
    identical = True
    for seed in seeds:
        observed = {}
        for name in arm_names:
            path = stage / name / f"s{seed}" / "census.csv"
            with path.open(newline="") as stream:
                rows = csv.DictReader(stream)
                if not rows.fieldnames or "sim_min" not in rows.fieldnames or set(metrics) - set(rows.fieldnames):
                    raise GuardError(f"early read: missing sim_min/expected metric column in {path}")
                row = next((r for r in rows if r.get("sim_min") == str(minute)), None)
            if row is None:
                raise GuardError(f"early read: no census row at minute {minute} in {path}")
            try:
                observed[name] = {metric: Decimal(row[metric]) for metric in metrics}
                if any(not value.is_finite() for value in observed[name].values()):
                    raise InvalidOperation
            except (InvalidOperation, TypeError, KeyError) as error:
                raise GuardError(f"early read: incomplete/nonfinite metric in {path}") from error
        for metric in metrics:
            print(f"early minute={minute} seed={seed} {metric}: " + " | ".join(
                f"{name}={observed[name][metric]}" for name in arm_names), flush=True)
            identical &= len({observed[name][metric] for name in arm_names}) == 1
    if identical:
        reason = "early-identical: every expected metric identical across all seeds"
        atomic(out / "ABORTED.early-identical", reason + "\n")
        atomic(out / "DONE", reason + "\n")
        print(reason, flush=True)
    return not identical


def jobs_for(stage, arms, seeds, args, wrapper, hours, starts=None):
    jobs = []
    for seed in seeds:
        for name, arm in arms.items():
            root = stage / name / f"s{seed}"
            slot = args.slot_base + len(jobs)
            source = [f"load={starts[name, seed]}"] if starts else [f'config={arm["config"]}', f"seed={seed}"]
            command = prefix(wrapper, slot) + [arm["binary"], str(hours), *source,
                "heuristic", f"threads={args.threads}", f"vitals={root / 'vitals'}",
                f"run={root / 'run'}", "check_min=3", f"segment_min={args.segment_min}", "keep=3"]
            arm["commands"].append(dict(kind=stage.name, seed=seed, argv=command, line=shlex.join(command)))
            jobs.append(dict(arm=name, seed=seed, slot=slot, command=command, state="queued"))
    return jobs


def save_manifest(out, manifest):
    atomic(out / "ab.json", json.dumps(manifest, indent=2, default=str) + "\n")


def study(out, manifest, args, wrapper):
    groups = topology(wrapper)
    if args.threads > min(map(len, groups)):
        raise GuardError("--threads exceeds an assigned L3 group's CPU count")
    cpus = os.sched_getaffinity(0)
    ceiling = min(math.floor(len(cpus) * 0.9), len(cpus) - 1) // args.threads
    parallel = args.max_parallel if args.max_parallel is not None else max(1, min(ceiling, physical_cores(cpus) // args.threads))
    if not 1 <= parallel <= ceiling:
        raise GuardError(f"--max-parallel must be between 1 and {ceiling} (CPU reserve)")
    manifest.update(l3_groups=groups, max_parallel=parallel, stages={})
    seeds, arms = manifest["seeds"], manifest["arms"]
    metrics = manifest["expect_metric"]
    early = args.early_read and bool(metrics)
    horizons = [("early", args.early_minute / 60)] if early else [("full", args.hours)]
    starts = None
    if early and args.hours > args.early_minute / 60:
        horizons.append(("continuation", args.hours - args.early_minute / 60))
    print(f"parallel cap: {parallel}; early read: {early}", flush=True)
    for stage_name, hours in horizons:
        for arm in arms.values():
            unchanged(arm)
        stage = out / stage_name
        stage.mkdir()
        jobs = jobs_for(stage, arms, seeds, args, wrapper, hours, starts)
        manifest["stages"][stage_name] = jobs
        save_manifest(out, manifest)
        execute(stage, jobs, parallel, groups, args.threads)
        save_manifest(out, manifest)
        if any(job["state"] != "done" for job in jobs):
            raise GuardError(f"{stage_name}: failed/paused runs; see {stage / 'STATE.tsv'}")
        if stage_name == "early":
            if not early_read(out, stage, list(arms), seeds, metrics, args.early_minute):
                return
            starts = {}
            for job in jobs:
                checkpoint = stage / job["arm"] / f's{job["seed"]}' / "run/ckpt" / f"m{args.early_minute:06d}"
                if len(horizons) > 1 and not (checkpoint / "minute").is_file():
                    raise GuardError(f"missing early checkpoint: {checkpoint}")
                starts[job["arm"], job["seed"]] = checkpoint
    atomic(out / "DONE", "ok: all runs finished\n")


def detach(out, args, argv):
    if not args.unit or not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_.-]*", args.unit):
        raise GuardError("study runs require a safe --unit NAME for systemd detachment")
    command = ["systemd-run", "--user", f"--unit={args.unit}", "--collect",
               f"--property=WorkingDirectory={Path.cwd()}",
               f"--property=StandardOutput=append:{out / 'launch.log'}",
               f"--property=StandardError=append:{out / 'launch.log'}",
               "chrt", "-i", "0", "nice", "-n", "19", sys.executable, str(Path(__file__).resolve()),
               *argv, "--worker"]
    subprocess.run(command, check=True, timeout=10, stdin=subprocess.DEVNULL)
    print(f"detached unit={args.unit}; log={out / 'launch.log'}; completion={out / 'DONE'}", flush=True)


class LaunchLog:
    def __init__(self, log, stream):
        self.log = log
        self.stream = stream

    def write(self, content):
        self.stream.write(content)
        return self.log.write(content)

    def flush(self):
        self.stream.flush()
        self.log.flush()


def main(argv=None):
    argv = list(sys.argv[1:] if argv is None else argv)
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("out", type=Path)
    parser.add_argument("--arm", action="append", required=True, metavar="NAME=TREE")
    parser.add_argument("--bin", action="append", metavar="NAME=PATH")
    parser.add_argument("--commit", action="append", metavar="NAME=REF")
    parser.add_argument("--config", action="append", metavar="NAME=PATH")
    parser.add_argument("--config-path", default="config/desktop/terrarium-wide.toml")
    parser.add_argument("--expect", action="append", metavar="KEY[,KEY...]")
    parser.add_argument("--expect-metric", action="append", metavar="COLUMN[,COLUMN...]")
    parser.add_argument("--early-minute", type=int, default=20)
    parser.add_argument("--early-read", action=argparse.BooleanOptionalAction, default=True)
    parser.add_argument("--preflight-only", action="store_true")
    parser.add_argument("--unit")
    parser.add_argument("--seeds", default="1-8")
    parser.add_argument("--hours", type=float, default=2)
    parser.add_argument("--threads", type=int, default=1)
    parser.add_argument("--max-parallel", type=int)
    parser.add_argument("--slot-base", type=int, default=0)
    parser.add_argument("--segment-min", type=int, default=15)
    parser.add_argument("--worker", action="store_true", help=argparse.SUPPRESS)
    args = parser.parse_args(argv)
    out = args.out.resolve()
    log = None
    stdout = sys.stdout
    manifest = None
    owns_output = False
    try:
        seeds, expect, metrics = seed_list(args.seeds), names(args.expect), names(args.expect_metric)
        if args.threads < 1 or args.slot_base < 0 or args.segment_min < 1 or args.early_minute < 1 or not math.isfinite(args.hours) or args.hours <= 0:
            raise GuardError("require positive threads, minutes, hours and nonnegative slot-base")
        if not args.preflight_only and args.early_read and metrics and args.hours * 60 < args.early_minute:
            raise GuardError("--hours ends before --early-minute")
        if not args.worker:
            if out.exists() and any(out.iterdir()):
                raise GuardError("output directory must be new or empty")
        out.mkdir(parents=True, exist_ok=True)
        owns_output = True
        if not args.preflight_only and not args.worker:
            detach(out, args, argv)
            return 0
        if not args.worker:
            log = (out / "launch.log").open("a")
            sys.stdout = LaunchLog(log, stdout)
        wrapper = Path(__file__).resolve().parents[1] / "run-study.py"
        arms = arm_inputs(args, out)
        manifest = dict(arms=arms, seeds=seeds, expect=expect, expect_metric=metrics, args=vars(args))
        save_manifest(out, manifest)
        manifest["diff"] = preflight(out, arms, expect, wrapper, args.slot_base)
        save_manifest(out, manifest)
        if args.preflight_only:
            atomic(out / "DONE", "preflight-only: passed; no study launched\n")
        else:
            study(out, manifest, args, wrapper)
        return 1 if (out / "ABORTED.early-identical").exists() else 0
    except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as error:
        reason = f"aborted: {error}"
        print(reason, flush=True)
        if owns_output:
            atomic(out / "DONE", reason + "\n")
            if manifest:
                manifest["error"] = str(error)
                save_manifest(out, manifest)
        return 1
    finally:
        if log:
            sys.stdout = stdout
            log.close()


if __name__ == "__main__":
    sys.exit(main())
