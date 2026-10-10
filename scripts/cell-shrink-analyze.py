"""Round 3 cost table and streaming pixel counts (same metric as logs2/steps.py)."""
import csv
import json
from pathlib import Path
import re
import statistics as st
import subprocess
import sys

import numpy as np


def median(values):
    return st.median(values) if values else None


def cost(logs):
    walks = {}
    print("== Still state, 6 px, 300 frames, repack; three interleaved rounds")
    for side in ("main", "r3"):
        values, packs = [], []
        for round_ in (1, 2, 3):
            text = (logs / f"cost-{side}-{round_}.log").read_text()
            matches = re.findall(r"pack ([\d.]+) ms/tick, GPU upload [\d.]+ \+ slab walk ([\d.]+)", text)
            if len(matches) != 1:
                print(f"INCOMPLETE: {side} round {round_}: {len(matches)} timing records")
            for pack, walk in matches:
                packs.append(float(pack))
                values.append(float(walk))
        walks[side] = median(values)
        print(f"{side}: walk {values}, median {walks[side]}; pack median {median(packs)} ms/tick")
    if all(v is not None for v in walks.values()):
        delta = walks["r3"] - walks["main"]
        print(f"branch - main = {delta:+.3f} ms/frame; <=0.1 ms: {delta <= 0.1}")

    print("== Changing cells: B GPU ms/frame; A is off in every arm")
    for kind in ("day", "shed"):
        timing = {}
        for arm in ("on", "off"):
            runs = []
            for round_ in (1, 2, 3):
                path = logs / f"cost-{kind}-{arm}-{round_}.csv"
                if not path.exists():
                    print(f"INCOMPLETE: {path.name}")
                    continue
                with path.open() as f:
                    rows = list(csv.DictReader(f))
                if not rows:
                    print(f"INCOMPLETE: {path.name}: no frames")
                    continue
                def col(key, positive=False):
                    values = [float(r[key]) for r in rows]
                    return [v for v in values if v > 0] if positive else values
                active = [r for r in rows if float(r["b_changing"]) > 0]
                walk = median(col("b_walk_gpu_ms", True))
                plant = median(col("b_plant_gpu_ms", True))
                active_walk = median([float(r["b_walk_gpu_ms"]) for r in active if float(r["b_walk_gpu_ms"]) > 0])
                stages = col("b_stage_ms", True)
                stage = median(stages)
                changing = max(col("b_changing"))
                runs.append((walk, plant, active_walk))
                print(f"{kind} {arm} round {round_}: frames {len(rows)}, walk {walk}, plant {plant}, "
                      f"walk while changing {active_walk}, stage median/max {stage}/{max(stages, default=None)}, "
                      f"changing max {changing}, changing frames {len(active)}")
                if not active:
                    print(f"NO CHANGING CELLS: {path.name}")
            timing[arm] = [median([r[k] for r in runs if r[k] is not None]) for k in range(3)]
        for k, label in enumerate(("walk", "plant", "walk while changing")):
            on, off = timing["on"][k], timing["off"][k]
            if on is not None and off is not None:
                print(f"{kind} on-off {label}: {on - off:+.3f} ms/frame")


def steps(video, start, end):
    probe = subprocess.run(["ffprobe", "-v", "error", "-show_entries", "stream=width,height",
                            "-of", "csv=p=0", str(video)], capture_output=True, text=True, check=True)
    w, h = map(int, probe.stdout.split()[0].split(","))
    command = ["ffmpeg", "-v", "error", "-i", str(video), "-vf",
               f"select='between(n\\,{start}\\,{end})'", "-fps_mode", "vfr",
               "-f", "rawvideo", "-pix_fmt", "rgb24", "-"]
    half = (w - 8) // 2
    rows, before = [], None
    with subprocess.Popen(command, stdout=subprocess.PIPE) as proc:
        frame = start
        while True:
            raw = proc.stdout.read(w * h * 3)
            if not raw:
                break
            if len(raw) != w * h * 3:
                raise ValueError(f"partial frame {frame} in {video}")
            now = np.frombuffer(raw, np.uint8).reshape(h, w, 3).astype(np.int16)
            if before is not None:
                delta = np.abs(now - before).max(axis=2)
                a, b = delta[:, :half], delta[:, w-half:]
                rows.append((frame, *(int((side > threshold).sum())
                                      for side in (a, b) for threshold in (150, 60, 20))))
            before = now
            frame += 1
        if proc.wait() != 0:
            raise RuntimeError(f"ffmpeg failed for {video}")
    if len(rows) != end - start:
        raise ValueError(f"expected {end-start} steps, got {len(rows)} in {video}")
    return rows


def evidence(logs, art):
    for kind, name, start, end in (
        ("shed", "shed-close6x-today-fixed-x1.mp4", 330, 480),
        ("day", "day-today-fixed-x1-close2x.mp4", 0, 1799),
    ):
        rows = steps(art / name, start, end)
        reference = steps(art.parent / "round2" / name, start, end)
        with (logs / f"{kind}-steps.csv").open("w") as f:
            writer = csv.writer(f)
            writer.writerow(("frame", "a150", "a60", "a20", "b150", "b60", "b20"))
            writer.writerows(rows)
        with (logs / f"{kind}-steps.txt").open("w") as f:
            for r in rows:
                if r[3] > 200 or r[6] > 200 or r[0] == 342:
                    f.write("frame %d A>150 %d A>60 %d A>20 %d | B>150 %d B>60 %d B>20 %d\n" % r)
        print(f"== {kind}: threshold counts, old fixed versus new fixed")
        for k, label in ((5, ">60"), (6, ">20")):
            old_peak = max(reference, key=lambda r: r[k])
            peak = max(rows, key=lambda r: r[k])
            change = max(zip(rows, reference), key=lambda pair: pair[0][k] - pair[1][k])
            print(f"B{label} peak old {old_peak[k]} @ {old_peak[0]}, new {peak[k]} @ {peak[0]}; "
                  f"largest increase {change[0][k]-change[1][k]} @ {change[0][0]}")
        if kind == "shed":
            r = next(r for r in rows if r[0] == 342)
            old = next(r for r in reference if r[0] == 342)
            print(f"frame 342: today A>20 {r[3]}, fixed B>20 {r[6]}, old fixed {old[6]}; "
                  f"<=2286: {r[6] <= 2286}; A/B>60 {r[2]}/{r[5]}")


def main():
    logs, art = map(Path, sys.argv[1:3])
    with (logs / "commands.tsv").open() as f:
        failures = [r for r in csv.DictReader(f, delimiter="\t") if r["exit"] != "0"]
    print("command failures:", json.dumps(failures))
    cost(logs)
    evidence(logs, art)


if __name__ == "__main__":
    main()
