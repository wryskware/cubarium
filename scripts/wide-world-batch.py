#!/usr/bin/env python3
"""The wide desktop terrarium's cost: one straight-through detached batch.

640 x 128 x 96 at 0.125 m (config/desktop/terrarium-wide.toml), 16 threads on one L3
slot, drawn headless at 2560x720 (4 px/voxel), the same measurement as
scripts/deep-world-batch.py: step ms/tick and its split over minutes 0-5 and 55-60,
pack and render at 2560x720, founding and 30-minute pictures.

Build first in this worktree:
  timeout 270 cargo build --release -p cubarium --example deep_world
Then launch and leave it alone:
  timeout 270 scripts/wide-world-batch.py --out runs/wide-world --slot 0
The launcher returns immediately. DONE means the arm and the table were written;
FAILED means a batch-level error or the single long timeout. No chunking, no gates.
"""

import argparse
from datetime import datetime, timezone
import json
import os
from pathlib import Path
import shlex
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
CONFIG = "config/desktop/terrarium-wide.toml"
RASTER = "2560x720"


def table(out, errors, complete):
    lines = [
        "# Wide desktop terrarium cost (640 x 128 x 96, 2560x720)",
        "",
        f"{'Finished' if complete else 'Partial'} batch; founding seed {SEED}; release build; "
        "live water_every=1; normal controllers and ecology; overlap on.",
        f"Headless lit sink at **{RASTER.replace('x', '×')}**, 4 px/voxel (the config's fixed size). "
        "Three frames per tick (60 frames/s at 1×). 16 threads = one L3 (8 cores/16 SMT threads), idle priority.",
        "World/flora/fauna are timed phase wall durations; overlap means **do not add them**. "
        "Pack and render are the sink's wall counters; GPU raster is its timestamp-query stage sum. "
        "Startup, sky settling and PNG readback are excluded.",
        "",
        "Tick max × = 50 / step_ms (20 ticks/s at 1×). Live max × at 60 fps = "
        "max(0, 1000 − 60 × render_ms) / [20 × (step_ms + pack_ms)]. Mean-cost headroom, not a latency guarantee.",
        "",
        "| Threads / CPUs | Sim min | Ticks | Step ms (max) | World ms | Flora ms | Fauna ms | "
        "Pack ms/tick | Render ms/frame | GPU raster ms/frame | Tick max × | Live max × (60 fps) | Stands / ground / animals |",
        "|:---|:---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|:---|",
    ]
    path = out / "wide-t16" / "cost.json"
    if path.exists():
        report = json.loads(path.read_text())
        for r in report["rows"]:
            lines.append(f"| 16 / {report['cpus']} | {r['window']} | {r['ticks']} | "
                         f"{r['step_ms']:.3f} ({r['step_max_ms']:.3f}) | {r['world_ms']:.3f} | "
                         f"{r['flora_ms']:.3f} | {r['fauna_ms']:.3f} | {r['pack_ms']:.3f} | "
                         f"{r['render_ms']:.3f} | {r['gpu_raster_ms']:.3f} | {r['tick_speed']:.2f} | "
                         f"{r['live_speed_60fps']:.2f} | {r['stands']} / {r['ground']} / {r['animals']} |")
        if not report["founding_accepted"]:
            lines += ["", "Explicit seed retained despite founding acceptance warnings: "
                      + "; ".join(report["founding_warnings"])]
    log = out / "wide-t16" / "run.log"
    if log.exists():
        lines += [""] + [f"`{l.strip()}`" for l in log.read_text().splitlines() if l.startswith("founding:")]
    lines += ["", "cost.json includes every phase and work counter, **including zeros**.", "",
              "- [founding](wide-t16/founding.png), [30 min](wide-t16/m30.png)",
              "", "## Execution", "", "```json", (out / "launch.json").read_text(), "```"]
    if errors:
        lines += ["", "## Unfinished or failed", ""] + [f"- {e}" for e in errors]
    (out / "headroom.md").write_text("\n".join(lines) + "\n")


def run(args, out):
    allowed = sorted(os.sched_getaffinity(0))
    if len(allowed) != 16:
        raise RuntimeError(f"expected one 16-thread L3 slot, got {allowed}")
    errors = []
    arm = out / "wide-t16"
    arm.mkdir(exist_ok=True)
    command = [str(ROOT / "target/release/examples/deep_world"), f"config={ROOT / CONFIG}",
               f"out={arm}", "threads=16", f"seed={args.seed}", "minutes=60", f"raster={RASTER}",
               "pictures=true"]
    print(f"{datetime.now(timezone.utc).isoformat()} {shlex.join(command)}", flush=True)
    with (arm / "run.log").open("w") as log:
        result = subprocess.run(command, cwd=ROOT, stdout=log, stderr=subprocess.STDOUT)
    if result.returncode:
        errors.append(f"exit {result.returncode}; see {arm / 'run.log'}")
    else:
        report = json.loads((arm / "cost.json").read_text())
        if ([row["window"] for row in report["rows"]] != ["0–5", "55–60"]
                or any(row["ticks"] != 6000 for row in report["rows"])):
            errors.append("incomplete measurement windows")
        if any(not (arm / name).is_file() for name in ("founding.png", "m30.png")):
            errors.append("missing requested pictures")
    table(out, errors, True)
    (out / "exit.json").write_text(json.dumps({"errors": errors, "finished": datetime.now(timezone.utc).isoformat()}, indent=2) + "\n")
    (out / "DONE").write_text("finished with errors\n" if errors else "success\n")
    return bool(errors)


def launch(args, out):
    binary = ROOT / "target/release/examples/deep_world"
    if not binary.is_file():
        raise RuntimeError(f"build {binary} first")
    if (out / "launch.json").exists():
        raise RuntimeError(f"{out} already has a launch; choose a fresh output directory")
    command = ["timeout", "--signal=TERM", "--kill-after=30s", str(args.timeout),
               "chrt", "-i", "0", "nice", "-n", "19", str(ROOT / "scripts/run-study.py"),
               "--slot", str(args.slot), "--", sys.executable, str(Path(__file__).resolve()),
               "--run", "--out", str(out), "--seed", str(args.seed)]
    failed = shlex.quote(str(out / "FAILED"))
    body = f"{shlex.join(command)}; rc=$?; if [ $rc -ne 0 ]; then printf 'exit %s\\n' \"$rc\" > {failed}; fi; exit $rc"
    metadata = {"root": str(ROOT), "started": datetime.now(timezone.utc).isoformat(),
                "commit": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
                "slot": args.slot, "seed": args.seed, "timeout_seconds": args.timeout,
                "command": shlex.join(command), "load_average": list(os.getloadavg())}
    (out / "launch.json").write_text(json.dumps(metadata, indent=2) + "\n")
    table(out, [], False)
    with (out / "batch.log").open("w") as log:
        process = subprocess.Popen(["setsid", "nohup", "bash", "-c", body], cwd=ROOT,
                                   stdin=subprocess.DEVNULL, stdout=log, stderr=subprocess.STDOUT,
                                   close_fds=True)
    (out / "pid").write_text(f"{process.pid}\n")
    print(f"Detached batch PID {process.pid}; slot {args.slot}; output {out}")
    print(f"Table {out / 'headroom.md'}; completion {out / 'DONE'}; failure {out / 'FAILED'}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--slot", type=int, default=0)
    parser.add_argument("--seed", type=int, default=1)
    parser.add_argument("--timeout", type=int, default=21600, help="single whole-batch timeout, seconds")
    parser.add_argument("--run", action="store_true", help=argparse.SUPPRESS)
    args = parser.parse_args()
    SEED = args.seed
    out = args.out.resolve()
    out.mkdir(parents=True, exist_ok=True)
    try:
        sys.exit(run(args, out) if args.run else launch(args, out))
    except Exception as error:
        if args.run:
            (out / "FAILED").write_text(f"{type(error).__name__}: {error}\n")
        raise
