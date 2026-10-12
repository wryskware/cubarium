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
        for name, weather in (("day", "fixed:0.5,0,clear,0"), ("dusk", "fixed:0.74,0,clear,0"),
                              ("night", "fixed:0.95,0,clear,0"), ("rain", "fixed:0.5,0.8,shower,0"),
                              ("fog", "fixed:0.3,0.3,clear,0.8")):
            commands.append(base + ["--name", name, "--weather-preview", weather])
        commands.append(base + ["--name", "water-clock", "--weather-preview", "fixed:0.5,0,clear,0",
                                "--anim", "2", "--anim-hz", "1"])
        commands.append(base + ["--name", "pan128", "--weather-preview", "fixed:0.5,0,clear,0",
                                "--view-x", "128"])
    elif args.suite == "figures":
        # figure_capture uses the session loader's grouped world-N/ flora-N/
        # fauna-N filenames; the light_capture loader uses unnumbered filenames.
        # The grouping suffix is only for discovery: each layer retains its saved
        # tick and configuration unchanged inside the hardlinked envelope.
        session_state = fixture / "session-state"
        session_state.mkdir(exist_ok=True)
        for source_name, target_name in (("world.voxel", "world-0.voxel"),
                                         ("flora.bin", "flora-0.bin"),
                                         ("fauna.bin", "fauna-0.bin")):
            source, target = fixture / "state" / source_name, session_state / target_name
            if not target.exists():
                target.hardlink_to(source)
            elif not target.samefile(source):
                raise RuntimeError(f"figure fixture alias does not match {source}")
        commands.append([args.binary, "--config", str(cfg), "--save", str(session_state),
                         "--out", str(out / "figures"), "--px", "4", "--warm", "0",
                         "--seconds", "0.1", "--fps", "60", "--set", "world.width = 128",
                         "--set", "world.height = 64", "--set", "world.depth = 32"])
    elif args.suite == "present":
        source = Path(args.source).resolve()
        for size in ("800x200", "400x120", "100x40", "128x384"):
            commands.append([args.binary, str(source), size, str(out / f"window-fit-{size}.png")])
    else:
        base = [args.binary, "--gpu", "--lit", "--px", "4", "--anim-start", "1.25",
                "--weather", "fixed:0.5,0,clear,0"]
        commands.append(base + [str(out / "models-plants.png")])
        commands.append(base + ["--weather", "fixed:0.95,0,clear,0", str(out / "models-plants-glow.png")])
        commands.append(base + ["--textures", "assets/voxel-textures", str(out / "plants-textures.png")])
    records = []
    for command in commands:
        print(json.dumps(command), flush=True)
        subprocess.run(command, cwd=root, check=True, timeout=300)
        records.append(command)
        size = sum(p.stat().st_size for d in (out, fixture) for p in d.rglob("*") if p.is_file())
        if size > 100 * 1024**2:
            raise RuntimeError(f"parity artifacts exceeded 100 MiB: {size} bytes")
    images = {str(p.relative_to(out)): hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(out.rglob("*.png"))}
    (out / f"{args.suite}-commands.json").write_text(json.dumps(dict(commands=records, images=images), indent=2) + "\n")


def smoke(args):
    import re
    import shutil
    import tempfile
    fixture = Path(args.fixture).resolve()
    with tempfile.TemporaryDirectory(prefix="smoke-", dir=fixture) as temporary:
        state = Path(temporary)
        for source, target in (("world.voxel", "world-0.voxel"),
                               ("flora.bin", "flora-0.bin"), ("fauna.bin", "fauna-0.bin")):
            shutil.copyfile(fixture / "state" / source, state / target)
        config = (fixture / "terrarium.toml").read_text()
        config, count = re.subn(r"(?m)^px_per_voxel\s*=.*$", "px_per_voxel = 4", config, count=1)
        if count != 1:
            raise RuntimeError("smoke config must declare px_per_voxel")
        cfg = state / "smoke.toml"
        cfg.write_text(config)
        subprocess.run([args.binary, "voxel", "--config", str(cfg), "--load", str(state),
                        "--sink", "gpu", "--gpu-target", args.target, "--seconds", "0.1",
                        "--fps", "60", "--serve", "127.0.0.1:0", "--set", "world.width = 128",
                        "--set", "world.height = 64", "--set", "world.depth = 32"],
                       cwd=Path(args.tree).resolve(), check=True, timeout=300)


def gallery(path, rows, baseline, candidate):
    import base64
    import html
    images = []
    for row in rows:
        images.append(dict(row, ash="data:image/png;base64," + base64.b64encode((baseline / row["name"]).read_bytes()).decode(),
                           wgpu="data:image/png;base64," + base64.b64encode((candidate / row["name"]).read_bytes()).decode()))
    payload = json.dumps(images).replace("</", "<\\/")
    options = "".join(f'<option value="{i}">{html.escape(row["name"])}</option>' for i, row in enumerate(rows))
    document = '''<!doctype html><meta charset="utf-8"><title>Cubarium renderer parity</title>
<style>
body{background:#10131b;color:#eee;font:16px system-ui;margin:24px}select{max-width:90vw}
label{display:block;margin:16px 0}#stage{position:relative;display:inline-block;max-width:100%;border:1px solid #555}
img{display:block;image-rendering:pixelated;width:min(1024px,90vw);height:auto}#wgpu{position:absolute;inset:0}
input{width:min(1024px,90vw)}p{max-width:72ch}#stats{font-variant-numeric:tabular-nums}
</style>
<h1>Cubarium: ash → wgpu</h1><p>Identical saved world, time, camera and assets. Slide from ash (0%) to wgpu (100%) to inspect each pair.</p>
<label>Screenshot <select id="pick">OPTIONS</select></label><p id="stats"></p>
<label>Reveal wgpu <input id="reveal" type="range" min="0" max="100" value="50"></label>
<div id="stage"><img id="ash" alt="ash baseline"><img id="wgpu" alt="wgpu candidate"></div>
<script>
const shots=PAYLOAD, pick=document.querySelector('#pick'), reveal=document.querySelector('#reveal');
function mask(){document.querySelector('#wgpu').style.clipPath=`inset(0 ${100-reveal.value}% 0 0)`}
function show(){const s=shots[Number(pick.value)];document.querySelector('#ash').src=s.ash;document.querySelector('#wgpu').src=s.wgpu;
document.querySelector('#stats').textContent=`${s.size[0]} × ${s.size[1]} · ${s.changed_pixels} / ${s.pixels} changed pixels · maximum channel error ${s.max_channel_error}`;mask()}
pick.addEventListener('change',show);reveal.addEventListener('input',mask);show();
</script>'''.replace("OPTIONS", options).replace("PAYLOAD", payload)
    destination = Path(path)
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_text(document)


def compare(args):
    from PIL import Image, ImageChops
    baseline, candidate = Path(args.baseline), Path(args.candidate)
    left = {str(p.relative_to(baseline)): p for p in baseline.rglob("*.png")}
    right = {str(p.relative_to(candidate)): p for p in candidate.rglob("*.png")}
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
            (diff / name).parent.mkdir(parents=True, exist_ok=True)
            ImageChops.difference(a.convert("RGB"), b.convert("RGB")).save(diff / name)
    print(json.dumps(rows, indent=2))
    if args.report:
        report = Path(args.report)
        report.parent.mkdir(parents=True, exist_ok=True)
        report.write_text(json.dumps(dict(
            screenshots=len(rows),
            changed_pixels=sum(row["changed_pixels"] for row in rows),
            max_channel_error=max(row["max_channel_error"] for row in rows),
            images=rows,
        ), indent=2) + "\n")
    if args.gallery:
        gallery(args.gallery, rows, baseline, candidate)
    # A difference is evidence requiring review; no permissive taste threshold.
    return int(any(row["changed_pixels"] for row in rows))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    cap = sub.add_parser("capture")
    for key in ("binary", "tree", "fixture", "out"):
        cap.add_argument("--" + key, required=True)
    cap.add_argument("--suite", choices=("synthetic", "world", "specimens", "figures", "present"), required=True)
    cap.add_argument("--source", help="raster PNG input for the present suite")
    comp = sub.add_parser("compare")
    comp.add_argument("baseline")
    comp.add_argument("candidate")
    comp.add_argument("--diff")
    comp.add_argument("--gallery", help="standalone screenshot comparison HTML")
    comp.add_argument("--report", help="machine-readable comparison summary")
    sm = sub.add_parser("smoke", help="bounded startup/shutdown with a disposable world copy")
    for key in ("binary", "tree", "fixture"):
        sm.add_argument("--" + key, required=True)
    sm.add_argument("--target", choices=("headless", "window"), required=True)
    args = parser.parse_args()
    if args.command == "smoke":
        return smoke(args)
    return capture(args) if args.command == "capture" else compare(args)


if __name__ == "__main__":
    raise SystemExit(main())
