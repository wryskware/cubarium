#!/usr/bin/env python3
"""Enqueue a saved API-format workflow on the local ComfyUI and fetch the result.

Loopback only (127.0.0.1:8188 by default). Standard library only.

    comfy_run.py WORKFLOW.json --out DIR/0003.png \
        --set 4.inputs.prompt=@prompt.txt --set 6.inputs.seed=12345

`--set NODE.inputs.KEY=VALUE` overrides one widget; `=@path` reads the value from a
file; numbers and booleans are coerced when the JSON already holds that type.
Prints one line: `<out> | <WxH> | seed=<n> | <seconds>s`.
"""

from __future__ import annotations

import argparse
import json
import pathlib
import shutil
import sys
import time
import urllib.parse
import urllib.request

DEFAULT_HOST = "http://127.0.0.1:8188"


def _post(host: str, route: str, payload: dict) -> dict:
    data = json.dumps(payload).encode()
    req = urllib.request.Request(
        host + route, data=data, headers={"Content-Type": "application/json"}
    )
    try:
        with urllib.request.urlopen(req, timeout=60) as r:
            return json.loads(r.read())
    except urllib.error.HTTPError as e:  # ComfyUI puts the real reason in the body
        print(e.read().decode()[:4000], file=sys.stderr)
        raise


def _get(host: str, route: str) -> dict:
    with urllib.request.urlopen(host + route, timeout=60) as r:
        return json.loads(r.read())


def apply_set(wf: dict, spec: str) -> None:
    path, _, value = spec.partition("=")
    if value.startswith("@"):
        value = pathlib.Path(value[1:]).read_text()
    node = wf
    keys = path.split(".")
    for k in keys[:-1]:
        node = node[k]
    last = keys[-1]
    old = node.get(last)
    if isinstance(old, bool):
        node[last] = value.lower() in ("1", "true", "yes")
    elif isinstance(old, int) and not isinstance(old, bool):
        node[last] = int(value)
    elif isinstance(old, float):
        node[last] = float(value)
    else:
        node[last] = value


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("workflow")
    ap.add_argument("--out", required=True, help="where to write the first output image")
    ap.add_argument("--set", action="append", default=[], dest="sets")
    ap.add_argument("--host", default=DEFAULT_HOST)
    ap.add_argument("--timeout", type=float, default=900.0)
    a = ap.parse_args()

    wf = json.loads(pathlib.Path(a.workflow).read_text())
    for s in a.sets:
        apply_set(wf, s)

    t0 = time.time()
    resp = _post(a.host, "/prompt", {"prompt": wf})
    pid = resp["prompt_id"]
    hist: dict = {}
    while time.time() - t0 < a.timeout:
        time.sleep(2.0)
        hist = _get(a.host, f"/history/{pid}").get(pid, {})
        if hist.get("status", {}).get("completed"):
            break
        if hist.get("status", {}).get("status_str") == "error":
            print(json.dumps(hist.get("status"), indent=2)[:2000], file=sys.stderr)
            return 2
    else:
        print(f"timeout after {a.timeout}s", file=sys.stderr)
        return 3

    images = [
        im
        for node in hist.get("outputs", {}).values()
        for im in node.get("images", [])
        if im.get("type") == "output"
    ]
    if not images:
        print("no output images", file=sys.stderr)
        return 4
    im = images[0]
    q = urllib.parse.urlencode(
        {"filename": im["filename"], "subfolder": im.get("subfolder", ""), "type": "output"}
    )
    out = pathlib.Path(a.out)
    out.parent.mkdir(parents=True, exist_ok=True)
    with urllib.request.urlopen(f"{a.host}/view?{q}", timeout=120) as r, out.open("wb") as f:
        shutil.copyfileobj(r, f)

    try:
        from PIL import Image

        size = "x".join(str(v) for v in Image.open(out).size)
    except Exception:
        size = "?"
    seed = next(
        (
            n["inputs"]["seed"]
            for n in wf.values()
            if isinstance(n.get("inputs"), dict) and "seed" in n["inputs"]
        ),
        "?",
    )
    print(f"{out} | {size} | seed={seed} | {time.time() - t0:.1f}s")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
