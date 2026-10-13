"""Off-loop transfers: launch staging, output mirrors, resume subsets and the optional S3 path.

Threading model (design/lab-daemon.md, Execution): the daemon thread prepares a remote
launch against a `Recorder`, which performs no I/O and only lists the uploads and remote
writes the launch needs. A transfer worker replays that list against the real transport and
puts its outcome on a queue; the daemon thread drains the queue, writes the store and starts
the unit. Workers never write the store. Output mirrors (rsync fetches) work the same way.

Resume subsets: a task's existing outputs go up without old checkpoints. Everything outside a
`ckpt/` folder is sent (args, end_minute, status, census.csv, vitals, logs, batch.json: what
`resume=` and census stitching read, all small); under each run's `ckpt/` only its newest
complete checkpoint, or none when S3 already holds one at least as new.

S3 (optional, `[ec2] s3_bucket`): every S3 command runs on the instance through its own aws
cli and instance role; labd never holds AWS credentials.
"""

import inspect
import os
from pathlib import Path
import re
import shlex
import subprocess

CKPT = re.compile(r"m\d+")
LAYERS = ("world.bin", "flora.bin", "fauna.bin")
UPLOAD_FLOOR_MB_S = 0.5   # [lab] upload_floor_mb_s default
UPLOAD_BASE_S = 60        # [lab] upload_base_s default


class StageError(RuntimeError):
    """A transfer failed while staging; the task goes back to pending without spending a retry."""


# ---------------------------------------------------------------- checkpoints and subsets
def checkpoint_minute(path, name=None):
    """The minute of a complete checkpoint directory, else None.

    Complete: an `mNNNNNN` name, its unsigned integer minute, and all three nonempty
    layer files written by saved::save. `name` validates an S3 staging directory against
    its eventual name; discovery never accepts `.partial` directories."""
    path = Path(path)
    name = path.name if name is None else name
    if not CKPT.fullmatch(name):
        return None
    try:
        minute = int((path / "minute").read_text().strip())
        if minute != int(name[1:]) or not 0 <= minute <= 2**64 - 1:
            return None
        if not all((path / layer).is_file() and (path / layer).stat().st_size > 0 for layer in LAYERS):
            return None
    except (OSError, ValueError):
        return None
    return minute


def checkpoint_check_script(path, name=None):
    """Run the same completeness predicate on the host, before an S3 push or rename."""
    code = (f"from pathlib import Path\nimport re, sys\nCKPT = re.compile({CKPT.pattern!r})\n"
            f"LAYERS = {LAYERS!r}\n" + inspect.getsource(checkpoint_minute) +
            "\nsys.exit(checkpoint_minute(sys.argv[1], sys.argv[2]) is None)\n")
    return shlex.join(["python3", "-c", code, str(path), name or Path(path).name])


def newest_checkpoint(ckpt_dir):
    """(minute, path) of the newest complete checkpoint in a `ckpt/` folder, or None."""
    best = None
    try:
        children = list(Path(ckpt_dir).iterdir())
    except OSError:
        return None
    for child in children:
        minute = checkpoint_minute(child)
        if minute is not None and (best is None or minute > best[0]):
            best = (minute, child)
    return best


def name_minute(name):
    return int(name[1:]) if name and CKPT.fullmatch(name) else None


def resume_plan(local_out):
    """What a (re)launch with existing outputs needs from home.

    Returns (files, checkpoints, runs): `files` every file outside a `ckpt/` folder, relative
    to local_out; `checkpoints` {run_rel: (minute, name, [files])} for each run's newest
    complete checkpoint; `runs` every run dir that has a `ckpt/` folder (some may have none
    complete). Older checkpoints and `.partial` folders are never listed."""
    root = Path(local_out)
    files, checkpoints, runs = [], {}, []
    for dirpath, dirnames, filenames in os.walk(root):
        here = Path(dirpath)
        dirnames[:] = sorted(d for d in dirnames if not d.endswith(".partial"))
        if here.name == "ckpt" and here != root:
            dirnames[:] = []
            run_rel = here.parent.relative_to(root).as_posix()
            runs.append(run_rel)
            best = newest_checkpoint(here)
            if best:
                listed = sorted(p.relative_to(root).as_posix() for p in best[1].rglob("*") if p.is_file())
                checkpoints[run_rel] = (best[0], best[1].name, listed)
            continue
        files += [(here / name).relative_to(root).as_posix() for name in filenames]
    return sorted(files), checkpoints, sorted(runs)


def is_checkpoint_dir(path):
    path = Path(path)
    return path.parent.name == "ckpt" and checkpoint_minute(path) is not None


def size_of(root, files):
    total = 0
    for rel in files:
        try:
            total += (Path(root) / rel).stat().st_size
        except OSError:
            pass
    return total


def tree_size(path):
    path = Path(path)
    if path.is_file():
        return path.stat().st_size
    total = 0
    for dirpath, _, filenames in os.walk(path):
        for name in filenames:
            try:
                total += os.path.getsize(os.path.join(dirpath, name))
            except OSError:
                pass
    return total


def upload_timeout(nbytes, lab):
    """Seconds an upload of nbytes may take: `upload_base_s` plus the bytes at the floor rate
    `upload_floor_mb_s` (both [lab] knobs). A fixed 600 s cut every checkpoint resume on a
    ~2 MB/s home line (2026-10-12) and spent the task's retry."""
    rate = float(lab.get("upload_floor_mb_s", UPLOAD_FLOOR_MB_S))
    base = float(lab.get("upload_base_s", UPLOAD_BASE_S))
    return base + nbytes / max(rate, 1e-3) / 1e6


# ---------------------------------------------------------------- recording transport
class Recorder:
    """Stands in for a remote transport while a launch is prepared on the daemon thread:
    records each put and remote script, performs nothing. `ops` replays in a worker."""
    remote = True

    def __init__(self):
        self.ops = []

    def run(self, script, timeout=60):
        self.ops.append(("run", script, timeout))
        return subprocess.CompletedProcess([], 0, "", "")

    def put(self, local, remote, timeout=None):
        self.ops.append(("put", str(local), str(remote), tree_size(local)))

    def put_dir(self, local, remote, timeout=None):
        self.ops.append(("put_dir", str(local), str(remote), tree_size(local)))

    def put_files(self, local, remote, files, timeout=None):
        if files:
            self.ops.append(("put_files", str(local), str(remote), list(files), size_of(local, files)))

    def add(self, op):
        self.ops.append(op)

    def bytes(self):
        """Bytes this launch may send from home (an S3 op counts its fallback)."""
        total = 0
        for op in self.ops:
            if op[0] in ("put", "put_dir"):
                total += op[3]
            elif op[0] == "put_files":
                total += op[4]
            elif op[0] == "s3_input":
                total += op[1]["bytes"]
        return total


def replay(ops, transport, lab, s3_misses):
    """Run recorded ops against the real transport (worker thread). Raises StageError on the
    first failure. S3 pulls fall back to home's copy; a missing S3 checkpoint is noted in
    `s3_misses` so the daemon stops pointing at it."""
    for op in ops:
        kind = op[0]
        try:
            if kind == "run":
                done = transport.run(op[1], timeout=op[2])
                if done.returncode:
                    raise StageError(f"remote script failed: {done.stderr.strip()[-300:]}")
            elif kind == "put":
                transport.put(op[1], op[2], timeout=upload_timeout(op[3], lab))
            elif kind == "put_dir":
                transport.put_dir(op[1], op[2], timeout=upload_timeout(op[3], lab))
            elif kind == "put_files":
                transport.put_files(op[1], op[2], op[3], timeout=upload_timeout(op[4], lab))
            elif kind == "s3_ckpt":
                spec = op[1]
                try:
                    done = transport.run(s3_pull_script(**spec["pull"]), timeout=spec["timeout"])
                    error = done.stderr.strip()[-200:]
                    pulled = done.returncode == 0
                except (OSError, subprocess.SubprocessError) as failure:
                    pulled, error = False, str(failure)[-200:]
                if pulled:
                    continue
                s3_misses.append(spec["rel"])
                if not spec["fallback"]:
                    raise StageError(f"S3 checkpoint {spec['pull']['uri']} missing and home has none: "
                                     f"{error}")
                files = spec["fallback"]
                transport.put_files(spec["local"], spec["remote"], files,
                                    timeout=upload_timeout(size_of(spec["local"], files), lab))
            elif kind == "s3_input":
                spec = op[1]
                done = transport.run(s3_fetch_binary_script(**spec["fetch"]), timeout=spec["timeout"])
                if done.returncode == 0:
                    continue
                transport.put(spec["source"], spec["remote"], timeout=upload_timeout(spec["bytes"], lab))
                # Seed S3 from the instance (in-region) so the next instance skips this upload.
                transport.run(s3_seed_binary_script(**spec["fetch"]), timeout=spec["timeout"])
            else:
                raise StageError(f"unknown staging op {kind}")
        except StageError:
            raise
        except (OSError, subprocess.SubprocessError, ValueError) as error:
            raise StageError(f"{kind} {op[2] if len(op) > 2 and isinstance(op[2], str) else ''}: "
                             f"{type(error).__name__}: {str(error)[-300:]}") from error


# ---------------------------------------------------------------- S3 scripts (run on the instance)
def _aws(region):
    # A role-less instance must fail fast, not spend minutes on metadata retries.
    return (f"env AWS_METADATA_SERVICE_TIMEOUT=2 AWS_METADATA_SERVICE_NUM_ATTEMPTS=2 "
            f"aws --region {shlex.quote(region)}")


def s3_probe_script(bucket, region):
    q = shlex.quote
    return (f"out=$(timeout 40 {_aws(region)} s3 ls {q(f's3://{bucket}/')} 2>&1 >/dev/null) "
            f"&& echo S3-OK || {{ echo S3-NO; printf '%s\\n' \"$out\" | tail -c 300; }}\n")


def ckpt_uri(bucket, job, task, rel, name=""):
    return f"s3://{bucket}/ckpt/{job}/{task}/{rel}/" + (f"{name}/" if name else "")


def s3_push_script(bucket, region, job, task, remote_out, items):
    """Copy each (run_rel, name) checkpoint to S3 and keep only it under that run's prefix.
    Prints `S3CKPT <run_rel> <name>` for each one that landed."""
    q = shlex.quote
    aws = _aws(region)
    lines = ["set -u"]
    for rel, name in items:
        source = f"{remote_out}/{rel}/ckpt/{name}"
        prefix = ckpt_uri(bucket, job, task, rel)
        lines.append(f"if {checkpoint_check_script(source)} "
                     f"&& {aws} s3 cp --recursive --only-show-errors {q(source)} {q(prefix + name + '/')} "
                     f"&& {aws} s3 rm --recursive --only-show-errors {q(prefix)} --exclude {q(name + '/*')}; "
                     f"then echo S3CKPT {q(rel)} {q(name)}; fi")
    return "\n".join(lines) + "\n"


def s3_pull_script(uri, region, dest):
    """Pull one checkpoint into DEST.partial, then rename: never a half checkpoint at DEST."""
    q = shlex.quote
    partial = dest + ".partial"
    return (f"trap {q('rm -rf -- ' + q(partial))} EXIT\n"
            f"rm -rf {q(partial)} && mkdir -p {q(partial)} "
            f"&& {_aws(region)} s3 cp --recursive --only-show-errors {q(uri)} {q(partial + '/')} "
            f"&& {checkpoint_check_script(partial, Path(dest).name)} "
            f"&& rm -rf {q(dest)} && mv {q(partial)} {q(dest)}\n")


def bin_uri(bucket, safe):
    return f"s3://{bucket}/bin/{safe}/"


def s3_fetch_binary_script(bucket, region, safe, name, sha256, dest):
    """Fetch bin/<safe>/<name> to DEST only when its sha256 matches home's artifact."""
    q = shlex.quote
    temporary = f"{dest}.s3tmp"
    return (f"mkdir -p \"$(dirname {q(dest)})\" && {_aws(region)} s3 cp --only-show-errors "
            f"{q(bin_uri(bucket, safe) + name)} {q(temporary)} "
            f"&& [ \"$(sha256sum {q(temporary)} | cut -d' ' -f1)\" = {q(sha256)} ] "
            f"&& chmod +x {q(temporary)} && mv {q(temporary)} {q(dest)} || {{ rm -f {q(temporary)}; exit 1; }}\n")


def s3_seed_binary_script(bucket, region, safe, name, sha256, dest):
    q = shlex.quote
    aws = _aws(region)
    uri = bin_uri(bucket, safe)
    return (f"{aws} s3 cp --only-show-errors {q(dest)} {q(uri + name)} "
            f"&& printf '%s\\n' {q(sha256)} | {aws} s3 cp --only-show-errors - {q(uri + 'sha256')} || true\n")


def s3_build_wrapper(bucket, region, safe, artifact):
    """bash -c script for a build task: take bin/<safe>/ from S3 when its binary matches its
    sha256 file; otherwise run the build ("$@") and push the result with its checksum.
    `artifact` is the produced path (may hold the `{out}` placeholder)."""
    q = shlex.quote
    aws = _aws(region)
    uri = bin_uri(bucket, safe)
    return "\n".join([
        f"art={q(artifact)}; name=$(basename \"$art\"); s3={q(uri)}; tmp=$(mktemp -d)",
        f"if {aws} s3 cp --only-show-errors \"${{s3}}sha256\" \"$tmp/sha256\" "
        f"&& {aws} s3 cp --only-show-errors \"$s3$name\" \"$tmp/$name\" "
        "&& [ \"$(sha256sum \"$tmp/$name\" | cut -d' ' -f1)\" = \"$(cut -d' ' -f1 \"$tmp/sha256\")\" ]; then",
        "  mkdir -p \"$(dirname \"$art\")\" && chmod +x \"$tmp/$name\" && mv \"$tmp/$name\" \"$art\" && rm -rf \"$tmp\"",
        "  echo \"labd: $name from $s3 (sha256 verified); no build\"; exit 0",
        "fi",
        "rm -rf \"$tmp\"; echo \"labd: no matching binary at $s3; building\" >&2",
        "\"$@\"; rc=$?",
        "if [ $rc = 0 ] && [ -f \"$art\" ]; then",
        "  sum=$(mktemp); sha256sum \"$art\" | cut -d' ' -f1 > \"$sum\"",
        f"  {{ {aws} s3 cp --only-show-errors \"$art\" \"$s3$name\" "
        f"&& {aws} s3 cp --only-show-errors \"$sum\" \"${{s3}}sha256\"; }} "
        "|| echo \"labd: S3 binary push failed; the build stands\" >&2",
        "  rm -f \"$sum\"",
        "fi",
        "exit $rc",
    ]) + "\n"


def parse_pushed(stdout):
    pushed = {}
    for line in stdout.splitlines():
        parts = line.split()
        if len(parts) == 3 and parts[0] == "S3CKPT":
            pushed[parts[1]] = parts[2]
    return pushed
