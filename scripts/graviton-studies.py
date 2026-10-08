#!/usr/bin/env python3
"""Offline plans/local kits, then explicitly authorized independent Graviton jobs.

plan and kit never contact AWS or a host. up/down delegate to aws-graviton.sh;
ship/run/fetch/restore use only explicitly supplied SSH addresses. Stdlib only.
"""
import argparse
import fcntl
import hashlib
import json
import math
import os
from pathlib import Path
import re
import resource
import shlex
import shutil
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
REMOTE = "/home/ec2-user/cubarium-kit"
PYTHON = "python3.11"
TOOLS = ["batch", "score", "study_contract", "vitals_summary", "eco_search"]
LIMIT = 64 * 1024**3


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def read_spec(path):
    spec = json.loads(path.read_text())
    jobs = spec["jobs"]
    if not jobs or len(jobs) > 2:
        raise ValueError("spec needs 1 or 2 independent host jobs (current quota)")
    for job in jobs:
        if not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_-]*", job["name"]):
            raise ValueError("job names must be safe single path components")
        if job["kind"] not in ("batch", "eco", "live"):
            raise ValueError("job kind must be batch, eco or live")
        if not isinstance(job["args"], list) or any(not isinstance(a, str) for a in job["args"]):
            raise ValueError("job args must be a list of strings")
        owned = ("--out", "--bin", "--detach", "--resume", "--threads", "--max-parallel",
                 "--wall-hours", "--wall-seconds")
        if any(a.split("=", 1)[0] in owned for a in job["args"]):
            raise ValueError("out/bin/resume/detach/threads/parallel/wall are launcher-owned")
    return spec


def command(job, root, parallel, hours, resume=False):
    out = f"{root}/runs/{job['name']}"
    binary = f"{root}/bin/voxel_census"
    if job["kind"] == "live":
        cmd = [f"{root}/bin/cubarium-search", "voxel-live-train", *job["args"],
               "--out", out, "--threads", str(min(16, parallel)),
               "--wall-seconds", str(hours * 3600)]
        if resume:
            cmd += ["--resume", out]
        return [PYTHON, f"{root}/scripts/run-study.py", "--slot", "0", "--", *cmd]
    script = "batch" if job["kind"] == "batch" else "eco_search"
    cmd = [PYTHON, f"{root}/scripts/studies/{script}.py", out, "--bin", binary,
           "--max-parallel", str(parallel)]
    if resume:
        cmd += ["--resume"]
    else:
        cmd += [*job["args"], "--threads", "1"]
    if job["kind"] == "eco":
        cmd += ["--wall-hours", str(hours)]
    # batch.py places each child in a slot on THIS host. Wrapping the dispatcher
    # itself would hide the other L3 groups and shrink its CPU reserve twice.
    return cmd


def quote(cmd):
    return shlex.join(list(map(str, cmd)))


def ssh(host, key, body):
    return ["ssh", "-i", str(key), "-o", "BatchMode=yes", "-o", "ConnectTimeout=10",
            "-o", "StrictHostKeyChecking=accept-new", host, body]


def rsync(host, key, source, dest, extra=()):
    transport = quote(["ssh", "-i", key, "-o", "BatchMode=yes", "-o", "ConnectTimeout=10",
                       "-o", "StrictHostKeyChecking=accept-new"])
    return ["rsync", "-az", "-e", transport, *extra, source, dest]


def remote_launch(spec_path, number, args):
    return quote([PYTHON, f"{REMOTE}/scripts/graviton-studies.py", "execute",
                  "--spec", spec_path, "--index", number, "--hours", args.hours,
                  "--max-parallel", args.max_parallel, *(["--resume"] if args.resume else [])])


def expected_workers(job, parallel):
    def option(name, default):
        try:
            return job["args"][job["args"].index(name) + 1]
        except ValueError:
            return default
    if job["kind"] == "eco":
        return min(parallel, (int(option("--lambda", "8")) + 1) * int(option("--seeds-per-gen", "2")))
    if job["kind"] == "live":
        return min(16, parallel, int(option("--worlds", "16")))
    # Load no job and execute no host code; just the pure seed parser.
    sys.path.insert(0, str(ROOT / "scripts/studies"))
    import batch
    return min(parallel, job["args"].count("--arm") * len(batch.seed_list(option("--seeds", "1-8"))))


def workflow(args, spec, planning=False):
    hosts = args.host or [f"ec2-user@HOST-{i+1}" for i in range(len(spec["jobs"]))]
    if len(hosts) != len(spec["jobs"]):
        raise ValueError("supply one --host per job, in spec order")
    if any(not re.fullmatch(r"[A-Za-z0-9_.@-]+", h) or h.startswith("-") for h in hosts):
        raise ValueError("invalid SSH host")
    remote_spec = f"{REMOTE}/job-spec.json"
    for i, (host, job) in enumerate(zip(hosts, spec["jobs"])):
        job_path = f"{REMOTE}/runs/{job['name']}"
        local = args.dest / f"host-{i+1}" / job["name"]
        actions = {
            "ship": [ssh(host, args.key, f"mkdir -p {REMOTE}"),
                     rsync(host, args.key, f"{args.kit_dir}/", f"{host}:{REMOTE}/"),
                     rsync(host, args.key, str(args.spec), f"{host}:{remote_spec}")],
            "run": [ssh(host, args.key, remote_launch(remote_spec, i, args))],
            "fetch": [ssh(host, args.key, quote([PYTHON, f"{REMOTE}/scripts/graviton-studies.py",
                       "checkpoint-list", "--dest", job_path])),
                      rsync(host, args.key, f"{host}:{job_path}/", f"{local}/",
                            [f"--include-from={local}/checkpoint-files", "--exclude=**/ckpt/***",
                             "--exclude=checkpoint-a/***", "--exclude=checkpoint-b/***", "--exclude=*.tmp*"])],
            "restore": [ssh(host, args.key, f"mkdir -p {job_path}"),
                        rsync(host, args.key, f"{local}/", f"{host}:{job_path}/")],
        }
        if planning:
            print(f"host {i+1}: {job['kind']} {job['name']}")
            print("  job: " + quote(command(job, REMOTE, args.max_parallel, args.hours, args.resume)))
            for action, commands in actions.items():
                for cmd in commands:
                    print(f"  {action}: {quote(cmd)}")
                if action == "fetch":
                    print("  verify fetch: " + quote(commands[0]))
            resume_args = argparse.Namespace(**vars(args))
            resume_args.resume = True
            print("  resume: " + quote(ssh(host, args.key, remote_launch(remote_spec, i, resume_args))))
        elif args.action == "fetch":
            local.parent.mkdir(parents=True, exist_ok=True)
            result = subprocess.run(actions["fetch"][0], check=True, capture_output=True, text=True)
            snapshot = json.loads(result.stdout)
            # A changed checkpoint selection/state means no consistent copy. Fetch
            # once, fail clearly, and keep the last off-host copy; never poll.
            with tempfile.TemporaryDirectory(prefix=f".{job['name']}-fetch-", dir=local.parent) as temporary:
                incoming = Path(temporary)
                (incoming / "checkpoint-files").write_text(snapshot["filters"])
                transfer = actions["fetch"][1]
                transfer[-1] = f"{incoming}/"
                transfer[transfer.index(f"--include-from={local}/checkpoint-files")] = f"--include-from={incoming}/checkpoint-files"
                subprocess.run(transfer, check=True)
                after = subprocess.run(actions["fetch"][0], check=True, capture_output=True, text=True)
                if json.loads(after.stdout) != snapshot or checkpoint_snapshot(incoming) != snapshot:
                    raise ValueError("checkpoint changed during fetch; previous copy retained, fetch again at a quiet boundary")
                if local.exists():
                    shutil.rmtree(local)
                incoming.rename(local)
                # TemporaryDirectory tolerates the renamed-away directory.
        else:
            if args.action == "restore" and not local.is_dir():
                raise ValueError(f"no fetched recovery directory: {local}")
            for cmd in actions[args.action]:
                subprocess.run(cmd, check=True)


def checkpoint_files(directory):
    """rsync include rules: just the newest committed census checkpoint per run."""
    rules = []
    for ckpt in sorted(directory.rglob("ckpt")):
        if not ckpt.is_dir():
            continue
        complete = sorted(p for p in ckpt.glob("m*") if re.fullmatch(r"m\d+", p.name) and (p / "minute").is_file())
        if not complete:
            continue
        latest = complete[-1].relative_to(directory)
        for parent in reversed(latest.parents):
            if str(parent) != ".":
                rules.append(f"/{parent}/")
        rules.append(f"/{latest}/***")
    pointer = directory / "checkpoint.json"
    if pointer.is_file() and pointer.read_text().strip() in ("checkpoint-a", "checkpoint-b"):
        rules.append(f"/{pointer.read_text().strip()}/***")
    return "\n".join(dict.fromkeys(rules)) + "\n"


def checkpoint_snapshot(directory):
    filters = checkpoint_files(directory)
    identity = {}
    for rule in filters.splitlines():
        if not rule.endswith("/***"):
            continue
        folder = directory / rule[1:-4]
        names = ["minute", "world.bin", "flora.bin", "fauna.bin"] if folder.parent.name == "ckpt" else ["state.json"]
        for name in names:
            path = folder / name
            identity[str(path.relative_to(directory))] = digest(path)
        if folder.name in ("checkpoint-a", "checkpoint-b"):
            identity["checkpoint.json"] = digest(directory / "checkpoint.json")
            for path in sorted(folder.glob("world-*/*.bin")):
                identity[str(path.relative_to(directory))] = digest(path)
    return dict(filters=filters, identity=identity)


def assemble(dest, binary_dir, inputs):
    if dest.exists() and any(dest.iterdir()):
        raise ValueError("kit directory must be new or empty")
    binaries = [binary_dir / n for n in ("voxel_census", "cubarium-search")]
    for binary in binaries:
        with binary.open("rb") as stream:
            header = stream.read(20)
        if header[:6] != b"\x7fELF\x02\x01" or int.from_bytes(header[18:20], "little") != 183:
            raise ValueError(f"not a little-endian aarch64 ELF: {binary}")
    files = [(p, Path("bin") / p.name) for p in binaries]
    if (binary_dir / "build-commit").is_file():
        files.append((binary_dir / "build-commit", Path("build-commit")))
    files += [(ROOT / "scripts/run-study.py", Path("scripts/run-study.py")),
              (Path(__file__), Path("scripts/graviton-studies.py"))]
    files += [(ROOT / f"scripts/studies/{name}.py", Path(f"scripts/studies/{name}.py")) for name in TOOLS]
    for folder in [Path("config"), *inputs]:
        resolved = (ROOT / folder).resolve(strict=True)
        if folder.is_absolute() or ".." in folder.parts or not resolved.is_relative_to(ROOT):
            raise ValueError("inputs must be paths inside the repository")
        if resolved.is_dir():
            files += [(p, p.relative_to(ROOT)) for p in resolved.rglob("*") if p.is_file()]
        else:
            files.append((resolved, folder))
    manifest = {}
    for source, relative in files:
        target = dest / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, target)
        manifest[str(relative)] = digest(target)
    (dest / "kit.json").write_text(json.dumps(manifest, indent=2) + "\n")
    print(f"kit: {dest} ({len(manifest)} files, binaries and inputs hashed)")


def execute(args, spec):
    job = spec["jobs"][args.index]
    out = ROOT / "runs" / job["name"]
    out.parent.mkdir(parents=True, exist_ok=True)
    lock = (ROOT / "runs/.host-job.lock").open("w")
    fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
    if not args.resume and out.exists() and any(out.iterdir()):
        raise ValueError("job already exists; choose a new name or --resume")
    if args.resume and not out.is_dir():
        raise ValueError("restore fetched job directory before --resume")
    # Keep dispatcher output outside OUT: both study tools require an empty OUT.
    log_path = out.parent / f"{job['name']}.log"
    pid = os.fork()
    if pid:
        print(f"detached {job['name']} pid={pid}, log={log_path}")
        return
    os.setsid()
    os.chdir(ROOT)
    os.nice(19)
    with log_path.open("a") as log, open(os.devnull) as null:
        os.dup2(null.fileno(), 0)
        os.dup2(log.fileno(), 1)
        os.dup2(log.fileno(), 2)
        old_hard = resource.getrlimit(resource.RLIMIT_AS)[1]
        cap = LIMIT if old_hard == resource.RLIM_INFINITY else min(LIMIT, old_hard)
        resource.setrlimit(resource.RLIMIT_AS, (cap, cap))
        cpus = len(os.sched_getaffinity(0))
        ceiling = min(57, math.floor(cpus * 0.9), cpus - 1)
        parallel = min(args.max_parallel, ceiling)
        if job["kind"] == "live":
            sys.path.insert(0, str(ROOT / "scripts/studies"))
            import batch
            groups = batch.topology(ROOT / "scripts/run-study.py")
            parallel = min(parallel, len(groups[0]))
        env = os.environ.copy()
        env.pop("CUBARIUM_WORKER_RESERVE", None)
        (out.parent / f"{job['name']}.pid").write_text(str(os.getpid()) + "\n")
        cmd = command(job, str(ROOT), parallel, args.hours, args.resume)
        print(quote(cmd), flush=True)
        # Wall budget is a last-resort stop; checkpoints survive without a final save.
        done = subprocess.run(["timeout", "--signal=TERM", "--kill-after=30s",
                               f"{args.hours * 3600}s", *cmd], env=env)
        out.mkdir(exist_ok=True)
        (out / "HOST_EXIT").write_text(str(done.returncode) + "\n")
    os._exit(0 if done.returncode == 0 else 1)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=["plan", "kit", "up", "ship", "run", "fetch", "restore",
                                           "down", "execute", "checkpoint-list"])
    parser.add_argument("--spec", type=Path)
    parser.add_argument("--host", action="append")
    parser.add_argument("--key", type=Path, default=Path.home() / ".ssh/cubarium-aws.pem")
    cache_root = ROOT
    git_file = ROOT / ".git"
    if git_file.is_file():
        git_dir = git_file.read_text().strip().removeprefix("gitdir: ")
        cache_root = (ROOT / git_dir).resolve().parents[2]
    parser.add_argument("--bin-dir", type=Path, default=cache_root / "target/aarch64-graviton2")
    parser.add_argument("--kit-dir", type=Path, default=Path("/tmp/cubarium-graviton-kit"))
    parser.add_argument("--dest", type=Path, default=Path("runs/graviton-fetch"))
    parser.add_argument("--input", type=Path, action="append", default=[])
    parser.add_argument("--max-parallel", type=int, default=57)
    parser.add_argument("--hours", type=int, default=12)
    parser.add_argument("--price-per-host-hour", type=float)
    parser.add_argument("--worlds-per-worker-hour", type=float, default=2.4,
                        help="explicit planning assumption, not a benchmark")
    parser.add_argument("--resume", action="store_true")
    parser.add_argument("--go", action="store_true", help="Wrysk authorized this specific paid launch")
    parser.add_argument("--index", type=int, default=0)
    args = parser.parse_args(argv)
    try:
        if not 1 <= args.max_parallel <= 57 or args.hours < 1:
            raise ValueError("require 1..57 max-parallel and positive integer hours")
        if args.action == "kit":
            assemble(args.kit_dir, args.bin_dir, args.input)
            return
        if args.action == "checkpoint-list":
            print(json.dumps(checkpoint_snapshot(args.dest), sort_keys=True))
            return
        if args.action == "down":
            subprocess.run([str(ROOT / "scripts/aws-graviton.sh"), "down"], check=True)
            return
        if not args.spec:
            raise ValueError("--spec is required")
        spec = read_spec(args.spec)
        if args.action == "plan":
            price = args.price_per_host_hour
            if price is None or not math.isfinite(price) or price < 0:
                raise ValueError("plan needs a finite nonnegative --price-per-host-hour supplied by you")
            rate = args.worlds_per_worker_hour
            if not math.isfinite(rate) or rate <= 0:
                raise ValueError("worlds-per-worker-hour must be positive and finite")
            count = len(spec["jobs"])
            print(f"OFFLINE PLAN: {count} host(s), {args.hours} h; no AWS/SSH calls")
            print(f"ceiling: {args.max_parallel} one-thread worlds/host; 64 GiB/process; nice 19; remote L3 slots")
            workers = sum(expected_workers(job, args.max_parallel) for job in spec["jobs"])
            print(f"assumption: {rate:g} worlds/worker-hour; expected <= {workers * rate:g} worlds/hour total (CPU-only; memory may reduce)")
            for job in spec["jobs"]:
                if job["kind"] == "eco":
                    argv = job["args"]
                    lam = int(argv[argv.index("--lambda") + 1]) if "--lambda" in argv else 8
                    seeds = int(argv[argv.index("--seeds-per-gen") + 1]) if "--seeds-per-gen" in argv else 2
                    evaluations = (lam + 1) * seeds
                    waves = math.ceil(evaluations / args.max_parallel)
                    print(f"{job['name']}: {lam} candidates x {seeds} seeds + {seeds} baseline = {evaluations} worlds/generation; {waves} waves; ~{waves/rate*60:.1f} min/generation before holdouts/setup")
            print(f"compute cost: {count} hosts x {args.hours} h x ${price:g}/host-h = ${count * args.hours * price:.2f}; storage/transfer extra")
            print(quote([sys.executable, __file__, "kit", "--bin-dir", args.bin_dir,
                         "--kit-dir", args.kit_dir, *sum((["--input", p] for p in args.input), [])]))
            print("after Wrysk's go: " + quote([ROOT / "scripts/aws-graviton.sh", "up", "-n", count, "--hours", args.hours]))
            workflow(args, spec, planning=True)
            print("ALWAYS: " + quote([ROOT / "scripts/aws-graviton.sh", "down"]))
        elif args.action == "up":
            if not args.go:
                raise ValueError("up requires Wrysk's go for this launch; then pass --go")
            subprocess.run([str(ROOT / "scripts/aws-graviton.sh"), "up", "-n", str(len(spec["jobs"])),
                            "--hours", str(args.hours)], check=True)
        elif args.action == "execute":
            execute(args, spec)
        else:
            if not args.host:
                raise ValueError("explicit --host addresses are required")
            workflow(args, spec)
    except (OSError, ValueError, KeyError, subprocess.SubprocessError) as error:
        parser.exit(1, f"graviton-studies: {error}\n")


if __name__ == "__main__":
    main()
