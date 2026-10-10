"""labd tests: fake hosts, fake binaries, real processes; no tuned numbers.

Everything lives in a temp dir inside this checkout (never /tmp: no trees or targets in
RAM). Tasks launch through the setsid launcher except the one systemd round trip.
"""

import json
import os
from pathlib import Path
import platform
import shutil
import signal
import subprocess
import sys
import tempfile
import time
import unittest
from unittest.mock import patch

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent / "studies"))

from labkit import cli  # noqa: E402
from labkit.core import Abort, Kind, Spawn, Task, load_kinds, register  # noqa: E402
from labkit.daemon import Daemon  # noqa: E402
from labkit.hosts import Host, Inventory, Usage, fits  # noqa: E402
from labkit.store import Paths, read_events, spool  # noqa: E402
import test_ab  # noqa: E402

REPO = HERE.parents[1]
RUN_STUDY = REPO / "scripts/run-study.py"


@register
class Relay(Kind):
    """Defined only here: proves a new kind needs no core or CLI edit."""
    name = "test-relay"
    calls = []

    def validate(self, spec, ctx):
        spec.setdefault("word", "hello")
        return spec

    def plan(self, job, ctx):
        Relay.calls.append("plan")
        return [Task(name="first", argv=["sh", "-c", 'printf %s "$0" > {out}/word.txt', job.spec["word"]])]

    def preflight(self, job, ctx):
        Relay.calls.append("preflight")
        job.state["checked"] = True

    def progress(self, job, task, ctx):
        if task is None:
            return None
        Relay.calls.append(f"progress:{task.name}:{task.state}")
        if task.state != "done":
            return Abort(f"{task.name} failed")
        if task.name == "first":
            word = (task.out / "word.txt").read_text()
            return Spawn([Task(name="second", argv=["sh", "-c", 'printf "%s!" "$0" > {out}/word.txt', word])])
        return None

    def finish(self, job, ctx):
        Relay.calls.append("finish")
        return f"relayed {(job.tasks['second'].out / 'word.txt').read_text()} checked={job.state.get('checked')}"


def wait_for(predicate, timeout=30, step=None):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if step:
            step()
        value = predicate()
        if value:
            return value
        time.sleep(0.05)
    raise AssertionError("condition not reached in time")


class LabCase(unittest.TestCase):
    hosts = None

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix=".lab-test-", dir=REPO)
        self.root = Path(self.temp.name)
        self.paths = Paths(state=self.root / "state", cache=self.root / "cache", runs=self.root / "runs",
                           hosts=self.root / "hosts.toml").ensure()
        self.now = [time.time()]
        self.daemons = []

    def tearDown(self):
        for daemon in self.daemons:
            for row in daemon.store.q("SELECT * FROM tasks WHERE state='running'"):
                daemon.stop_task(row, "cancelled", "test teardown")
            daemon.pool.shutdown(wait=True)
        # A fake "remote" host (local transport) keeps its tasks after a simulated down.
        for proc in Path("/proc").glob("[0-9]*"):
            try:
                command = (proc / "cmdline").read_bytes().split(b"\0")
                if int(proc.name) != os.getpid() and any(str(self.root).encode() in a for a in command):
                    os.kill(int(proc.name), signal.SIGKILL)
            except (OSError, ValueError):
                pass
        self.temp.cleanup()

    def inventory(self, **extra):
        data = dict(lab=dict(progress_s=0.5, probe_s=0.5, remote_poll_s=0),
                    build=dict(seed_targets=[], threads=2, mem_gb=1, keep_trees=10),
                    hosts=dict(fake=dict(kind="local", arch=platform.machine(), cpus=4, groups=[[0, 1], [2, 3]],
                                         max_cpu_fraction=1.0, reserve_cpus=0, max_mem_gb=8, mem_gb=8,
                                         launcher="setsid", idle=True, external_load=False)))
        for key, value in extra.items():
            data[key] = value
        return data

    def daemon(self, data=None, clock=None):
        daemon = Daemon(self.paths, Inventory(data or self.inventory(), RUN_STUDY), kinds=load_kinds(),
                        clock=clock or time.time, log=lambda m: None)
        daemon.start()
        self.daemons.append(daemon)
        return daemon

    def submit(self, spec, job_id=None):
        job_id = job_id or f"job-{len(list(self.paths.state.glob('incoming/*')))}-{time.monotonic_ns()}"
        spec.setdefault("_cwd", str(self.root))
        spool(self.paths, "incoming", dict(id=job_id, spec=spec), job_id)
        return job_id

    def state(self, daemon, job_id):
        row = daemon.store.one("SELECT state, reason FROM jobs WHERE id=?", job_id)
        return (row["state"], row["reason"]) if row else (None, None)

    def finished(self, daemon, job_id, timeout=30):
        wait_for(lambda: self.state(daemon, job_id)[0] in ("done", "aborted", "cancelled", "failed", "rejected"),
                 timeout, daemon.step)
        return self.state(daemon, job_id)

    def events(self, name=None, job=None):
        records, _ = read_events(self.paths.events)
        return [r for r in records if (name is None or r["event"] == name) and (job is None or r.get("job") == job)]


class KindInterfaceTests(LabCase):
    def test_kind_defined_in_test_file_runs_end_to_end(self):
        Relay.calls.clear()
        daemon = self.daemon()
        job = self.submit(dict(kind="test-relay", word="ping"))
        state, reason = self.finished(daemon, job)
        self.assertEqual(state, "done", reason)
        self.assertEqual(Relay.calls[:2], ["plan", "preflight"])
        self.assertIn("progress:first:done", Relay.calls)
        self.assertEqual(Relay.calls[-1], "finish")
        out = Path(daemon.store.one("SELECT out FROM jobs WHERE id=?", job)["out"])
        self.assertIn("relayed ping! checked=True", (out / "summary.txt").read_text())
        self.assertTrue((out / "DONE").read_text().startswith("ok:"))
        names = [r["event"] for r in self.events(job=job)]
        for expected in ("submitted", "planned", "preflight-ok", "started", "task-done", "spawned", "done"):
            self.assertIn(expected, names)

    def test_unknown_kind_and_bad_spec_are_rejected(self):
        daemon = self.daemon()
        bad = self.submit(dict(kind="no-such-kind"))
        empty = self.submit(dict(kind="command", argv=[]))
        daemon.step()
        self.assertEqual(self.state(daemon, bad)[0], "rejected")
        self.assertEqual(self.state(daemon, empty)[0], "rejected")

    def test_windowed_command_refused_without_hidden(self):
        daemon = self.daemon()
        job = self.submit(dict(kind="command", argv=["cubarium", "voxel", "--gpu-target", "window"]))
        daemon.step()
        self.assertEqual(self.state(daemon, job)[0], "rejected")

    def test_task_environment_is_headless(self):
        daemon = self.daemon()
        with patch.dict(os.environ, {"DISPLAY": ":0", "WAYLAND_DISPLAY": "wayland-1"}):
            job = self.submit(dict(kind="command", argv=["sh", "-c", 'echo "D=${DISPLAY-unset} W=$WAYLAND_DISPLAY"']))
            self.assertEqual(self.finished(daemon, job)[0], "done")
        out = Path(daemon.store.one("SELECT out FROM jobs WHERE id=?", job)["out"]) / "run/stdout.log"
        self.assertIn("D=unset", out.read_text())
        self.assertNotIn("wayland-1", out.read_text())

    def test_failed_task_aborts_job_with_reason(self):
        daemon = self.daemon()
        job = self.submit(dict(kind="command", argv=["sh", "-c", "exit 3"]))
        state, reason = self.finished(daemon, job)
        self.assertEqual(state, "aborted")
        self.assertIn("exit 3", reason)

    def test_unplaceable_task_aborts_at_plan(self):
        daemon = self.daemon()
        job = self.submit(dict(kind="command", argv=["true"], threads=999))
        state, reason = self.finished(daemon, job)
        self.assertEqual(state, "aborted")
        self.assertIn("never be placed", reason)

    def test_cancel_stops_running_task(self):
        daemon = self.daemon()
        job = self.submit(dict(kind="command", argv=["sleep", "60"]))
        wait_for(lambda: self.events("started", job), step=daemon.step)
        cli.main(["--state", str(self.paths.state), "cancel", job])
        self.assertEqual(self.finished(daemon, job)[0], "cancelled")
        pid = json.loads((Path(daemon.store.one("SELECT out FROM jobs WHERE id=?", job)["out"]) / ".lab/run/pid.json").read_text())["child"]
        wait_for(lambda: not Path(f"/proc/{pid}").exists() or "Z" in Path(f"/proc/{pid}/stat").read_text().split()[2])

    def test_stalled_task_is_stopped(self):
        daemon = self.daemon()
        job = self.submit(dict(kind="command", argv=["sleep", "60"], liveness=dict(stall_min=0.01, paths=["*.log"])))
        state, reason = self.finished(daemon, job)
        self.assertEqual(state, "aborted")
        self.assertIn("stalled", reason)


class PlacementTests(LabCase):
    def test_fits_never_oversubscribes_and_respects_groups(self):
        host = Host(name="h", cpus=6, groups=[[0, 1], [2, 3, 4, 5]], max_cpu_fraction=1.0, reserve_cpus=0,
                    max_mem_gb=100)
        usage = Usage()
        wide = Task(name="wide", argv=["x"], threads=3, l3=True)
        self.assertEqual(fits(wide, host, usage), 1)         # only the larger group can hold 3
        usage.add(wide, 1)
        self.assertIsNone(fits(wide, host, usage))            # group 1 has 1 left, group 0 has 2
        small = Task(name="small", argv=["x"], threads=2, l3=True)
        self.assertEqual(fits(small, host, usage), 0)
        usage.add(small, 0)
        self.assertIsNone(fits(Task(name="t", argv=["x"], threads=2), host, usage))   # host total
        self.assertEqual(fits(Task(name="t", argv=["x"], threads=1), host, usage), -1)

    def test_second_job_waits_until_threads_free(self):
        daemon = self.daemon()
        gates = [self.root / "gate1", self.root / "gate2"]
        jobs = [self.submit(dict(kind="command", threads=3, argv=["sh", "-c", f'while [ ! -f {g} ]; do sleep 0.05; done']))
                for g in gates]
        wait_for(lambda: self.events("started", jobs[0]), step=daemon.step)
        for _ in range(10):
            daemon.step()
            time.sleep(0.02)
        self.assertFalse(self.events("started", jobs[1]))
        self.assertEqual(daemon.store.one("SELECT state FROM tasks WHERE job=?", jobs[1])["state"], "pending")
        gates[0].touch()
        wait_for(lambda: self.events("started", jobs[1]), step=daemon.step)
        self.assertLess(self.events("task-done", jobs[0])[0]["seq"], self.events("started", jobs[1])[0]["seq"])
        gates[1].touch()
        self.assertEqual(self.finished(daemon, jobs[1])[0], "done")

    def test_l3_task_is_confined_to_one_live_group(self):
        data = self.inventory()
        data["hosts"]["fake"].update(groups="live", cpus=None)
        del data["hosts"]["fake"]["cpus"]
        daemon = self.daemon(data)
        host = daemon.inv.hosts["fake"]
        groups = [set(g) for g in host.groups]
        job = self.submit(dict(kind="command", l3=True, threads=1,
                               argv=["sh", "-c", "grep Cpus_allowed_list /proc/self/status"]))
        self.assertEqual(self.finished(daemon, job)[0], "done")
        out = Path(daemon.store.one("SELECT out FROM jobs WHERE id=?", job)["out"]) / "run/stdout.log"
        allowed = set()
        for part in out.read_text().split(":")[1].strip().split(","):
            a, _, b = part.partition("-")
            allowed.update(range(int(a), int(b or a) + 1))
        self.assertIn(allowed, groups)
        group = daemon.store.one("SELECT grp FROM tasks WHERE job=?", job)["grp"]
        self.assertEqual(allowed, groups[group])


class RestartTests(LabCase):
    def labd(self):
        hosts = self.root / "hosts.toml"
        return subprocess.Popen([sys.executable, str(HERE / "lab"), "--state", str(self.paths.state), "--cache",
                                 str(self.paths.cache), "--runs", str(self.paths.runs), "--hosts", str(hosts),
                                 "daemon", "--tick", "0.1"], stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)

    def test_daemon_restart_reconciles_running_task(self):
        (self.root / "hosts.toml").write_text(
            '[lab]\nprogress_s = 1\n[hosts.fake]\nkind = "local"\narch = "%s"\ncpus = 2\ngroups = [[0], [1]]\n'
            'max_cpu_fraction = 1.0\nreserve_cpus = 0\nmax_mem_gb = 8\nlauncher = "setsid"\n' % platform.machine())
        gate = self.root / "gate"
        spec = self.root / "job.toml"
        spec.write_text(f'kind = "command"\nargv = ["sh", "-c", "while [ ! -f {gate} ]; do sleep 0.05; done; echo finished"]\n')
        first = self.labd()
        try:
            out = subprocess.run([sys.executable, str(HERE / "lab"), "--state", str(self.paths.state), "--hosts",
                                  str(self.root / "hosts.toml"), "submit", str(spec)], capture_output=True, text=True)
            job = out.stdout.strip()
            self.assertTrue(job, out.stderr)
            wait_for(lambda: self.events("started", job))
        finally:
            first.send_signal(signal.SIGTERM)
            first.wait(timeout=20)
        lab = self.paths.runs / job / ".lab/run"
        child = json.loads((lab / "pid.json").read_text())["child"]
        self.assertTrue(Path(f"/proc/{child}").exists(), "task must survive the daemon stopping")
        self.assertFalse((lab / "rc.json").exists())
        second = self.labd()
        try:
            wait_for(lambda: [e for e in self.events("daemon-start") if e.get("running_tasks") == 1][1:] or
                     len(self.events("daemon-start")) >= 2)
            gate.touch()
            wait_for(lambda: self.events("done", job))
        finally:
            second.send_signal(signal.SIGTERM)
            second.wait(timeout=20)
        self.assertIn("finished", (self.paths.runs / job / "run/stdout.log").read_text())
        self.assertEqual(len(self.events("started", job)), 1)


@unittest.skipUnless(shutil.which("systemd-run") and subprocess.run(
    ["systemctl", "--user", "show-environment"], capture_output=True).returncode == 0, "no systemd user manager")
class SystemdLauncherTests(LabCase):
    def test_task_runs_as_transient_user_unit(self):
        data = self.inventory()
        data["hosts"]["fake"]["launcher"] = "systemd"
        data["hosts"]["fake"]["max_mem_gb"] = 1
        daemon = self.daemon(data)
        job = self.submit(dict(kind="command", argv=["sh", "-c", "cat /proc/self/cgroup"], mem_gb=0.1))
        self.assertEqual(self.finished(daemon, job)[0], "done")
        unit = daemon.store.one("SELECT unit FROM tasks WHERE job=?", job)["unit"]
        out = Path(daemon.store.one("SELECT out FROM jobs WHERE id=?", job)["out"]) / "run/stdout.log"
        self.assertIn(f"{unit}.service", out.read_text())


class RemoteHostTests(LabCase):
    def test_ssh_kind_host_through_local_transport_uploads_tools_and_fetches_outputs(self):
        data = self.inventory()
        data["hosts"]["far"] = dict(kind="ssh", address="local", arch=platform.machine(), cpus=2,
                                    groups="auto", max_cpu_fraction=1.0, reserve_cpus=0, max_mem_gb=4,
                                    launcher="setsid", root=str(self.root / "far"), python=sys.executable)
        daemon = self.daemon(data)
        self.assertTrue(daemon.inv.hosts["far"].up)
        job = self.submit(dict(kind="command", hosts=["far"], argv=["sh", "-c", "echo remote > {out}/r.txt"]))
        self.assertEqual(self.finished(daemon, job)[0], "done")
        self.assertTrue((self.root / "far/tools/taskwrap.py").exists())
        out = Path(daemon.store.one("SELECT out FROM jobs WHERE id=?", job)["out"])
        self.assertEqual((out / "run/r.txt").read_text().strip(), "remote")


class WaitTests(LabCase):
    def test_wait_returns_when_job_finishes(self):
        daemon = self.daemon()
        gate = self.root / "gate"
        job = self.submit(dict(kind="command", argv=["sh", "-c", f"while [ ! -f {gate} ]; do sleep 0.05; done"]))
        daemon.step()
        waiter = subprocess.Popen([sys.executable, str(HERE / "lab"), "--state", str(self.paths.state), "wait",
                                   "--job", job, "--interval", "0.1", "--timeout", "60"],
                                  stdout=subprocess.PIPE, text=True)
        try:
            wait_for(lambda: self.events("started", job), step=daemon.step)
            time.sleep(0.3)
            self.assertIsNone(waiter.poll())
            gate.touch()
            self.finished(daemon, job)
            out, _ = waiter.communicate(timeout=20)
        finally:
            if waiter.poll() is None:
                waiter.kill()
        self.assertEqual(waiter.returncode, 0)
        self.assertEqual(json.loads(out)["event"], "done")

    def test_wait_any_new_wakes_on_next_terminal_event(self):
        daemon = self.daemon()
        first = self.submit(dict(kind="command", argv=["true"]))
        self.finished(daemon, first)
        waiter = subprocess.Popen([sys.executable, str(HERE / "lab"), "--state", str(self.paths.state), "wait",
                                   "--any-new", "--interval", "0.1", "--timeout", "60"], stdout=subprocess.PIPE, text=True)
        time.sleep(0.5)
        second = self.submit(dict(kind="command", argv=["true"]))
        self.finished(daemon, second)
        out, _ = waiter.communicate(timeout=20)
        self.assertEqual(json.loads(out)["job"], second)


class ABKindTests(LabCase):
    def setUp(self):
        super().setUp()
        self.trees, self.binary = test_ab.make_fixture(self.root)
        data = self.inventory()
        data["hosts"]["fake"].update(groups="live")
        del data["hosts"]["fake"]["cpus"]
        self.data = data

    def commit_change(self, old="rate=3", new="rate=5", tree="branch"):
        config = self.trees[tree] / "config/input.toml"
        config.write_text(config.read_text().replace(old, new))
        subprocess.run(["git", "-C", str(self.trees[tree]), "-c", "user.name=T", "-c", "user.email=t@x.invalid",
                        "commit", "-qam", "change"], check=True)

    def spec(self, **extra):
        arms = {n: dict(tree=str(t), bin=str(self.binary), commit="HEAD") for n, t in self.trees.items()}
        spec = dict(kind="ab", arms=arms, config_path="config/input.toml", preflight_only=True, seeds="1")
        spec.update(extra)
        return spec

    def run_ab(self, **extra):
        daemon = self.daemon(self.data)
        job = self.submit(self.spec(**extra))
        return daemon, job, self.finished(daemon, job)

    def test_identical_arms_abort_at_preflight(self):
        _, job, (state, reason) = self.run_ab()
        self.assertEqual(state, "aborted")
        self.assertIn("identical binary and expanded config", reason)
        self.assertTrue(self.events("preflight-abort", job))

    def test_expected_key_missing_from_diff_aborts(self):
        self.commit_change()
        _, _, (state, reason) = self.run_ab(expect=["flora.rate", "fauna.rate"])
        self.assertEqual(state, "aborted")
        self.assertIn("expected keys absent from diff: flora.rate", reason)

    def test_branch_config_from_another_tree_is_refused(self):
        spec_arms = self.spec()["arms"]
        spec_arms["branch"]["config"] = str(self.trees["main"] / "config/input.toml")
        _, _, (state, reason) = self.run_ab(arms=spec_arms)
        self.assertEqual(state, "aborted")
        self.assertIn("different tree than binary source", reason)

    def test_preflight_only_passes_with_expected_key_and_launches_nothing(self):
        self.commit_change()
        daemon, job, (state, reason) = self.run_ab(expect=["fauna.rate"])
        self.assertEqual(state, "done", reason)
        self.assertIn("preflight-only", reason)
        self.assertEqual(daemon.store.q("SELECT * FROM tasks WHERE job=?", job), [])

    def test_identical_early_metrics_trip_the_abort(self):
        self.commit_change()
        self.binary.write_text(self.binary.read_text().replace("count=8", "count=7"))
        daemon, job, (state, reason) = self.run_ab(preflight_only=False, expect=["fauna.rate"],
                                                   expect_metric=["count"], hours=0.4)
        self.assertEqual(state, "aborted")
        self.assertIn("early-identical", reason)
        stages = {json.loads(r["spec"])["meta"]["stage"] for r in daemon.store.q("SELECT spec FROM tasks WHERE job=?", job)}
        self.assertEqual(stages, {"early"})

    def test_differing_early_metrics_continue_from_own_checkpoints(self):
        self.commit_change()
        daemon, job, (state, reason) = self.run_ab(preflight_only=False, expect=["fauna.rate"],
                                                   expect_metric=["count"], hours=0.4, seeds="1-2")
        self.assertEqual(state, "done", reason)
        rows = daemon.store.q("SELECT spec FROM tasks WHERE job=?", job)
        continuation = [json.loads(r["spec"]) for r in rows if json.loads(r["spec"])["meta"]["stage"] == "continuation"]
        self.assertEqual(len(continuation), 4)
        for task in continuation:
            load = next(a for a in task["argv"] if a.startswith("load="))
            self.assertIn(f"/early/{task['meta']['arm']}/s{task['meta']['seed']}/", load)


class SnapshotBuildTests(LabCase):
    def setUp(self):
        super().setUp()
        self.source = self.root / "src"
        self.source.mkdir()
        (self.source / "version.txt").write_text("one\n")
        subprocess.run(["git", "init", "-q", str(self.source)], check=True)
        self.git("add", "version.txt")
        self.git("commit", "-qm", "one")
        bin_dir = self.root / "bin"
        bin_dir.mkdir()
        cargo = bin_dir / "cargo"
        # Another session commits in the live source tree while this build runs.
        cargo.write_text(f'''#!/bin/sh
mkdir -p "$CARGO_TARGET_DIR/release/examples"
echo two > {self.source}/version.txt
git -C {self.source} -c user.name=T -c user.email=t@x.invalid commit -qam concurrent
cp version.txt "$CARGO_TARGET_DIR/release/examples/voxel_census"
''')
        cargo.chmod(0o755)
        self.env = patch.dict(os.environ, {"PATH": f"{bin_dir}{os.pathsep}{os.environ['PATH']}"})
        self.env.start()

    def tearDown(self):
        self.env.stop()
        subprocess.run(["git", "-C", str(self.source), "worktree", "prune"], capture_output=True)
        super().tearDown()

    def git(self, *args):
        return subprocess.run(["git", "-C", str(self.source), *args], check=True, capture_output=True,
                              text=True).stdout.strip()

    def test_build_uses_snapshot_and_caches_by_commit_and_arch(self):
        seed = self.root / "seed-target/release"
        seed.mkdir(parents=True)
        (seed / "seed-marker").write_text("seeded")
        data = self.inventory()
        data["build"]["seed_targets"] = [str(self.root / "seed-target")]
        daemon = self.daemon(data)
        commit = self.git("rev-parse", "HEAD")
        job = self.submit(dict(kind="build", repo=str(self.source), commit="HEAD"))
        state, reason = self.finished(daemon, job)
        self.assertEqual(state, "done", reason)
        key = f"binary:{commit}:{platform.machine()}:voxel_census"
        artifact = daemon.ctx.artifact(key)
        self.assertEqual(artifact.read_text(), "one\n")
        self.assertNotEqual(self.git("rev-parse", "HEAD"), commit)   # the concurrent commit happened
        tree = self.paths.cache / "trees" / commit[:16]
        self.assertTrue((tree / "target/release/seed-marker").exists())
        again = self.submit(dict(kind="build", repo=str(self.source), commit=commit))
        state, reason = self.finished(daemon, again)
        self.assertEqual(state, "done")
        self.assertIn("cached", reason)
        self.assertEqual(daemon.store.q("SELECT * FROM tasks WHERE job=?", again), [])

    def test_dirty_tree_builds_a_stash_create_snapshot_without_touching_the_stash(self):
        (self.source / "version.txt").write_text("dirty\n")
        daemon = self.daemon()
        job = self.submit(dict(kind="build", repo=str(self.source)))
        state, reason = self.finished(daemon, job)
        self.assertEqual(state, "done", reason)
        spec = json.loads(daemon.store.one("SELECT spec FROM jobs WHERE id=?", job)["spec"])
        self.assertTrue(spec["dirty"])
        artifact = daemon.ctx.artifact(f"binary:{spec['commit']}:{platform.machine()}:voxel_census")
        self.assertEqual(artifact.read_text(), "dirty\n")
        self.assertEqual(self.git("stash", "list"), "")

    def test_job_needing_a_binary_gets_a_provider_build(self):
        daemon = self.daemon()
        commit = self.git("rev-parse", "HEAD")
        job = self.submit(dict(kind="command", binary=dict(tree=str(self.source), commit=commit),
                               argv=["cat", "{bin}"]))
        state, reason = self.finished(daemon, job)
        self.assertEqual(state, "done", reason)
        self.assertTrue(self.events("input-wait", job))
        out = Path(daemon.store.one("SELECT out FROM jobs WHERE id=?", job)["out"]) / "run/stdout.log"
        self.assertEqual(out.read_text(), "one\n")


class Ec2DryRunTests(LabCase):
    def setUp(self):
        super().setUp()
        self.log = self.root / "aws.log"
        self.script = self.root / "aws.sh"
        self.script.write_text(f'''#!/bin/sh
echo "$@" >> {self.log}
case "$1" in
  up) [ -f {self.root}/expired ] && {{ echo "An error occurred (ExpiredToken)" >&2; exit 254; }}
      touch {self.root}/instance; echo i-0abc123 ;;
  status) [ -f {self.root}/instance ] && echo "1 i-0abc123 127.0.0.1 running spot 2026-10-10T00:00:00 c7g.16xlarge"; true ;;
  down) rm -f {self.root}/instance; echo terminated ;;
esac
''')
        self.script.chmod(0o755)
        self.data = self.inventory(ec2=dict(script=str(self.script), types=["c7g.16xlarge"], transport="local",
                                            root=str(self.root / "ec2root"), ready_file="", launcher="setsid",
                                            python=sys.executable, status_s=0, max_mem_gb=8,
                                            instance_types={"c7g.16xlarge": dict(arch=platform.machine(), cpus=8,
                                                                                 mem_gb=16, spot_usd_h=0.75)}))

    def calls(self, verb):
        return [l for l in (self.log.read_text().splitlines() if self.log.exists() else []) if l.split()[0] == verb]

    def approve(self, request, hours=2, usd=5):
        cli.main(["--state", str(self.paths.state), "approve", "ec2", request, "--max-hours", str(hours),
                  "--max-usd", str(usd)])

    def test_no_approval_means_no_up_and_one_request(self):
        daemon = self.daemon(self.data)
        job = self.submit(dict(kind="command", hosts=["ec2"], argv=["true"]))
        wait_for(lambda: self.events("approval-needed"), step=daemon.step)
        for _ in range(5):
            daemon.step()
        self.assertEqual(self.calls("up"), [])
        self.assertEqual(len(self.events("approval-needed")), 1)
        self.assertIn(job, self.events("approval-needed")[0]["jobs"])

    def test_approval_launches_once_and_drained_queue_goes_down(self):
        daemon = self.daemon(self.data)
        job = self.submit(dict(kind="command", hosts=["ec2"], argv=["sh", "-c", "echo on-ec2 > {out}/x"]))
        request = wait_for(lambda: self.events("approval-needed"), step=daemon.step)[0]["approval"]
        self.approve(request)
        state, reason = self.finished(daemon, job)
        self.assertEqual(state, "done", reason)
        wait_for(lambda: self.calls("down"), step=daemon.step)
        self.assertEqual(len(self.calls("up")), 1)
        self.assertIn("--hours 2", self.calls("up")[0])
        self.assertEqual(self.events("host-down")[-1]["reason"], "queue drained")
        out = Path(daemon.store.one("SELECT out FROM jobs WHERE id=?", job)["out"])
        self.assertEqual((out / "run/x").read_text().strip(), "on-ec2")
        self.assertEqual(daemon.ec2.s["state"], "idle")

    def limit(self, hours, usd, advance_h):
        daemon = self.daemon(self.data, clock=lambda: self.now[0])
        gate = self.root / "gate"
        job = self.submit(dict(kind="command", hosts=["ec2"], argv=["sh", "-c", f"while [ ! -f {gate} ]; do sleep 0.05; done"]))
        request = wait_for(lambda: self.events("approval-needed"), step=daemon.step)[0]["approval"]
        self.approve(request, hours, usd)
        wait_for(lambda: self.events("started", job), step=daemon.step)
        self.now[0] += advance_h * 3600
        daemon.step()
        gate.touch()
        self.assertTrue(self.events("ec2-limit"))
        self.assertEqual(len(self.calls("down")), 1)
        self.assertEqual(daemon.ec2.s["state"], "idle")
        self.assertIn(daemon.store.one("SELECT state FROM tasks WHERE job=?", job)["state"], ("lost", "pending"))
        cli.main(["--state", str(self.paths.state), "cancel", job])
        self.finished(daemon, job)

    def test_hour_limit_forces_down(self):
        self.limit(hours=1, usd=100, advance_h=1.01)

    def test_dollar_limit_forces_down(self):
        self.limit(hours=10, usd=0.5, advance_h=0.7)

    def test_expired_login_is_reported_not_fixed(self):
        (self.root / "expired").touch()
        daemon = self.daemon(self.data)
        self.submit(dict(kind="command", hosts=["ec2"], argv=["true"]))
        request = wait_for(lambda: self.events("approval-needed"), step=daemon.step)[0]["approval"]
        self.approve(request)
        wait_for(lambda: self.events("ec2-auth-expired"), step=daemon.step)
        self.assertEqual(daemon.ec2.s["state"], "blocked-auth")
        self.assertEqual(len(self.calls("up")), 1)


if __name__ == "__main__":
    unittest.main()
