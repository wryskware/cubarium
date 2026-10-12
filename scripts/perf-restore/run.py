#!/usr/bin/env python3
"""Pinned, serial validation and benchmark chain. One outer timeout owns the job."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import traceback

SSH = ["ssh", "-o", "BatchMode=yes", "-o", "IdentityAgent=none", "-o", "HostKeyAlias=voidrunner",
       "-o", "ConnectTimeout=15", "wrysk@192.168.68.57"]


def command(argv, cwd, log, env=None):
    print(json.dumps(dict(argv=list(map(str, argv)), cwd=str(cwd), log=str(log))), flush=True)
    log.parent.mkdir(parents=True, exist_ok=True)
    with log.open("w") as output:
        try:
            return subprocess.run(list(map(str, argv)), cwd=cwd, env=env,
                                  stdout=output, stderr=subprocess.STDOUT).returncode
        except OSError as error:
            output.write(str(error) + "\n")
            return 127


def snapshot(repo, commit, destination):
    """Source snapshots only; the authorized worktree's one target cache is reused."""
    destination.mkdir(parents=True, exist_ok=True)
    available = subprocess.check_output(["git", "ls-tree", "--name-only", commit], cwd=repo, text=True).splitlines()
    paths = [p for p in available if p not in ("assets", "art")]
    producer = subprocess.Popen(["git", "archive", commit, *paths], cwd=repo, stdout=subprocess.PIPE)
    consumer = subprocess.run(["tar", "-x", "-C", str(destination)], stdin=producer.stdout)
    producer.stdout.close()
    if producer.wait() or consumer.returncode:
        raise RuntimeError(f"source snapshot failed: {commit}")
    # Runtime art stays the ordinary assets; no frozen asset/build copy is retained.
    (destination / "assets").symlink_to(repo / "assets", target_is_directory=True)
    (destination / "art").symlink_to(repo / "art", target_is_directory=True)
    return destination


def build_examples(tree, target, logs, env):
    commands = [
        ["cargo", "build", "--release", "-j", "8", "-p", "cubarium-gpu", "--example", "synthetic", "--example", "window_fit"],
        ["cargo", "build", "--release", "-j", "8", "-p", "cubarium", "--example", "light_capture",
         "--example", "voxel_specimens", "--example", "figure_capture"],
    ]
    statuses = [command(argv, tree, logs / f"examples-{i}.log", env) for i, argv in enumerate(commands)]
    return all(rc == 0 for rc in statuses)


def validate(args, candidate, env, stages):
    out, repo = args.out, args.repo
    baseline = snapshot(repo, "ce035f3a", out / "source-port")
    # Run both suites straight through, including all test failures.
    stages["gpu-tests"] = command(["cargo", "nextest", "run", "-p", "cubarium-gpu", "--features", "gpu-counters",
                                    "--no-fail-fast", "--test-threads", "8"], candidate, out / "gpu-tests.log", env)
    stages["workspace-tests"] = command(["cargo", "nextest", "run", "--workspace", "--exclude", "cubarium-gpu",
                                          "--features", "cubarium/gpu-counters", "--no-fail-fast", "--test-threads", "8"],
                                         candidate, out / "workspace-tests.log", env)
    stages["all-target-check"] = command(["cargo", "check", "--workspace", "--all-targets", "-j", "8"],
                                         candidate, out / "all-target-check.log", env)
    fixture = out / "fixture"
    shutil.copytree(args.fixture, fixture)
    inputs = {str(p.relative_to(fixture)): hashlib.sha256(p.read_bytes()).hexdigest()
              for p in fixture.rglob("*") if p.is_file()}
    (out / "fixture-hashes.json").write_text(json.dumps(inputs, indent=2) + "\n")
    for arm, tree, commit in (("baseline", baseline, "ce035f3a"), ("candidate", candidate, args.commit)):
        arm_env = dict(env, CUBARIUM_BUILD_REV=commit)
        stages[f"build-{arm}"] = 0 if build_examples(tree, args.target, out / "build" / arm, arm_env) else 1
        for suite in ("synthetic", "world", "specimens", "figures", "present"):
            spec = json.loads((candidate / "scripts/perf-restore" / f"gpu-parity-{suite}.json").read_text())
            values = dict(runner_tree=str(candidate), tree=str(tree), fixture=str(fixture), out=str(out), arm=arm,
                          commit=commit, bin=str(args.target / "release/examples" / spec["binary"]["example"]))
            argv = [a.format(**values) for a in spec["argv"]]
            stages[f"capture-{arm}-{suite}"] = command(argv, tree, out / "capture" / f"{arm}-{suite}.log", env)
    stages["parity"] = command([sys.executable, candidate / "scripts/gpu-parity.py", "compare", out / "parity/baseline",
                                out / "parity/candidate", "--report", out / "comparison.json"], repo, out / "comparator.log", env)
    # Raw nextest summaries and the strict comparator are the evidence, not thresholds.
    lines = []
    for name in ("gpu-tests", "workspace-tests"):
        lines += [f"{name}: {line.strip()}" for line in (out / f"{name}.log").read_text(errors="replace").splitlines()
                  if "Summary" in line or "tests run:" in line]
    if (out / "comparison.json").exists():
        comparison = json.loads((out / "comparison.json").read_text())
        lines.append("parity: " + json.dumps({k: v for k, v in comparison.items() if k != "images"}))
    (out / "results.txt").write_text("\n".join(lines) + "\n")


def bench_harness(source, destination):
    """Keep baseline scenarios/measurements; finish the whole trimmed matrix."""
    text = source.read_text()
    text = text.replace('        (root / "FAIL").write_text("proof failed\\n" + "\\n".join(bad) + "\\n")\n', '')
    text = text.replace('        return 1\n    print("proof passed", flush=True)',
                        '        print("continuing the requested matrix after proof failure", flush=True)\n    print("proof complete", flush=True)')
    text = text.replace('        return do_run(ns)', '        result = do_run(ns)\n        if not result.get("ok"): a._perf_failed = True\n        return result')
    text = text.replace('        return dict(ok=False, problem=repr(e))', '        a._perf_failed = True\n        return dict(ok=False, problem=repr(e))')
    text = text.replace('def matrix(a):\n', 'def matrix(a):\n    a._perf_failed = False\n')
    text = text.replace('60, 20, cold=True', '60, a.seconds, cold=True').replace('60, 20, warm_of_cold=True', '60, a.seconds, warm_of_cold=True')
    text = text.replace('    (root / "DONE").write_text(time.strftime("%F %T") + "\\n")\n    return 0',
                        '    failures = [p for p in (root / "runs").glob("*/result.json") if not json.loads(p.read_text()).get("ok")]\n'
                        '    failed = bool(bad or failures or a._perf_failed)\n'
                        '    (root / ("FAIL" if failed else "DONE")).write_text(time.strftime("%F %T") + "\\n")\n'
                        '    return int(failed)')
    text = text.replace('    meta = dict(arm=a.arm,', '    argv = [sys.executable, str(Path(a.cwd) / "scripts/run-study.py"), "--slot", str((a.rep - 1) * 2 + int(a.arm == "B")), "--"] + argv\n    meta = dict(arm=a.arm,')
    # Private Xvfb contains the remote window arm. The benchmark continues to
    # measure the actual binary rather than the display wrapper's shell.
    text = text.replace('        subprocess.run(sd + argv, check=True)', '        argv = [str(Path(a.cwd) / "scripts/hidden.sh")] + argv\n        subprocess.run(sd + argv, check=True)')
    text = text.replace('        s = proc_sample(pid) if pid else None', '        sample_pid = binary_pid(pid, a.bin) if a.target == "window" else pid\n        s = proc_sample(sample_pid) if sample_pid else None')
    text = text.replace('nvidia_mem(pid, a.gpu)', 'nvidia_mem(sample_pid, a.gpu)')
    helper = '''
def binary_pid(root, binary):
    pending = [root]
    while pending:
        pid = pending.pop()
        try:
            argv = Path(f"/proc/{pid}/cmdline").read_bytes().split(b"\\0")
            if argv and os.fsdecode(argv[0]) == binary:
                return pid
            children = Path(f"/proc/{pid}/task/{pid}/children").read_text().split()
            pending.extend(map(int, children))
        except (OSError, ValueError):
            pass
    return root

'''
    text = text.replace('# ---------------------------------------------------------------- one run', helper + '# ---------------------------------------------------------------- one run')
    destination.write_text(text)


def benchmark(args, candidate, env, stages):
    out, repo = args.out, args.repo
    ash = snapshot(repo, "953e688f", out / "source-ash")
    patch = repo / "design/handoffs/wgpu-review-2026-10-11/bench-frames.diff"
    bins = out / "binaries"
    bins.mkdir()
    for arm, tree in (("ash", ash), ("branch", candidate)):
        # The patch is identical in both arms and contains only frame diagnostics.
        stages[f"bench-patch-{arm}"] = command(["patch", "-p1", "--batch", "--forward", "-i", patch], tree, out / f"patch-{arm}.log", env)
        build_env = dict(env, CUBARIUM_BUILD_REV=("953e688f" if arm == "ash" else args.commit) + "+bench")
        stages[f"bench-build-{arm}"] = command(["cargo", "build", "--release", "-p", "cubarium", "--bin", "cubarium", "-j", "8"],
                                               tree, out / f"bench-build-{arm}.log", build_env)
        source = args.target / "release/cubarium"
        if stages[f"bench-build-{arm}"] == 0:
            shutil.copyfile(source, bins / arm)
            (bins / arm).chmod(0o755)
    harness = out / "bench.py"
    bench_harness(args.bench_harness, harness)
    config = out / "bench.toml"
    config.write_text("\n".join(line for line in (ash / "config/voidrunner/terrarium-d96.toml").read_text().splitlines()
                               if not line.startswith("serve = ")) + "\n")
    metadata = dict(ash="953e688f", branch=args.commit, patch_sha256=hashlib.sha256(patch.read_bytes()).hexdigest(),
                    repeats=2, seconds=45, desktop_gpu="5090", desktop_target="headless")
    (out / "benchmark-source.json").write_text(json.dumps(metadata, indent=2) + "\n")
    # This is the sole baseline gate. No remote files, builds or runs are touched
    # unless its DONE/FAIL marker is present. There is no polling or waiting loop.
    with (out / "voidrunner-gate.log").open("w") as log:
        gate = subprocess.run(SSH + ["bash", "-s"], input='test -f "$HOME/bench-wgpu/DONE" || test -f "$HOME/bench-wgpu/FAIL"\n',
                              text=True, stdout=log, stderr=subprocess.STDOUT).returncode
    stages["voidrunner-baseline-gate"] = gate
    if gate == 0:
        with (out / "baseline-voidrunner-summary.md").open("w") as log:
            subprocess.run(SSH + ["bash", "-s"], input='cat "$HOME/bench-wgpu/out/summary.md"\n', text=True, stdout=log, stderr=subprocess.STDOUT)
    # Launch remote work before the desktop matrix; both chains have their own
    # single long timeout and final marker, and never open a desktop window.
    if gate == 0:
        stages["voidrunner-launch"] = launch_remote(args, ash, candidate, bins, harness, config, env)
    else:
        (out / "voidrunner-FAIL").write_text("baseline marker absent; remote rerun was not launched\n")
    desktop = out / "desktop"
    stages["desktop-benchmark"] = command([sys.executable, harness, "matrix", "--root", desktop, "--host", "desktop",
                                           "--bin-a", bins / "ash", "--bin-b", bins / "branch", "--tree-a", ash,
                                           "--tree-b", candidate, "--world", args.world, "--config", config,
                                           "--expect-gpu", "5090", "--gpu", "5090", "--reps", "2", "--cold-reps", "2", "--seconds", "45"],
                                          repo, out / "desktop-benchmark.log", env)


def launch_remote(args, ash, candidate, bins, harness, config, env):
    """Upload task-owned source, never modify an existing remote checkout."""
    import shlex
    remote = args.remote_out
    archive = args.out / "remote-source.tar"
    # Sources are small; ordinary runtime assets are transferred once. A single
    # normal remote target cache builds both arms, then only two binaries remain.
    runtime = args.out / "assets"
    runtime.symlink_to(args.repo / "assets", target_is_directory=True)
    with archive.open("wb") as stream:
        rc = subprocess.run(["tar", "--exclude=source-ash/assets", "--exclude=source-branch/assets", "--exclude=source-ash/art", "--exclude=source-branch/art", "-chf", "-", "-C", str(args.out), "source-ash", "source-branch", "assets", "bench.py", "bench.toml", "benchmark-source.json"], stdout=stream).returncode
    if rc:
        return rc
    transfer = subprocess.run(["rsync", "-a", "-e", " ".join(SSH[:-1]), str(archive),
                               f"{SSH[-1]}:{remote}.tar"]).returncode
    if transfer:
        return transfer
    runner = f'''set -uo pipefail
PERF_DIR={shlex.quote(remote)}
mkdir -p "$PERF_DIR"
tar -xf "$PERF_DIR.tar" -C "$PERF_DIR"
rm "$PERF_DIR.tar"
ln -s ../assets "$PERF_DIR/source-ash/assets"
ln -s ../assets "$PERF_DIR/source-branch/assets"
cat > "$PERF_DIR/run.sh" <<'CHAIN'
#!/usr/bin/env bash
set -uo pipefail
PERF_DIR=$(cd -- "$(dirname -- "$0")" && pwd)
export CLAUDECODE=1 CARGO_BUILD_JOBS=8 CARGO_TARGET_DIR="$HOME/bench-wgpu/target"
rc=0
mkdir -p "$PERF_DIR/binaries"
for arm in ash branch; do
  if [ "$arm" = ash ]; then
    export CUBARIUM_BUILD_REV=953e688f+bench
  else
    export CUBARIUM_BUILD_REV=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["branch"] + "+bench")' "$PERF_DIR/benchmark-source.json")
  fi
  (cd "$PERF_DIR/source-$arm" && cargo build --release -j 8 -p cubarium --bin cubarium) > "$PERF_DIR/build-$arm.log" 2>&1
  code=$?; [ "$code" -eq 0 ] || rc=1
  if [ "$code" -eq 0 ]; then cp "$CARGO_TARGET_DIR/release/cubarium" "$PERF_DIR/binaries/$arm"; fi
done
python3 "$PERF_DIR/bench.py" matrix --root "$PERF_DIR/out" --host voidrunner \\
  --bin-a "$PERF_DIR/binaries/ash" --bin-b "$PERF_DIR/binaries/branch" \\
  --tree-a "$PERF_DIR/source-ash" --tree-b "$PERF_DIR/source-branch" \\
  --world "$HOME/bench-wgpu/world" --config "$PERF_DIR/bench.toml" \\
  --expect-gpu Radeon --reps 2 --cold-reps 2 --seconds 45 --window
code=$?; [ "$code" -eq 0 ] || rc=1
rm -rf "$PERF_DIR/source-ash" "$PERF_DIR/source-branch" "$PERF_DIR/assets"
[ "$rc" -eq 0 ] && echo complete > "$PERF_DIR/DONE" || echo "chain exit $rc" > "$PERF_DIR/FAIL"
exit "$rc"
CHAIN
chmod +x "$PERF_DIR/run.sh"
nohup chrt -i 0 nice -n 19 bash -c 'timeout 21600 "$1/run.sh"; rc=$?; [ "$rc" -eq 0 ] || echo "outer timeout/chain exit $rc" > "$1/FAIL"' _ "$PERF_DIR" > "$PERF_DIR/chain.log" 2>&1 < /dev/null &
echo "$!" > "$PERF_DIR/launcher.pid"
echo "launched $PERF_DIR"
'''
    with (args.out / "voidrunner-launch.log").open("w") as log:
        result = subprocess.run(SSH + ["bash", "-s"], input=runner, text=True, stdout=log, stderr=subprocess.STDOUT)
    archive.unlink(missing_ok=True)
    runtime.unlink(missing_ok=True)
    return result.returncode


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, required=True)
    parser.add_argument("--commit", required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--target", type=Path, required=True)
    parser.add_argument("--fixture", type=Path, required=True)
    parser.add_argument("--world", type=Path, required=True)
    parser.add_argument("--bench-harness", type=Path, required=True)
    parser.add_argument("--remote-out", required=True)
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    stages = {}
    env = dict(os.environ, CARGO_BUILD_JOBS="8", CARGO_TARGET_DIR=str(args.target), CUBARIUM_GPU="5090", CUBARIUM_BUILD_REV=args.commit)
    env["CUBARIUM_PIPELINE_CACHE"] = str(args.out / "pipeline-cache")
    for name in ("DISPLAY", "WAYLAND_DISPLAY", "WAYLAND_SOCKET"):
        env.pop(name, None)
    candidate = snapshot(args.repo, args.commit, args.out / "source-branch")
    for stage in (validate, benchmark):
        try:
            stage(args, candidate, env, stages)
        except Exception:
            stages[stage.__name__] = 1
            (args.out / f"{stage.__name__}-exception.log").write_text(traceback.format_exc())
    for source in ("source-port", "source-ash", "source-branch"):
        shutil.rmtree(args.out / source, ignore_errors=True)
    (args.out / "stages.json").write_text(json.dumps(stages, indent=2) + "\n")
    failed = any(stages.values())
    (args.out / ("FAIL" if failed else "DONE")).write_text(json.dumps(stages) + "\n")
    return int(failed)


if __name__ == "__main__":
    raise SystemExit(main())
