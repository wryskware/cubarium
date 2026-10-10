#!/usr/bin/env python3
"""Run a cargo command in a snapshot of one exact commit (standalone, stdlib only).

    snaprun.py --repo R --commit SHA --trees DIR [--seed TARGET ...] [--profile release]
               [--source-tar TAR] [--artifact NAME=REL ...] --out OUT -- cargo build ...

Builds never run in a live, shared checkout. The snapshot is a detached git worktree at
SHA under --trees (or an extracted `git archive` on a host without the repo). Its target/
is seeded once from the newest candidate profile dir with `cp -a --reflink=always`; when
reflink fails it builds cold, never a full copy. Nothing goes under /tmp (tmpfs).
Artifacts are copied to OUT/artifact/NAME with meta.json beside them.
"""

import argparse
import fcntl
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import time


def sha256(path):
    digest = hashlib.sha256()
    with open(path, "rb") as stream:
        for block in iter(lambda: stream.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def on_tmpfs(path):
    path = str(Path(path).absolute())
    best, kind = "", ""
    for line in Path("/proc/mounts").read_text().splitlines():
        parts = line.split()
        if path == parts[1] or path.startswith(parts[1].rstrip("/") + "/"):
            if len(parts[1]) >= len(best):
                best, kind = parts[1], parts[2]
    return kind in ("tmpfs", "ramfs") or path.startswith("/tmp/")


def git(tree, *args):
    return subprocess.run(["git", "-C", str(tree), *args], check=True, capture_output=True, text=True,
                          stdin=subprocess.DEVNULL).stdout.strip()


def snapshot(args, tree):
    if tree.exists():
        return
    partial = tree.with_name(tree.name + ".partial")
    if args.source_tar:
        shutil.rmtree(partial, ignore_errors=True)
        partial.mkdir(parents=True)
        subprocess.run(["tar", "-x", "-f", args.source_tar, "-C", str(partial)], check=True)
        partial.rename(tree)
        return
    subprocess.run(["git", "-C", args.repo, "worktree", "prune"], capture_output=True)
    git(args.repo, "worktree", "add", "--detach", "--quiet", str(tree), args.commit)
    if git(tree, "rev-parse", "HEAD") != args.commit:
        raise SystemExit(f"snapshot {tree} is not at {args.commit}")


def seed(args, tree, log):
    target = tree / "target"
    if (target / args.profile).exists():
        return f"reused {target}"
    candidates = [Path(s).expanduser() / args.profile for s in args.seed]
    candidates += [t / "target" / args.profile for t in Path(args.trees).glob("*") if t != tree and t.is_dir()]
    candidates = [c for c in candidates if c.is_dir() and not on_tmpfs(c)]
    if not candidates:
        return "cold: no seed target"
    source = max(candidates, key=lambda c: c.stat().st_mtime)
    (target / args.profile).parent.mkdir(parents=True, exist_ok=True)
    started = time.monotonic()
    done = subprocess.run(["cp", "-a", "--reflink=always", str(source), str(target / args.profile)],
                          capture_output=True, text=True, stdin=subprocess.DEVNULL)
    if done.returncode:
        shutil.rmtree(target / args.profile, ignore_errors=True)
        log.write(f"reflink seed from {source} failed ({done.stderr.strip()[-200:]}); building cold\n")
        return f"cold: reflink from {source} failed"
    return f"seeded from {source} in {time.monotonic() - started:.1f}s"


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--repo")
    parser.add_argument("--commit", required=True)
    parser.add_argument("--trees", required=True)
    parser.add_argument("--seed", action="append", default=[])
    parser.add_argument("--profile", default="release")
    parser.add_argument("--source-tar")
    parser.add_argument("--artifact", action="append", default=[])
    parser.add_argument("--out", required=True)
    parser.add_argument("command", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    command = args.command[1:] if args.command[:1] == ["--"] else args.command
    trees, out = Path(args.trees).expanduser(), Path(args.out)
    for path in (trees, out):
        if on_tmpfs(path):
            raise SystemExit(f"refusing tmpfs path {path}")
    trees.mkdir(parents=True, exist_ok=True)
    out.mkdir(parents=True, exist_ok=True)
    tree = trees / args.commit[:16]
    log = sys.stderr
    with open(trees / f"{args.commit[:16]}.lock", "w") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        snapshot(args, tree)
        seeded = seed(args, tree, log)
    log.write(f"snapshot {tree} at {args.commit}; {seeded}\n")
    log.flush()
    os.utime(tree)
    env = os.environ | {"CARGO_TARGET_DIR": str(tree / "target")}
    rc = subprocess.run(command, cwd=tree, env=env, stdin=subprocess.DEVNULL).returncode
    if args.source_tar is None and git(tree, "rev-parse", "HEAD") != args.commit:
        raise SystemExit("snapshot HEAD moved during the build")
    if rc:
        return rc
    meta = dict(commit=args.commit, seed=seeded, tree=str(tree), command=command, files={})
    for item in args.artifact:
        name, _, relative = item.partition("=")
        source = tree / "target" / relative
        destination = out / "artifact" / name
        destination.parent.mkdir(parents=True, exist_ok=True)
        temporary = destination.with_name(f".{name}.tmp")
        subprocess.run(["cp", "--reflink=auto", str(source), str(temporary)], check=True)
        temporary.replace(destination)
        meta["files"][name] = sha256(destination)
    if args.artifact:
        (out / "artifact" / "meta.json").write_text(json.dumps(meta, indent=1) + "\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
