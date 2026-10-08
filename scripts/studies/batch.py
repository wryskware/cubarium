#!/usr/bin/env python3
"""Launch bounded paired, staged voxel_census runs on this host (stdlib only).

Copy this script into scripts/studies and run-study.py into scripts on a remote
host; no repository is needed. Inputs and the binary are supplied by path.
--detach returns immediately; inspect STATE.tsv and wait for DONE externally.
DONE means every child ended, including failures and pauses, not success.
set= beside load= changes the loaded world's configs (flora values in authored units);
a rate change branched from a save shows a transient first.
"""

import argparse
import csv
import glob
import hashlib
import importlib.util
import json
import math
import os
from pathlib import Path
import re
import resource
import shlex
import subprocess
import sys


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
    pending = list(jobs)
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
            root.mkdir(parents=True)
            cpus = groups[job["slot"] % len(groups)]

            def confine():
                os.sched_setaffinity(0, cpus)
                limit = 64 * 1024**3
                old_hard = resource.getrlimit(resource.RLIMIT_AS)[1]
                if old_hard != resource.RLIM_INFINITY:
                    limit = min(limit, old_hard)
                resource.setrlimit(resource.RLIMIT_AS, (limit, limit))

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


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("out", type=Path)
    parser.add_argument("--bin", required=True, type=Path)
    parser.add_argument("--arm", required=True, action="append", metavar='NAME="ARGS"')
    sources = parser.add_mutually_exclusive_group(required=True)
    sources.add_argument("--starts", metavar="GLOB")
    sources.add_argument("--config", type=Path)
    parser.add_argument("--seeds", default="1-8")
    parser.add_argument("--hours", required=True, type=float)
    parser.add_argument("--threads", type=int, default=1)
    parser.add_argument("--max-parallel", type=int)
    parser.add_argument("--slot-base", type=int, default=0)
    parser.add_argument("--stage", default="check_min=3 segment_min=15 keep=3")
    parser.add_argument("--detach", action="store_true")
    args = parser.parse_args(argv)
    try:
        if args.threads < 1 or args.slot_base < 0 or not math.isfinite(args.hours) or args.hours <= 0:
            raise ValueError("require positive threads/hours and nonnegative slot-base")
        binary = args.bin.resolve(strict=True)
        wrapper = Path(__file__).resolve().parents[1] / "run-study.py"
        groups = topology(wrapper)
        if args.threads > min(map(len, groups)):
            raise ValueError("--threads exceeds an assigned L3 group's CPU count")
        cpus = len(os.sched_getaffinity(0))
        ceiling = min(math.floor(cpus * 0.9), cpus - 1) // args.threads
        parallel = args.max_parallel if args.max_parallel is not None else ceiling
        if not 1 <= parallel <= ceiling:
            raise ValueError(f"--max-parallel must be between 1 and {ceiling} (CPU reserve)")
        starts = {}
        if args.starts:
            for matched in sorted(glob.glob(args.starts)):
                path = Path(matched).resolve(strict=True)
                if not (path / "minute").is_file():
                    raise ValueError(f"not a checkpoint: {path}")
                seed = start_seed(path)
                if seed in starts:
                    raise ValueError(f"multiple starts for seed {seed}")
                starts[seed] = str(path)
            if not starts:
                raise ValueError("--starts matched no checkpoints")
            seeds = sorted(starts)
            start_args = {s: [f"load={starts[s]}"] for s in seeds}
        else:
            seeds = seed_list(args.seeds)
            config = args.config.resolve(strict=True)
            start_args = {s: [f"config={config}", f"seed={s}"] for s in seeds}
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
        digest = hashlib.sha256()
        with binary.open("rb") as stream:
            for block in iter(lambda: stream.read(1024**2), b""):
                digest.update(block)
        manifest = dict(arms=arms, seeds=seeds, starts=starts, args=vars(args) | {"out": str(out), "bin": str(binary), "config": str(args.config.resolve()) if args.config else None}, commit=commit, binary_sha256=digest.hexdigest(), l3_groups=groups, max_parallel=parallel, runs=jobs)
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
