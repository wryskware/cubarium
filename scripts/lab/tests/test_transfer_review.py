"""Adversarial laws for off-loop transfers; temporary fakes only, no AWS or SSH."""
from collections import Counter
from concurrent.futures import ThreadPoolExecutor
import hashlib
import json
from pathlib import Path
import queue
import shutil
import subprocess
import sys
import threading
from types import SimpleNamespace as NS
import pytest
from unittest.mock import patch

REPO = Path(__file__).resolve().parents[3]
sys.path[:0] = [str(REPO / "scripts/lab"), str(REPO / "scripts/studies")]
from labkit import census_resume, transfer
from labkit.core import Task
from labkit.daemon import Daemon
from labkit.launch import LocalTransport, SetsidLauncher
import batch


@pytest.fixture(autouse=True)
def temporary_data(tmp_path, monkeypatch):
    def data(name):
        root = tmp_path / name
        root.mkdir()
        return root
    monkeypatch.setitem(globals(), "data", data)


def data(name):
    raise AssertionError("temporary_data fixture is required")


def ckpt(run, minute):
    path = run / "ckpt" / f"m{minute:06d}"
    path.mkdir(parents=True)
    (path / "minute").write_text(str(minute))
    for layer in transfer.LAYERS:
        (path / layer).write_bytes(layer.encode())
    return path


class Rows:
    def __init__(self, rows=()):
        self.rows = list(rows)
        for row in self.rows:
            row.setdefault("reason", None)
        self.kv = {}

    def q(self, *_):
        return self.rows

    def one(self, sql, *args):
        return next((row for row in self.rows if row["job"] == args[0] and row["name"] == args[1]), None)

    def update(self, table, key, **values):
        for row in self.rows:
            if all(row.get(k) == v for k, v in key.items()):
                row.update(values)

    def kv_get(self, key, default=None):
        return self.kv.get(key, default)

    def kv_set(self, key, value):
        self.kv[key] = value


def bare(rows=()):
    d = Daemon.__new__(Daemon)
    d.store = Rows(rows)
    d.events = NS(emit=lambda *a, **kw: None)
    d.clock = lambda: 100.0
    d.log = lambda *a: None
    d.inv = NS(get=lambda key, default=None: default)
    d.done = queue.SimpleQueue()
    d.fetch_limit = 3
    d.fetches, d.fetched, d.timers, d.backoff, d.staging = {}, {}, {}, {}, {}
    d.fetch_waiting, d.fetch_epoch, d.fetch_seen = {}, {}, {}
    d.fetch_seq = 0
    d.ec2 = NS(s3=lambda _: None)
    return d


def test_fetch_slots_eventually_reach_every_running_task():
    root = data("fairness")
    rows = [dict(job="job", name=f"run-{n}", host="far", unit=f"u{n}", started=n, state="running") for n in range(4)]
    d = bare(rows)
    d.host = lambda _: NS(name="far", kind="ssh", up=True)
    d.launchers = {"far": NS(poll=lambda units: {unit: (None, True) for unit, lab in units})}
    d.lab_dir = lambda row: "unused"
    counts = Counter()

    class Transport:
        def get(self, remote, local):
            counts[remote] += 1

    transport = Transport()
    d.mirror_args = lambda row, final: dict(host="far", transport=transport, remote=row["name"],
                                          local=root, batch=None, lab=None)
    # Each mirror finishes well before the next poll. Its completion is drained on that tick.
    d.xfer = NS(submit=lambda fn, *args: fn(*args))
    for round_number in range(20):
        d.drain_transfers()
        d.poll_tasks(force=True)
        if round_number == 2:
            assert all(counts[row["name"]] > 0 for row in rows), dict(counts)
    assert counts["run-3"] > 0, dict(counts)


def test_uncertain_ssh_start_is_reconciled_before_starting_again():
    row = dict(job="job", name="run", state="pending", attempts=0)
    d = bare([row])
    starts = []

    class AckLostTransport:
        def run(self, script, timeout=60):
            # The remote shell did launch; only the SSH acknowledgement was lost.
            if "setsid -f" in script:
                starts.append(script)
                return subprocess.CompletedProcess([], 255 if len(starts) == 1 else 0, "", "connection lost")
            return subprocess.CompletedProcess([], 0, "lab-job-run ALIVE\n", "")

    d.launchers = {"far": SetsidLauncher(AckLostTransport())}
    host = NS(name="far", kind="ssh", max_mem_gb=1)
    task = Task(name="run", argv=["true"])
    spec, start = dict(cwd="remote-output"), dict(lab="remote-lab", taskwrap="taskwrap.py")
    assert d.start_unit(0, "job", task, host, 0, spec, start) is False
    assert row["state"] == "pending" and row["attempts"] == 0
    d.start_unit(row["attempts"], "job", task, host, 0, spec, start)
    assert len(starts) == 1, f"{len(starts)} remote setsid launches after one lost acknowledgement"
    assert row["state"] == "running" and row["attempts"] == 1


def test_old_progress_fetch_cannot_overwrite_a_resumed_task():
    root = data("stale-fetch")
    home, remote = root / "home", root / "remote"
    home.mkdir()
    remote.mkdir()
    (remote / "census.csv").write_text("sim_min,count\n10,1\n")
    task = Task(name="run", argv=["true"], resume_argv=["true"], retries=1)
    row = dict(job="job", name="run", state="running", host="far", unit="u", started=1,
               attempts=1, spec=json.dumps(task.to_dict()))
    d = bare([row])
    d.host = lambda _: NS(name="far", kind="ssh", up=True)
    gate, captured = threading.Event(), threading.Event()

    class SnapshotTransport:
        def __init__(self):
            self.calls = 0

        def get(self, source, target):
            self.calls += 1
            contents = (Path(source) / "census.csv").read_text()
            if self.calls == 1:
                captured.set()
                assert gate.wait(5)
            (Path(target) / "census.csv").write_text(contents)

    transport = SnapshotTransport()
    d.mirror_args = lambda row, final: dict(host="far", transport=transport, remote=remote,
                                          local=home, batch=None, lab=None)
    d.cleanup_task = lambda _: None  # Do not delete even scratch data.
    d.xfer = ThreadPoolExecutor(max_workers=1)
    try:
        d.fetch_async(row)
        assert captured.wait(5)
        (remote / "census.csv").write_text("sim_min,count\n10,1\n20,1\n")
        d.task_lost(dict(row), "host lost")  # Its synchronous fetch gets minute 20.
        assert "20,1" in (home / "census.csv").read_text()
        # The replacement now runs locally and advances the same output path.
        row.update(state="running", host="home", started=2)
        (home / "census.csv").write_text("sim_min,count\n10,1\n20,1\n30,1\n")
        gate.set()
        d.xfer.shutdown(wait=True)
        d.drain_transfers()
        assert "30,1" in (home / "census.csv").read_text(), (home / "census.csv").read_text()
    finally:
        gate.set()
        d.xfer.shutdown(wait=True)


def test_s3_pull_failure_partial_is_ignored_by_census_stitching():
    root = data("s3-partial")
    home, remote = root / "home", root / "remote"
    ckpt(home / "run", 20)
    _, checkpoints, _ = transfer.resume_plan(home)
    spec = dict(rel="run", local=str(home), remote=str(remote), fallback=checkpoints["run"][2], timeout=1,
                pull=dict(uri="s3://unused/ckpt/job/task/run/m000030/", region="unused",
                          dest=str(remote / "run/ckpt/m000030")))

    class FailedPull:
        def run(self, script, timeout):
            # A recursive copy transfers minute first, then fails before completing its layers.
            partial = Path(spec["pull"]["dest"] + ".partial")
            partial.mkdir(parents=True)
            (partial / "minute").write_text("30")
            return subprocess.CompletedProcess([], 1, "", "interrupted copy")

        def put_files(self, local, target, files, timeout):
            for rel in files:
                dest = Path(target) / rel
                dest.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(Path(local) / rel, dest)

    misses = []
    transfer.replay([("s3_ckpt", spec)], FailedPull(), {}, misses)
    assert misses == ["run"]
    assert transfer.newest_checkpoint(remote / "run/ckpt")[0] == 20
    minute, chosen = census_resume.latest_checkpoint(remote / "run")
    assert minute == 20, f"stitching chose {chosen.name} at {minute}; simulator selects m000020"


def test_s3_timeout_falls_back_to_home_checkpoint():
    uploaded = []
    spec = dict(rel="run", local="unused", remote="unused", fallback=["run/ckpt/m000020/minute"], timeout=1,
                pull=dict(uri="s3://unused/ckpt/job/task/run/m000030/", region="unused", dest="unused"))

    class TimedOutPull:
        def run(self, script, timeout):
            raise subprocess.TimeoutExpired(["fake-s3-copy"], timeout)

        def put_files(self, *args, **kw):
            uploaded.append(args)

    error = None
    misses = []
    try:
        transfer.replay([("s3_ckpt", spec)], TimedOutPull(), {}, misses)
    except transfer.StageError as failure:
        error = str(failure)
    assert uploaded and misses == ["run"], f"no fallback, misses={misses}, error={error}"


def test_resume_does_not_choose_a_checkpoint_with_no_layers():
    root = data("minute-only")
    ckpt(root / "run", 20)
    cut = root / "run/ckpt/m000030"
    cut.mkdir()
    (cut / "minute").write_text("30")
    _, checkpoints, _ = transfer.resume_plan(root)
    assert checkpoints["run"][0] == 20, checkpoints["run"]


def test_batch_resume_stages_the_start_inputs_it_always_validates():
    root = data("batch-start")
    job = NS(id="batch-start", out=root / "job")
    local_out = job.out / "batch-task"
    start = ckpt(root / "library/run", 5)
    ckpt(local_out / "batch/base/s1/run", 20)
    binary = root / "binary"
    binary.write_bytes(b"fake binary")
    manifest = dict(binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),
                    identity=dict(config=None, required=None, policies={}, genomes={},
                                  starts={"1": {name: hashlib.sha256((start / name).read_bytes()).hexdigest()
                                                for name in transfer.LAYERS}}),
                    starts={"1": str(start)}, runs=[], args=dict(out=str(local_out / "batch")))
    (local_out / "batch/batch.json").write_text(json.dumps(manifest))
    task = Task(name="batch", argv=["true"], resume_argv=["true"], meta=dict(batch=True),
                files={str(start): "{job}/inputs/starts/m000005"})
    d = bare()
    d.ctx = NS(repo=REPO, artifact=lambda _: None)
    d.paths = NS(repo=REPO)
    d.tools = {"far"}
    d.launchers = {"far": NS(python=sys.executable)}
    host = NS(name="far", kind="ssh", root=str(root / "remote"), idle=True)
    recorder = transfer.Recorder()
    d.prepare(job, NS(out=local_out), task, host, 0, recorder)
    transfer.replay(recorder.ops, LocalTransport(), {}, [])
    remote_out = Path(host.root) / "jobs/batch-start/batch-task/batch"
    # Real batch.resume reads identity.starts before choosing children's own checkpoints.
    # This must succeed before executing anything or removing any markers.
    batch.resume(remote_out, binary, REPO / "scripts/run-study.py", 1, [[0]], 1)


@pytest.mark.parametrize("answer", ["DEAD", "UNKNOWN", "RC", "failed-probe"])
def test_uncertain_start_survives_restart_and_only_absence_allows_relaunch(answer):
    row = dict(job="job", name="run", state="pending", attempts=0)
    starts = []

    class Transport:
        def run(self, script, timeout=60):
            if "setsid -f" in script:
                starts.append(script)
                return subprocess.CompletedProcess([], 255 if len(starts) == 1 else 0, "", "lost ack")
            if answer == "failed-probe":
                return subprocess.CompletedProcess([], 255, "", "host unreachable")
            payload = 'RC {"rc": 0}' if answer == "RC" else answer
            return subprocess.CompletedProcess([], 0, f"lab-job-run {payload}\n", "")

    d = bare([row])
    d.launchers = {"far": SetsidLauncher(Transport())}
    host = NS(name="far", kind="ssh", max_mem_gb=1)
    task = Task(name="run", argv=["true"])
    spec, start = dict(cwd="remote-output"), dict(lab="remote-lab", taskwrap="taskwrap.py")
    assert d.start_unit(0, "job", task, host, 0, spec, start) is False
    assert row["unit"] == "lab-job-run" and row["attempts"] == 0
    # All in-memory transfer state is gone. The persisted task still requires a probe.
    restarted = bare([row])
    restarted.launchers = d.launchers
    assert restarted.start_unit(0, "job", task, host, 0, spec, start) is False
    assert len(starts) == 1
    if answer == "RC":
        assert row["state"] == "running" and row["attempts"] == 1
    else:
        assert row["state"] == "pending" and row["attempts"] == 0
        if answer == "DEAD":
            assert row["unit"] is None
            assert restarted.start_unit(0, "job", task, host, 0, spec, start) is True
            assert len(starts) == 2 and row["attempts"] == 1
        else:
            assert row["unit"] == "lab-job-run"
            assert row["reason"].startswith("start uncertain:")


def test_final_fetch_precedes_waiting_progress_and_both_get_service():
    root = data("final-fairness")
    rows = [dict(job="job", name=f"run-{n}", state="running", host="far", started=n) for n in range(5)]
    d = bare(rows)
    transport = NS(get=lambda *args: None)
    d.mirror_args = lambda row, final: dict(host="far", transport=transport, remote=row["name"],
                                          local=root, batch=None, lab=None)
    workers = []
    d.xfer = NS(submit=lambda fn, *args: workers.append((fn, args)))
    for row in rows[:4]:
        d.fetch_async(row)
    d.fetch_async(rows[4], final=True)
    assert len(workers) == d.fetch_limit
    fn, args = workers[0]
    fn(*args)
    d.drain_transfers()
    assert workers[3][1][0] == ("job", "run-4") and workers[3][1][1][1] is True
    fn, args = workers[1]
    fn(*args)
    d.drain_transfers()
    assert workers[4][1][0] == ("job", "run-3")
    for fn, args in workers[2:]:
        fn(*args)
    d.drain_transfers()
    assert d.fetch_result(rows[4], final=True) == (True, None)
    assert not d.fetches and not d.fetch_waiting


def test_mirror_wait_does_not_count_as_stall_but_fresh_unchanged_mirrors_do():
    root = data("liveness")
    (root / "status").write_text("minute=10")
    task = Task(name="run", argv=["true"], liveness=dict(minute_file="status", stall_min=1))
    row = dict(job="job", name="run", state="running", host="far", started=1,
               live=json.dumps(dict(sig=10, since=1)))
    d = bare([row])
    d.host = lambda _: NS(kind="ssh")
    stopped = []
    d.stop_task = lambda *args: stopped.append(args)
    d.after_task = lambda *args: None
    view, job = NS(name="run", task=task, out=root), NS(id="job")
    now = [100.0]
    d.clock = lambda: now[0]
    d.check_liveness(job, view)
    assert not stopped  # No completed mirror since launch, even though the timeout elapsed.
    d.fetch_seen[("job", "run")] = ((1, False, "far", 1, 0), now[0])
    d.check_liveness(job, view)
    now[0] += 100
    d.check_liveness(job, view)
    assert not stopped  # The old mirror is still unchanged while its refresh is delayed.
    now[0] += 1
    d.fetch_seen[("job", "run")] = ((1, False, "far", 2, 0), now[0])
    d.check_liveness(job, view)
    assert not stopped
    now[0] += 61
    d.fetch_seen[("job", "run")] = ((1, False, "far", 3, 0), now[0])
    d.check_liveness(job, view)
    assert len(stopped) == 1 and stopped[0][1] == "stalled"


@pytest.mark.parametrize("invalid", ["missing-layer", "empty-layer", "bad-minute", "wrong-minute"])
def test_s3_pull_validates_before_rename_and_cleans_partial(invalid):
    root = data("validation")
    source = ckpt(root / "source/run", 30)
    destination = ckpt(root / "destination/run", 30)
    original = (destination / "world.bin").read_bytes()
    (source / "world.bin").write_bytes(b"new world")
    if invalid == "missing-layer":
        (source / "fauna.bin").unlink()
    elif invalid == "empty-layer":
        (source / "fauna.bin").write_bytes(b"")
    else:
        (source / "minute").write_text("NaN" if invalid == "bad-minute" else "40")
    # Replace only the AWS command builder; execute the real validation/cleanup script.
    with patch.object(transfer, "_aws", return_value="fake_copy"):
        script = 'fake_copy() { cp -a "$5/." "$6"; }\n' + transfer.s3_pull_script(
            str(source), "unused", str(destination))
    result = LocalTransport().run(script)
    assert result.returncode != 0
    assert (destination / "world.bin").read_bytes() == original
    assert not Path(str(destination) + ".partial").exists()
    assert transfer.checkpoint_minute(destination) == 30
