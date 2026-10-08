#!/usr/bin/env python3
"""One straight-through depth cost batch, launched detached with one long timeout.

Build first in this worktree:
  timeout 270 cargo build --release -p cubarium --example deep_world
Then launch and leave it alone:
  timeout 270 scripts/deep-world-batch.py --out runs/deep-world --slot 1
The launcher returns immediately. DONE means all arms and the final table were
written; consult exit.json/headroom.md for errors. FAILED means a batch-level error
or the single long timeout. No checkpoint archives, chunking or counter gates.
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


def three_cores(allowed):
    """Keep both SMT siblings of three physical cores in this L3 slot."""
    cores = {}
    for cpu in sorted(allowed):
        topology = Path(f"/sys/devices/system/cpu/cpu{cpu}/topology")
        key = tuple((topology / name).read_text().strip()
                    for name in ("physical_package_id", "core_id"))
        cores.setdefault(key, []).append(cpu)
    if len(cores) < 3 or any(len(pair) != 2 for pair in list(cores.values())[:3]):
        raise RuntimeError("six-thread arm needs three physical cores with both SMT siblings")
    return sorted(cpu for pair in list(cores.values())[:3] for cpu in pair)


def comparison(out):
    from PIL import Image, ImageDraw

    tile_w, tile_h, label_h = 1280, 800, 32
    phases = [("founding", "founding"), ("m30", "30 sim-min")]
    if not any((out / f"d{depth}-t16/m30.png").is_file() for depth in (48, 72, 96)):
        phases = phases[:1]
    canvas = Image.new("RGB", (3 * tile_w, len(phases) * (tile_h + label_h)), "#111322")
    draw = ImageDraw.Draw(canvas)
    for col, depth in enumerate((48, 72, 96)):
        for row, (name, label) in enumerate(phases):
            path = out / f"d{depth}-t16" / f"{name}.png"
            if not path.exists():
                continue
            with Image.open(path) as im:
                im = im.convert("RGB").resize((tile_w, tile_h), Image.Resampling.LANCZOS)
                canvas.paste(im, (col * tile_w, row * (tile_h + label_h) + label_h))
            draw.text((col * tile_w + 12, row * (tile_h + label_h) + 8),
                      f"depth {depth} | {label} | seed 1".replace("seed 1", f"seed {SEED}"), fill="white")
    canvas.save(out / "side-by-side.png")


def table(out, errors, complete):
    lines = [
        "# Desktop terrarium depth headroom",
        "",
        f"{'Finished' if complete else 'Partial'} batch; founding seed {SEED} per depth; release build; "
        "live water_every=1; normal controllers and ecology; overlap on.",
        "Headless lit sink at **2560×1600**; auto integer voxel scale, whole strip in view. "
        "Three frames per tick (60 frames/s at 1×). Founding and 30-minute pictures use the 16-thread arm.",
        "",
        "The 16-thread budget is one L3 (8 physical cores/16 SMT threads). The 6-thread budget "
        "uses both SMT siblings of three cores in that same L3. It is a desktop CPU proxy for "
        "voidrunner's core budget, not a measurement on voidrunner. Arms run sequentially at idle priority.",
        "World/flora/fauna are timed phase wall durations; overlap means **do not add them**. "
        "Step is the measured whole tick. Pack and render are the sink's wall counters; "
        "GPU raster is its timestamp-query stage sum (excluding the sized presentation pass); "
        "render wall time includes that pass. Startup, sky settling and PNG readback are excluded.",
        "",
        "Tick max × = 50 / step_ms (20 ticks/s at 1×). Live max × at 60 fps = "
        "max(0, 1000 − 60 × render_ms) / [20 × (step_ms + pack_ms)]; this includes pack/render "
        "on the host loop and holds display rate constant as speed rises. Mean-cost headroom, not a latency guarantee.",
        "",
        "| Depth | Threads / CPUs | Sim min | Ticks | Step ms (max) | World ms | Flora ms | Fauna ms | "
        "Pack ms/tick | Render ms/frame | GPU raster ms/frame | Tick max × | Live max × (60 fps) | Stands / ground / animals |",
        "|---:|:---|:---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|:---|",
    ]
    for depth in (48, 72, 96):
        for threads in (16, 6):
            path = out / f"d{depth}-t{threads}" / "cost.json"
            if not path.exists():
                continue
            report = json.loads(path.read_text())
            for r in report["rows"]:
                lines.append(f"| {depth} | {threads} / {report['cpus']} | {r['window']} | {r['ticks']} | "
                             f"{r['step_ms']:.3f} ({r['step_max_ms']:.3f}) | {r['world_ms']:.3f} | "
                             f"{r['flora_ms']:.3f} | {r['fauna_ms']:.3f} | {r['pack_ms']:.3f} | "
                             f"{r['render_ms']:.3f} | {r['gpu_raster_ms']:.3f} | {r['tick_speed']:.2f} | "
                             f"{r['live_speed_60fps']:.2f} | {r['stands']} / {r['ground']} / {r['animals']} |")
            if not report["founding_accepted"]:
                lines += ["", f"d{depth}/t{threads}: explicit seed retained despite founding acceptance warnings: "
                          + "; ".join(report["founding_warnings"])]
    lines += ["", "Per-arm cost.json includes every phase and work counter, **including zeros**. "
              "No run stops because a population or counter is zero.", "",
              "Terrain: preserve the rear 6 m massif; add a low front plain and rolling hills. "
              "Extend the existing off-centre lake through the foreground to the glass. "
              "The existing aquifer spring/router feeds it; no new atmosphere-fed stream.", "",
              "[Six-view comparison](side-by-side.png)", ""]
    for depth in (48, 72, 96):
        lines.append(f"- d{depth}: [founding](d{depth}-t16/founding.png), [30 min](d{depth}-t16/m30.png)")
    lines += ["", "## Execution", "", "```json", (out / "launch.json").read_text(), "```"]
    if errors:
        lines += ["", "## Unfinished or failed arms", ""] + [f"- {e}" for e in errors]
    (out / "headroom.md").write_text("\n".join(lines) + "\n")


def run(args, out):
    # Fail dependency/setup errors before any long simulation.
    from PIL import Image  # noqa: F401

    allowed = sorted(os.sched_getaffinity(0))
    if len(allowed) != 16:
        raise RuntimeError(f"expected one 16-thread L3 slot, got {allowed}")
    budgets = {16: allowed, 6: three_cores(allowed)}
    errors = []
    for depth in (48, 72, 96):
        for threads in (16, 6):
            arm = out / f"d{depth}-t{threads}"
            arm.mkdir(exist_ok=True)
            config = ROOT / "config/desktop" / ("terrarium.toml" if depth == 48 else f"terrarium-d{depth}.toml")
            command = ["taskset", "--cpu-list", ",".join(map(str, budgets[threads])),
                       str(ROOT / "target/release/examples/deep_world"), f"config={config}",
                       f"out={arm}", f"threads={threads}", f"seed={args.seed}", "minutes=60",
                       f"pictures={'true' if threads == 16 else 'false'}"]
            print(f"{datetime.now(timezone.utc).isoformat()} {shlex.join(command)}", flush=True)
            with (arm / "run.log").open("w") as log:
                result = subprocess.run(command, cwd=ROOT, stdout=log, stderr=subprocess.STDOUT)
            if result.returncode:
                errors.append(f"d{depth}-t{threads}: exit {result.returncode}; see {arm / 'run.log'}")
            else:
                report = json.loads((arm / "cost.json").read_text())
                if ([row["window"] for row in report["rows"]] != ["0–5", "55–60"]
                        or any(row["ticks"] != 6000 for row in report["rows"])):
                    errors.append(f"d{depth}-t{threads}: incomplete measurement windows")
                if threads == 16 and any(not (arm / name).is_file() for name in ("founding.png", "m30.png")):
                    errors.append(f"d{depth}-t{threads}: missing requested pictures")
            table(out, errors, False)
    comparison(out)
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
    # The only long timeout covers the entire script and every sequential arm. No
    # per-arm/segment timeouts. The shell writes FAILED if this timeout kills Python.
    command = ["timeout", "--signal=TERM", "--kill-after=30s", str(args.timeout),
               "chrt", "-i", "0", "nice", "-n", "19", str(ROOT / "scripts/run-study.py"),
               "--slot", str(args.slot), "--", sys.executable, str(Path(__file__).resolve()),
               "--run", "--out", str(out), "--seed", str(args.seed)]
    failed = shlex.quote(str(out / "FAILED"))
    body = f"{shlex.join(command)}; rc=$?; if [ $rc -ne 0 ]; then printf 'exit %s\\n' \"$rc\" > {failed}; fi; exit $rc"
    metadata = {"root": str(ROOT), "started": datetime.now(timezone.utc).isoformat(),
                "commit": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
                "slot": args.slot, "seed": args.seed, "timeout_seconds": args.timeout,
                "command": shlex.join(command), "load_average": list(os.getloadavg()),
                "cpu": next((l.split(":", 1)[1].strip() for l in Path("/proc/cpuinfo").read_text().splitlines()
                             if l.startswith("model name")), "unknown")}
    (out / "launch.json").write_text(json.dumps(metadata, indent=2) + "\n")
    table(out, [], False)
    with (out / "batch.log").open("w") as log:
        process = subprocess.Popen(["setsid", "nohup", "bash", "-c", body], cwd=ROOT,
                                   stdin=subprocess.DEVNULL, stdout=log, stderr=subprocess.STDOUT,
                                   close_fds=True)
    (out / "pid").write_text(f"{process.pid}\n")
    print(f"Detached batch PID {process.pid}; root {ROOT}; slot {args.slot}")
    print(f"Output {out}; table {out / 'headroom.md'}; pictures {out / 'side-by-side.png'}")
    print(f"Completion {out / 'DONE'}; batch failure {out / 'FAILED'}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--slot", type=int, default=1)
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
