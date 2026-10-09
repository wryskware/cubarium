#!/usr/bin/env python3
"""Launch bounded paired, staged voxel_census runs on this host (stdlib only).

Copy this script and study_contract.py into scripts/studies, and run-study.py
into scripts on a remote
host; no repository is needed. Inputs and the binary are supplied by path.
--detach returns immediately; inspect STATE.tsv and wait for DONE externally.
DONE means every child ended, including failures and pauses, not success.
set= beside load= changes the loaded world's configs (flora values in authored units);
a rate change branched from a save shows a transient first.
"""

import argparse
import csv
import glob
import importlib.util
import json
import math
import os
from pathlib import Path
import re
import shlex
import shutil
import subprocess
import sys

from study_contract import founding_required, load_census, read_required, sha256


def atomic(path, content):
    temporary = path.with_suffix(path.suffix + ".tmp")
    temporary.write_text(content)
    temporary.replace(path)


def seed_list(value):
    seeds = []
    for part in value.split(","):
        match = re.fullmatch(r"(\d+)(?:-(\d+))?", part)
        if not match:
            raise ValueError("seeds must be a range or comma-separated integers")
        first, last = int(match[1]), int(match[2] or match[1])
        if last < first:
            raise ValueError("seed range is reversed")
        seeds.extend(range(first, last + 1))
    if len(set(seeds)) != len(seeds):
        raise ValueError("duplicate seed")
    return seeds


def physical_cores(allowed):
    """Distinct (package, core) pairs among the allowed CPUs; the logical count if sysfs
    topology is unreadable."""
    cores = set()
    for cpu in allowed:
        base = Path(f"/sys/devices/system/cpu/cpu{cpu}/topology")
        try:
            cores.add((base.joinpath("physical_package_id").read_text().strip(),
                       base.joinpath("core_id").read_text().strip()))
        except OSError:
            return len(allowed)
    return len(cores) or len(allowed)


def start_seed(path):
    # A checkpoint's sibling census is authoritative when available. For a
    # standalone start library accept sN / seedN directory names, with any
    # map prefix (d72-s4, train-flat_seed3).
    for parent in path.parents:
        census = parent / "census.csv"
        if census.is_file():
            with census.open(newline="") as stream:
                row = next(csv.DictReader(stream), None)
            if row and row.get("seed"):
                return int(row["seed"])
        match = re.fullmatch(r"(?:.*[-_])?(?:s|seed[-_]?)(\d+)", parent.name)
        if match:
            return int(match[1])
    raise ValueError(f"cannot identify seed for {path}; use sN directories or a sibling census.csv")


def topology(wrapper):
    spec = importlib.util.spec_from_file_location("study_placement", wrapper)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module.l3_groups(os.sched_getaffinity(0))


def status(run):
    for state in ("FAILED", "PAUSED", "DONE"):
        if (run / state).exists():
            return state.lower(), minute(run)
    return "failed", minute(run)


def minute(run):
    try:
        match = re.search(r"\bminute=(\d+)", (run / "status").read_text())
        return int(match[1]) if match else ""
    except OSError:
        return ""


def write_state(out, jobs):
    rows = ["arm\tseed\tstate\tminute"]
    for job in jobs:
        current = minute(out / job["arm"] / f's{job["seed"]}' / "run")
        rows.append(f'{job["arm"]}\t{job["seed"]}\t{job["state"]}\t{current}')
    atomic(out / "STATE.tsv", "\n".join(rows) + "\n")


def verify_threads(pid, cpus):
    # Affinity is applied before exec; verify every currently visible thread.
    for task in Path(f"/proc/{pid}/task").glob("*"):
        try:
            if not os.sched_getaffinity(int(task.name)) <= set(cpus):
                raise RuntimeError(f"pid {pid} thread {task.name} widened affinity")
        except ProcessLookupError:
            pass


def execute(out, jobs, parallel, groups, threads):
    active = {}
    pending = [job for job in jobs if job["state"] == "queued"]
    while active or pending:
        while len(active) < parallel and pending:
            # Unequal L3 groups must not be oversubscribed by the global budget.
            occupied = {}
            for _, _, cpus in active.values():
                occupied[tuple(cpus)] = occupied.get(tuple(cpus), 0) + threads
            index = next((index for index, job in enumerate(pending)
                          if occupied.get(tuple(groups[job["slot"] % len(groups)]), 0) + threads
                          <= len(groups[job["slot"] % len(groups)])), None)
            if index is None:
                break
            job = pending.pop(index)
            root = out / job["arm"] / f's{job["seed"]}'
            root.mkdir(parents=True, exist_ok=True)
            cpus = groups[job["slot"] % len(groups)]

            def confine():
                os.sched_setaffinity(0, cpus)

            try:
                with (root / "census.csv").open("w") as stdout, (root / "err.log").open("w") as stderr:
                    process = subprocess.Popen(job["command"], stdout=stdout, stderr=stderr,
                                               stdin=subprocess.DEVNULL, preexec_fn=confine)
                active[process.pid] = (process, job, cpus)
                job["state"] = "running"
                job["pid"] = process.pid
                verify_threads(process.pid, cpus)
            except OSError as error:
                job["state"] = "failed"
                (root / "err.log").write_text(str(error) + "\n")
            write_state(out, jobs)
        if active:
            # Blocking child notification, not a polling loop.
            pid, wait_status = os.waitpid(-1, 0)
            process, job, _ = active.pop(pid)
            process.returncode = os.waitstatus_to_exitcode(wait_status)
            job["returncode"] = process.returncode
            job["state"], _ = status(out / job["arm"] / f's{job["seed"]}' / "run")
            if process.returncode:
                job["state"] = "failed"
            for live_pid, (_, _, cpus) in active.items():
                verify_threads(live_pid, cpus)
            write_state(out, jobs)
    atomic(out / "DONE", "all runs ended; see STATE.tsv\n")


def detach(out):
    if os.fork():
        return False
    os.setsid()
    if os.fork():
        os._exit(0)
    with open(os.devnull, "rb") as source, (out / "batch.log").open("ab", buffering=0) as log:
        os.dup2(source.fileno(), 0)
        os.dup2(log.fileno(), 1)
        os.dup2(log.fileno(), 2)
    return True


def resume(out, binary, wrapper, parallel, groups, threads):
    """Continue incomplete children; completed children and their telemetry stay intact."""
    manifest = json.loads((out / "batch.json").read_text())
    if sha256(binary) != manifest["binary_sha256"]:
        raise ValueError("resume binary differs from batch.json")
    identity = manifest["identity"]
    for item in (identity.get("config"), identity.get("required")):
        if item and sha256(item["path"]) != item["sha256"]:
            raise ValueError(f"resume input changed: {item['path']}")
    for policies in identity["policies"].values():
        for item in policies.values():
            if sha256(item["path"]) != item["sha256"]:
                raise ValueError(f"resume policy changed: {item['path']}")
    for path, digest in identity.get("genomes", {}).items():
        if sha256(path) != digest:
            raise ValueError(f"resume genome changed: {path}")
    for seed, hashes in identity["starts"].items():
        for name, digest in hashes.items():
            if sha256(Path(manifest["starts"][seed]) / name) != digest:
                raise ValueError(f"resume start changed: seed {seed}/{name}")
    jobs = manifest["runs"]
    for job in jobs:
        root = out / job["arm"] / f's{job["seed"]}'
        run = root / "run"
        if (run / "DONE").exists() and not (run / "FAILED").exists():
            job["state"] = "done"
            continue
        # A failed liveness check needs investigation, not an automatic retry.
        if (run / "FAILED").exists():
            job["state"] = "failed"
            continue
        command = job["command"]
        job.setdefault("original_command", command)
        prefix = [sys.executable, str(wrapper), "--slot", str(job["slot"]), "--"]
        if any((run / "ckpt").glob("m*/minute")):
            job["command"] = prefix + ["chrt", "-i", "0", "nice", "-n", "19", str(binary),
                                        f"resume={run}", f"threads={threads}"]
        else:
            # Only task-owned incomplete outputs with no committed checkpoint.
            if root.exists():
                shutil.rmtree(root)
            original = job["original_command"]
            tail = original[original.index("--") + 1:]
            tail[tail.index("19") + 1] = str(binary)
            tail = [f"threads={threads}" if a.startswith("threads=") else a for a in tail]
            job["command"] = prefix + tail
        job["state"] = "queued"
    manifest.update(runs=jobs, l3_groups=groups, max_parallel=parallel)
    atomic(out / "batch.json", json.dumps(manifest, indent=2) + "\n")
    for marker in ("DONE", "PAUSED", "STOP"):
        (out / marker).unlink(missing_ok=True)
    write_state(out, jobs)
    execute(out, jobs, parallel, groups, threads)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("out", type=Path)
    parser.add_argument("--bin", required=True, type=Path)
    parser.add_argument("--arm", action="append", metavar='NAME="ARGS"')
    parser.add_argument("--resume", action="store_true")
    sources = parser.add_mutually_exclusive_group()
    sources.add_argument("--starts", metavar="GLOB")
    sources.add_argument("--config", type=Path)
    parser.add_argument("--seeds", default="1-8")
    parser.add_argument("--hours", type=float)
    parser.add_argument("--threads", type=int, default=1)
    parser.add_argument("--max-parallel", type=int)
    parser.add_argument("--slot-base", type=int, default=0)
    parser.add_argument("--stage", default="check_min=3 segment_min=15 keep=3")
    parser.add_argument("--detach", action="store_true")
    parser.add_argument("--required", type=Path, help="TOML: [required] animals, plants; [required.N_ref] founding bodies + eggs")

    args = parser.parse_args(argv)
    try:
        if args.resume:
            if args.arm or args.starts or args.config or args.required or args.hours is not None or args.detach:
                raise ValueError("resume uses saved inputs; pass only bin, threads and max-parallel")
            manifest = json.loads((args.out / "batch.json").read_text())
            args.hours = manifest["args"]["hours"]
            args.threads = manifest["args"]["threads"]
        elif not args.arm or not (args.starts or args.config) or args.hours is None:
            raise ValueError("new batches need --arm, --hours and --config or --starts")
        if args.threads < 1 or args.slot_base < 0 or not math.isfinite(args.hours) or args.hours <= 0:
            raise ValueError("require positive threads/hours and nonnegative slot-base")
        binary = args.bin.resolve(strict=True)
        wrapper = Path(__file__).resolve().parents[1] / "run-study.py"
        groups = topology(wrapper)
        if args.threads > min(map(len, groups)):
            raise ValueError("--threads exceeds an assigned L3 group's CPU count")
        cpus = len(os.sched_getaffinity(0))
        ceiling = min(math.floor(cpus * 0.9), cpus - 1) // args.threads
        # Default: one world per physical core. Measured 2026-10-08 on the desktop (16 cores,
        # 32 threads, mature d72 at 1 thread): 16 worlds gave 35 sim-h per wall-h, 24 gave 26
        # and 30 gave 31; SMT siblings add contention, not throughput.
        physical = physical_cores(os.sched_getaffinity(0)) // args.threads
        parallel = args.max_parallel if args.max_parallel is not None else max(1, min(ceiling, physical))
        if not 1 <= parallel <= ceiling:
            raise ValueError(f"--max-parallel must be between 1 and {ceiling} (CPU reserve)")
        print(f"parallel cap: {parallel} (CPU ceiling {ceiling})", flush=True)
        if args.resume:
            resume(args.out.resolve(), binary, wrapper, parallel, groups, args.threads)
            return
        starts = {}
        load_minutes = {}
        required = read_required(args.required.resolve(strict=True)) if args.required else None

        if args.starts:
            for matched in sorted(glob.glob(args.starts)):
                path = Path(matched).resolve(strict=True)
                if not (path / "minute").is_file():
                    raise ValueError(f"not a checkpoint: {path}")
                seed = start_seed(path)
                if seed in starts:
                    raise ValueError(f"multiple starts for seed {seed}")
                starts[seed] = str(path)
                minute_value = float((path / "minute").read_text().strip())
                if not math.isfinite(minute_value) or minute_value < 0 or not minute_value.is_integer():
                    raise ValueError(f"invalid checkpoint minute: {path}")
                load_minutes[str(seed)] = int(minute_value)
            if not starts:
                raise ValueError("--starts matched no checkpoints")
            seeds = sorted(starts)
            start_args = {s: [f"load={starts[s]}"] for s in seeds}
            rosters = {str(seed): dict(required or founding_required(Path(starts[seed]))) for seed in seeds}
            for seed in seeds:
                boundary = load_census(Path(starts[seed]), load_minutes[str(seed)])
                if boundary:
                    rosters[str(seed)]["load_census"] = boundary
        else:
            seeds = seed_list(args.seeds)
            config = args.config.resolve(strict=True)
            start_args = {s: [f"config={config}", f"seed={s}"] for s in seeds}
            if required is None:
                raise ValueError("fresh --config batches need --required; no founding census exists yet")
            rosters = {str(seed): required for seed in seeds}
            load_minutes = {str(seed): 0 for seed in seeds}
        arms = {}
        reserved = ("load=", "resume=", "seed=", "config=", "threads=", "run=", "vitals=", "save=", "check_min=", "segment_min=", "keep=", "pin=", "modes=")
        for arm in args.arm:
            name, separator, value = arm.partition("=")
            if not separator or not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_-]*", name) or name in arms:
                raise ValueError("arms need unique safe NAME=ARGS names")
            extra = shlex.split(value)
            if any(arg.startswith(reserved) or re.fullmatch(r"[\d.]+", arg) for arg in extra):
                raise ValueError("arm overrides a batch-owned argument")
            if args.starts and any(arg.startswith(("preset=", "eggbank=", "introduce=", "world=", "gap=", "night=")) or arg in {"generated", "closed", "nofauna", "still"} for arg in extra):
                raise ValueError("voxel_census refuses founding overrides beside load= (set= is fine); use --config for these arms")
            arms[name] = extra
        policies = {}
        for name, extra in arms.items():
            policies[name] = {}
            for argument in extra:
                if argument.startswith("policy="):
                    lineage, separator, file = argument[len("policy="):].partition("=")
                    if not separator or not lineage or not file:
                        raise ValueError("policy needs policy=LINEAGE=FILE")
                    path = Path(file).resolve(strict=True)
                    policies[name][lineage] = dict(path=str(path), sha256=sha256(path))
        identity = dict(config=dict(path=str(config), sha256=sha256(config)) if args.config else None,
                        starts={str(seed): {name: sha256(Path(path) / name)
                                           for name in ("world.bin", "flora.bin", "fauna.bin")}
                                for seed, path in starts.items()},
                        policies=policies,
                        genomes={str(Path(arg[len("genome="):]).resolve(strict=True)):
                                 sha256(Path(arg[len("genome="):]))
                                 for extra in arms.values() for arg in extra if arg.startswith("genome=")},
                        required=dict(path=str(args.required.resolve()), sha256=sha256(args.required)) if args.required else None)
        stage = shlex.split(args.stage)
        if any(not re.fullmatch(r"(check_min|segment_min|keep)=[1-9]\d*", arg) for arg in stage):
            raise ValueError("stage accepts positive check_min, segment_min and keep")
        out = args.out.resolve()
        if out.exists() and any(out.iterdir()):
            raise ValueError("output directory must be new or empty")
        out.mkdir(parents=True, exist_ok=True)
        jobs = []
        # Seed-major order keeps the paired arms close in launch time.
        for seed in seeds:
            for arm, extra in arms.items():
                root = out / arm / f"s{seed}"
                slot = args.slot_base + len(jobs)
                command = [sys.executable, str(wrapper), "--slot", str(slot), "--", "chrt", "-i", "0", "nice", "-n", "19", str(binary), str(args.hours), *start_args[seed], *extra, f"threads={args.threads}", f"vitals={root / 'vitals'}", f"run={root / 'run'}", *stage]
                jobs.append(dict(arm=arm, seed=seed, slot=slot, command=command, state="queued"))
        try:
            commit = subprocess.check_output(["git", "rev-parse", "HEAD"], stderr=subprocess.DEVNULL, timeout=5, text=True).strip()
        except (OSError, subprocess.SubprocessError):
            commit = None
        manifest = dict(arms=arms, seeds=seeds, starts=starts, load_minutes=load_minutes,
                        start_kind="saved start" if args.starts else "fresh founding",
                        required={"by_seed": rosters}, identity=identity,
                        args=vars(args) | {"out": str(out), "bin": str(binary),
                                           "config": str(args.config.resolve()) if args.config else None,
                                           "required": str(args.required.resolve()) if args.required else None},
                        commit=commit, binary_sha256=sha256(binary), l3_groups=groups,
                        max_parallel=parallel, runs=jobs)
        atomic(out / "batch.json", json.dumps(manifest, indent=2) + "\n")
        write_state(out, jobs)
        if args.detach and not detach(out):
            print(f"detached batch: {out}")
            return
        execute(out, jobs, parallel, groups, args.threads)
    except (OSError, ValueError, RuntimeError) as error:
        parser.exit(1, f"batch: {error}\n")


if __name__ == "__main__":
    main()
