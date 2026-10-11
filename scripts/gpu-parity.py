#!/usr/bin/env python3
"""Bounded headless renderer matrix; launch capture through labd, not directly."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess


def capture(args):
    out = Path(args.out).resolve()
    out.mkdir(parents=True, exist_ok=True)
    root = Path(args.tree).resolve()
    fixture = Path(args.fixture).resolve()
    fixture.mkdir(parents=True, exist_ok=True)
    cfg = fixture / "terrarium.toml"
    if not cfg.exists():
        cfg.write_bytes((root / "config/desktop/terrarium.toml").read_bytes())
    commands = []
    if args.suite == "synthetic":
        for scale in (1, 2):
            commands.append([args.binary, "--target", "headless", "--frames", "1", "--at", "1.25",
                             "--seed", "1", "--ring", "320x64", "--scale", str(scale),
                             "--png", str(out / f"ring-scale{scale}.png")])
    elif args.suite == "world":
        base = [args.binary, "--config", str(cfg), "--seed", "1", "--ticks", "0",
                "--state", str(fixture / "state"), "--frames", "1", "--px", "4", "--textures",
                "--out", str(out), "--set", "world.width = 128", "--set", "world.height = 64",
                "--set", "world.depth = 32"]
        for name, weather in (("day", "fixed:0.5,0,0,0"), ("dusk", "fixed:0.74,0,0,0"),
                              ("night", "fixed:0.95,0,0,0"), ("rain", "fixed:0.5,0.8,0.8,0"),
                              ("fog", "fixed:0.3,0.3,0,0.8")):
            commands.append(base + ["--name", name, "--weather-preview", weather])
        commands.append(base + ["--name", "water-clock", "--weather-preview", "fixed:0.5,0,0,0",
                                "--anim", "2", "--anim-hz", "1"])
        commands.append(base + ["--name", "pan128", "--weather-preview", "fixed:0.5,0,0,0",
                                "--view-x", "128"])
    else:
        base = [args.binary, "--gpu", "--lit", "--px", "4", "--anim-start", "1.25",
                "--weather", "fixed:0.5,0,0,0"]
        commands.append(base + [str(out / "models-plants.png")])
        commands.append(base + ["--weather", "fixed:0.95,0,0,0", str(out / "models-plants-glow.png")])
        commands.append(base + ["--textures", "assets/voxel-textures", str(out / "plants-textures.png")])
    records = []
    for command in commands:
        print(json.dumps(command), flush=True)
        subprocess.run(command, cwd=root, check=True, timeout=300)
        records.append(command)
        size = sum(p.stat().st_size for d in (out, fixture) for p in d.rglob("*") if p.is_file())
        if size > 100 * 1024**2:
            raise RuntimeError(f"parity artifacts exceeded 100 MiB: {size} bytes")
    images = {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(out.glob("*.png"))}
    (out / f"{args.suite}-commands.json").write_text(json.dumps(dict(commands=records, images=images), indent=2) + "\n")


def compare(args):
    from PIL import Image, ImageChops
    baseline, candidate = Path(args.baseline), Path(args.candidate)
    left = {p.name: p for p in baseline.glob("*.png")}
    right = {p.name: p for p in candidate.glob("*.png")}
    if not left or left.keys() != right.keys():
        raise SystemExit(f"image set mismatch: missing={sorted(left.keys() - right.keys())}, extra={sorted(right.keys() - left.keys())}")
    rows = []
    for name in sorted(left):
        a, b = Image.open(left[name]).convert("RGBA"), Image.open(right[name]).convert("RGBA")
        if a.size != b.size:
            raise SystemExit(f"{name}: size mismatch {a.size} != {b.size}")
        pairs = list(zip(a.getdata(), b.getdata()))
        changed = sum(x != y for x, y in pairs)
        errors = [abs(x[c] - y[c]) for x, y in pairs for c in range(4)]
        rows.append(dict(name=name, size=a.size, changed_pixels=changed, pixels=len(pairs),
                         max_channel_error=max(errors), mean_channel_error=sum(errors) / len(errors)))
        if changed and args.diff:
            diff = Path(args.diff)
            diff.mkdir(parents=True, exist_ok=True)
            ImageChops.difference(a.convert("RGB"), b.convert("RGB")).save(diff / name)
    print(json.dumps(rows, indent=2))
    # A difference is evidence requiring review; no permissive taste threshold.
    return int(any(row["changed_pixels"] for row in rows))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    cap = sub.add_parser("capture")
    for key in ("binary", "tree", "fixture", "out"):
        cap.add_argument("--" + key, required=True)
    cap.add_argument("--suite", choices=("synthetic", "world", "specimens"), required=True)
    comp = sub.add_parser("compare")
    comp.add_argument("baseline")
    comp.add_argument("candidate")
    comp.add_argument("--diff")
    args = parser.parse_args()
    return capture(args) if args.command == "capture" else compare(args)


if __name__ == "__main__":
    raise SystemExit(main())
