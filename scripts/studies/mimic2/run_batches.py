#!/usr/bin/env python3
"""Desktop MIMIC-2: one uninterrupted staged invocation per arm; zeros are results."""
import argparse
import concurrent.futures as cf
from pathlib import Path
import subprocess
import time

COMMON = ["config=config/desktop/terrarium.toml", "set=world.climate.dry_season_len_days=0"]
SWITCHES = {
    "main": ["set=fauna.founders.lanternjaw.lure_daylight=false", "set=fauna.founders.seedporter.catch_bellwings=false"],
    "mimic": ["set=fauna.founders.lanternjaw.lure_daylight=true", "set=fauna.founders.seedporter.catch_bellwings=false"],
    "both": ["set=fauna.founders.lanternjaw.lure_daylight=true", "set=fauna.founders.seedporter.catch_bellwings=true"],
}


def command(runner, binary, slot, args, timeout_s):
    return ["timeout", str(timeout_s), "chrt", "-i", "0", "nice", "-n", "19",
            "python3", str(runner), "--slot", str(slot), "--", str(binary)] + args


def run_simulation(slot, args, out, err, *, source, binary, runner, timeout_s):
    with out.open("w") as stdout, err.open("w") as stderr:
        return subprocess.run(command(runner, binary, slot, args, timeout_s),
                              cwd=source, stdout=stdout, stderr=stderr).returncode


def founding(root, source, binary, runner, seed, timeout_s):
    destination = root / "found" / f"s{seed}"
    rc = run_simulation(seed - 1, ["found", f"seed={seed}", f"save={destination}",
                        "minutes=0", "lanternjaws=4", "threads=1"] + COMMON + SWITCHES["main"],
                        root / "runs" / f"s{seed}.found", root / "runs" / f"s{seed}.found.err",
                        source=source, binary=binary, runner=runner, timeout_s=timeout_s)
    if rc or not (destination / "m0" / "fauna.bin").exists():
        raise RuntimeError(f"founding s{seed} exit {rc}; see s{seed}.found.err")
    return destination / "m0"


def one(root, binary, runner, seed, arm, slot, found, timeout_s, source=None):
    stage = root / "runs" / f"{arm}-s{seed}"
    args = ["run", f"load={found}", f"arm={arm}", "hours=6", "every=5", "threads=1",
            f"run={stage}", "segment_min=60", "check_min=10", "keep=2"] + COMMON + SWITCHES[arm]
    rc = run_simulation(slot, args, root / "runs" / f"{stage.name}.out",
                        root / "runs" / f"{stage.name}.err", source=source,
                        binary=binary, runner=runner, timeout_s=timeout_s)
    if (stage / "FAILED").exists():
        raise RuntimeError((stage / "FAILED").read_text().strip())
    if rc or not (stage / "DONE").exists():
        raise RuntimeError(f"exit {rc}; staged DONE missing")
    print(f"{stage.name}: complete (zeros retained)", flush=True)


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--root", type=Path, required=True)
    p.add_argument("--source", type=Path, required=True)
    p.add_argument("--binary", type=Path, required=True)
    p.add_argument("--runner", type=Path, required=True)
    p.add_argument("--wait-for", type=Path, required=True)
    p.add_argument("--timeout-s", type=int, default=30000)
    o = p.parse_args()
    while not o.wait_for.exists():
        print(f"waiting for {o.wait_for}", flush=True)
        time.sleep(300)
    (o.root / "runs").mkdir(parents=True, exist_ok=True)
    errors = []
    starts = {}
    def failed(seed, arm, error):
        stage = o.root / "runs" / f"{arm}-s{seed}"
        stage.mkdir(exist_ok=True)
        if not (stage / "FAILED").exists():
            (stage / "FAILED").write_text(str(error) + "\n")
        errors.append(f"{stage.name}: {error}")
        print(errors[-1], flush=True)
    with cf.ThreadPoolExecutor(max_workers=4) as pool:
        pending = {pool.submit(founding, o.root, o.source, o.binary, o.runner, s, o.timeout_s): s for s in range(1, 5)}
        for future in cf.as_completed(pending):
            seed = pending[future]
            try:
                starts[seed] = future.result()
            except Exception as error:
                for arm in SWITCHES:
                    failed(seed, arm, error)
    with cf.ThreadPoolExecutor(max_workers=12) as pool:
        pending = {pool.submit(one, o.root, o.binary, o.runner, seed, arm, (seed - 1)*3+i,
                               starts[seed], o.timeout_s, o.source): (seed, arm)
                   for seed in starts for i, arm in enumerate(SWITCHES)}
        for future in cf.as_completed(pending):
            try:
                future.result()
            except Exception as error:
                failed(*pending[future], error)
    if errors:
        (o.root / "runs" / "errors").write_text("\n".join(errors) + "\n")
        (o.root / "FAILED").write_text("one or more runs failed; see runs/errors\n")
        raise SystemExit(1)


if __name__ == "__main__":
    main()
