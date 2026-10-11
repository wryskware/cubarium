"""EC2 tail moves and fill laws: temporary stores, mocked launchers/provider, no AWS."""

import contextlib
import hashlib
import io
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import Mock, patch

from labkit import cli
from labkit.core import Task
from labkit.daemon import Daemon
from labkit.ec2 import fill_profile, tail_progress
from labkit.hosts import Host, Inventory
from labkit.store import Paths, read_events
from labkit.stops import CensusRun, census
from labkit.top import Dashboard

REPO = Path(__file__).resolve().parents[3]


class Ec2TailTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix=".ec2-tail-test-", dir=REPO)
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.paths = Paths(self.root / "state", self.root / "cache", self.root / "runs", self.root / "hosts.toml")
        self.paths.hosts.write_text('[hosts.home]\nkind="local"\ncpus=16\nmax_mem_gb=16\n')
        self.now = 1000
        data = dict(hosts=dict(home=dict(kind="local", cpus=16, groups=[list(range(16))],
                                        max_cpu_fraction=1, reserve_cpus=0, max_mem_gb=16,
                                        launcher="setsid", external_load=False)),
                    build=dict(threads=2, mem_gb=1, seed_targets=[]),
                    ec2=dict(types=["mock"], max_cpu_fraction=1, reserve_cpus=0,
                             tail_fraction=.3, tail_hold_min=2, tail_min_remaining_h=.4,
                             instance_types=dict(mock=dict(arch="aarch64", cpus=16, mem_gb=16,
                                                           speed=2, spot_usd_h=1))))
        self.d = Daemon(self.paths, Inventory(data), clock=lambda: self.now, log=lambda _: None)
        self.addCleanup(self.d.store.db.close)
        self.addCleanup(lambda: self.d.pool.shutdown(wait=False))
        self.pool = self.d.ec2
        self.pool.host = Host("ec2", kind="ec2", arch="aarch64", cpus=16, groups=[list(range(16))],
                              max_cpu_fraction=1, reserve_cpus=0, max_mem_gb=16, speed=2, price_usd_h=1,
                              root=str(self.root / "remote"), launcher="setsid")
        self.pool.s = dict(state="up", type="mock", instance="i-mock", launched=self.now,
                           max_hours=100, max_usd=100)
        self.d.launchers["ec2"] = Mock()
        self.d.launchers["home"] = Mock(python=sys.executable)
        self.d.launchers["home"].name = "mock"
        self.d.fetch_task = Mock()
        self.d.cleanup_task = Mock()
        self.d.local_load = Mock()
        self.d._due = Mock(side_effect=lambda name, interval: name == "ec2-tail-check")
        self.pool.script = Mock(return_value=subprocess.CompletedProcess([], 0, "", ""))
        commit = subprocess.run(["git", "rev-parse", "HEAD"], cwd=REPO, capture_output=True,
                                text=True, check=True, timeout=5).stdout.strip()
        self.arm_key = self.d.ctx.binary(REPO, commit, arch="aarch64")
        self.home_key = self.d.ctx.binary(REPO, commit, arch="x86_64")
        self.cache(self.arm_key, "arm")
        self.cache(self.home_key, "x86")

    def cache(self, key, content):
        path = self.root / key.replace(":", "-")
        path.write_text(content)
        self.d.store.x("INSERT OR REPLACE INTO artifacts(key,path) VALUES(?,?)", key, str(path))

    def task(self, name="continuation-a-s1", current=50, checkpoint=45, start=20, total=100, **extra):
        job = self.d.store.one("SELECT id FROM jobs WHERE id='sim'")
        out = self.paths.runs / "sim"
        if not job:
            spec = dict(hours=2, arms=dict(a={}), seed_list=[1], early_read=True,
                        early_minute=start, expect_metric=["count"])
            self.d.store.x("INSERT INTO jobs(id,kind,state,spec,out,kstate,submitted,priority) "
                           "VALUES('sim','ab','running',?,?,?,0,0)", json.dumps(spec), str(out), "{}")
        root = out / name
        (root / "run/ckpt" / f"m{checkpoint:06d}").mkdir(parents=True)
        (root / "run/ckpt" / f"m{checkpoint:06d}" / "minute").write_text(str(checkpoint))
        (root / "run/status").write_text(f"minute={current} end={start + total} speed=2")
        (root / "run/args").write_text("original\nvitals=/remote/vitals\n")
        task = Task(name, ["{input:" + self.arm_key + "}", "2", "run={out}/run"],
                    arch="aarch64", hosts=["ec2"], inputs=[self.arm_key], outputs=str(root), threads=2,
                    l3=True, est_h=4, resume_argv=["{input:" + self.arm_key + "}", "resume={out}/run"],
                    meta=dict(stage="continuation", start_minute=start, total_minutes=total), **extra)
        self.d.store.x("INSERT INTO tasks(job,name,seq,state,spec,host,grp,unit,attempts,started) "
                       "VALUES('sim',?,0,'running',?,'ec2',0,?,1,?)",
                       name, json.dumps(task.to_dict()), name, self.now)
        return task

    def held(self):
        self.pool.s["tail_since"] = self.now - self.pool.cfg["tail_hold_min"] * 60

    def row(self, name="continuation-a-s1"):
        return self.d.store.one("SELECT * FROM tasks WHERE job='sim' AND name=?", name)

    def moves(self):
        return [e for e in read_events(self.paths.events)[0] if e["event"] == "tail-moved"]

    def test_held_tail_moves_latest_checkpoint_same_commit_x86_and_drains(self):
        for name in ("continuation-a-s1", "continuation-b-s1"):
            self.task(name)
        self.held()
        self.pool.tick()
        for name in ("continuation-a-s1", "continuation-b-s1"):
            row = self.row(name)
            task = Task.from_dict(json.loads(row["spec"]))
            self.assertEqual((row["state"], row["host"], task.arch), ("running", "home", "x86_64"))
            self.assertEqual(task.inputs, [self.home_key])
            launch = json.loads((self.paths.runs / "sim/.lab" / name / "launch.json").read_text())
            self.assertEqual(launch["argv"][0], str(self.d.ctx.artifact(self.home_key)))
            self.assertIn("resume=", launch["argv"][1])
            self.assertEqual(row["attempts"], 1)  # a planned move consumes no retry
        self.assertEqual(len(self.moves()), 2)
        self.assertTrue(all(e["from_minute"] == 45 and e["est_h_saved"] > 0 and e["usd_saved"] >= 0
                            for e in self.moves()))
        self.assertAlmostEqual(sum(e["usd_saved"] for e in self.moves()),
                               (1 - (50 - 20) / 100) * 4 / 2 * self.pool.cfg["instance_types"]["mock"]["spot_usd_h"])
        self.assertEqual(self.pool.s["state"], "idle")
        self.pool.script.assert_called_once_with("down", timeout=300)

    def test_pending_eligible_work_resets_hold(self):
        task = self.task()
        self.held()
        self.d.store.x("INSERT INTO tasks(job,name,seq,state,spec) VALUES('sim','next',1,'pending',?)",
                       json.dumps(task.to_dict()))
        self.pool.tick()
        self.assertEqual(self.moves(), [])
        self.assertNotIn("tail_since", self.pool.s)

    def test_hold_and_high_fill_prevent_move(self):
        self.task()
        self.pool.tick()
        self.now += self.pool.cfg["tail_hold_min"] * 60 - 1
        self.pool.tick()
        self.assertEqual(self.moves(), [])
        self.task("continuation-b-s1")
        self.task("continuation-c-s1")
        self.now += 2
        self.pool.tick()
        self.assertEqual(self.moves(), [])
        self.assertNotIn("tail_since", self.pool.s)

    def test_near_end_or_excess_checkpoint_replay_stays(self):
        for current, checkpoint in ((119, 118), (75, 20)):
            with self.subTest(current=current):
                name = f"continuation-a-s{current}"
                self.task(name, current=current, checkpoint=checkpoint)
                self.held()
                self.pool.tick()
                self.assertEqual(self.row(name)["host"], "ec2")
                self.d.store.update("tasks", dict(job="sim", name=name), state="done")
        self.assertEqual(self.moves(), [])

    def test_no_local_threads_memory_l3_or_host_prevents_move(self):
        self.task()
        self.held()
        host = self.d.inv.hosts["home"]
        for field, value in (("external_threads", 16), ("max_mem_gb", 0), ("groups", None), ("up", False)):
            with self.subTest(field=field):
                old = getattr(host, field)
                host.external_load = True
                setattr(host, field, value)
                self.pool.tick()
                self.assertEqual(self.moves(), [])
                setattr(host, field, old)
        with patch("labkit.ec2.mem_available_gb", return_value=0):
            self.pool.tick()
        self.d.launchers["ec2"].stop.assert_not_called()

    def test_missing_binary_builds_before_move_without_queueing_tail(self):
        self.task()
        self.held()
        self.d.store.x("DELETE FROM artifacts WHERE key=?", self.home_key)
        self.pool.tick()
        self.pool.tick()
        self.assertEqual(self.row()["host"], "ec2")
        self.d.launchers["ec2"].stop.assert_not_called()
        providers = self.d.store.q("SELECT * FROM providers WHERE key=?", self.home_key)
        self.assertEqual(len(providers), 1)
        job = self.d.job(providers[0]["job"])
        self.assertEqual(job.spec["arch"], "x86_64")
        self.assertEqual(job.spec["commit"], self.home_key.split(":")[1])
        self.d.plan_jobs()
        self.assertTrue(self.d.job(job.id).tasks["build"].task.local_only)
        self.cache(self.home_key, "built-x86")
        self.pool.tick()
        self.assertEqual(self.row()["host"], "home")
        self.assertEqual(self.row()["state"], "running")

    def test_local_capacity_is_charged_between_moves(self):
        for name in ("continuation-a-s1", "continuation-b-s1"):
            self.task(name)
        host = self.d.inv.hosts["home"]
        host.cpus, host.groups = 2, [[0, 1]]
        self.held()
        self.pool.tick()
        self.assertEqual(len(self.moves()), 1)
        self.assertEqual(len(self.pool.on_ec2()), 1)
        self.assertEqual(self.pool.s["state"], "up")
        self.pool.script.assert_not_called()

    def test_exact_tail_fraction_triggers_but_unknown_or_uncheckpointed_work_stays(self):
        task = self.task()
        self.pool.cfg["tail_fraction"] = task.threads / self.pool.host.usable
        self.held()
        spec = task.to_dict()
        spec["est_h"] = None
        self.d.store.update("tasks", dict(job="sim", name=task.name), spec=json.dumps(spec))
        self.pool.tick()
        self.d.launchers["ec2"].stop.assert_not_called()
        spec["est_h"] = task.est_h
        self.d.store.update("tasks", dict(job="sim", name=task.name), spec=json.dumps(spec))
        minute = Path(task.outputs) / "run/ckpt/m000045/minute"
        minute.unlink()
        self.pool.tick()
        self.d.launchers["ec2"].stop.assert_not_called()
        minute.write_text("45")
        self.pool.tick()
        self.assertEqual(len(self.moves()), 1)

    def test_final_fetch_uses_newest_checkpoint(self):
        task = self.task()
        self.held()
        def fetch(row):
            if self.d.launchers["ec2"].stop.called:
                checkpoint = Path(task.outputs) / "run/ckpt/m000049"
                checkpoint.mkdir(exist_ok=True)
                (checkpoint / "minute").write_text("49")
        self.d.fetch_task.side_effect = fetch
        self.pool.tick()
        self.assertEqual(self.moves()[0]["from_minute"], 49)

    def test_stop_or_final_fetch_failure_never_launches_home(self):
        self.task()
        self.held()
        self.d.launchers["ec2"].stop.side_effect = OSError("stop failed")
        self.pool.tick()
        self.assertEqual(self.row()["state"], "running")
        self.d.launchers["ec2"].stop.side_effect = None
        self.d.fetch_task.side_effect = [None, OSError("fetch failed")]
        self.pool.tick()
        self.assertEqual(self.row()["state"], "pending")
        self.assertEqual(json.loads(self.row()["spec"])["arch"], "aarch64")
        self.d.cleanup_task.assert_not_called()
        self.d.launchers["home"].start.assert_not_called()
        self.assertEqual(self.pool.s["state"], "up")

    def test_saved_census_paths_follow_destination(self):
        task = self.task()
        task.meta["remote_paths"] = {str(Path(task.outputs) / "vitals"): "/remote/vitals"}
        self.d.store.update("tasks", dict(job="sim", name=task.name), spec=json.dumps(task.to_dict()))
        self.held()
        self.pool.tick()
        self.assertIn(f"vitals={task.outputs}/vitals", (Path(task.outputs) / "run/args").read_text())

    def test_batch_progress_counts_saved_starts_and_replay(self):
        root = self.root / "batch-task"
        batch = root / "batch"
        batch.mkdir(parents=True)
        (batch / "batch.json").write_text(json.dumps(dict(args=dict(hours=2),
            load_minutes={"1": 60}, runs=[dict(arm="a", seed=1), dict(arm="b", seed=1)])))
        run = batch / "a/s1/run"
        (run / "ckpt/m000090").mkdir(parents=True)
        (run / "ckpt/m000090/minute").write_text("90")
        (run / "status").write_text("minute=100 end=180")
        (run / "args").write_text("original")
        task = Task("batch", ["true"], outputs=str(root), meta=dict(batch=True))
        left, replay, minute = tail_progress(task)
        self.assertAlmostEqual(left, (240 - 40) / 240)
        self.assertAlmostEqual(replay, 10 / 240)
        self.assertEqual(minute, 90)

    def test_batch_move_updates_binary_identity_and_resumes_checkpoint(self):
        import batch
        task = self.task()
        root = Path(task.outputs) / "batch"
        run = root / "a/s1/run"
        (run / "ckpt/m000045").mkdir(parents=True)
        (run / "ckpt/m000045/minute").write_text("45")
        (run / "args").write_text("old\nvitals=/remote/vitals\n")
        (run / "status").write_text("minute=50 end=120")
        binary = self.d.ctx.artifact(self.arm_key)
        old_hash = hashlib.sha256(binary.read_bytes()).hexdigest()
        manifest = dict(binary_sha256=old_hash, args=dict(hours=2), identity=dict(
            config=None, required=None, policies={}, starts={}),
            runs=[dict(arm="a", seed=1, slot=0, command=[sys.executable, "wrapper", "--slot", "0", "--",
                "chrt", "-i", "0", "nice", "-n", "19", str(binary), "2"])])
        (root / "batch.json").write_text(json.dumps(manifest))
        task.meta.update(batch=True, remote_paths={str(run.parent / "vitals"): "/remote/vitals"})
        task.resume_argv = ["{python}", str(REPO / "scripts/studies/batch.py"), "{out}/batch",
                            "--bin", "{input:" + self.arm_key + "}", "--resume"]
        self.d.store.update("tasks", dict(job="sim", name=task.name), spec=json.dumps(task.to_dict()))
        self.held()
        self.pool.tick()
        self.assertEqual(self.row()["state"], "running")
        moved = json.loads((root / "batch.json").read_text())
        native = self.d.ctx.artifact(self.home_key)
        self.assertNotEqual(moved["binary_sha256"], old_hash)
        self.assertEqual(moved["binary_sha256"], hashlib.sha256(native.read_bytes()).hexdigest())
        self.assertIn(str(native), moved["runs"][0]["command"])
        self.assertIn(str(run.parent / "vitals"), (run / "args").read_text())
        with patch.object(batch, "execute") as execute:
            batch.resume(root, native, self.root / "wrapper", 1, [[0, 1]], 2)
        command = execute.call_args.args[1][0]["command"]
        self.assertIn(str(REPO / "scripts/lab/labkit/census_resume.py"), command)
        self.assertIn(str(run.parent), command)
        self.assertTrue(execute.call_args.args[1][0]["append_census"])
        self.assertIn(str(native), command)

    def test_batch_resume_adapter_keeps_census_minutes(self):
        import batch
        root = self.root / "batch"
        child = root / "a/s1"
        run = child / "run"
        (run / "ckpt/m000025").mkdir(parents=True)
        (run / "ckpt/m000025/minute").write_text("25")
        (run / "args").write_text("original")
        path = child / "census.csv"
        path.write_text("sim_min,count\n" + "".join(f"{m},1\n" for m in range(29)))
        binary = self.root / "census"
        binary.write_text(f"#!{sys.executable}\nprint('sim_min,count')\n"
                          "for m in range(25, 31): print(f'{m},2')\n")
        binary.chmod(0o755)
        manifest = dict(binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(), identity=dict(
            config=None, required=None, policies={}, starts={}),
            runs=[dict(arm="a", seed=1, slot=0, command=["original"])])
        (root / "batch.json").write_text(json.dumps(manifest))
        with patch.object(batch, "execute") as execute:
            batch.resume(root, binary, self.root / "wrapper", 1, [[0, 1]], 1)
        command = execute.call_args.args[1][0]["command"]
        with path.open("ab") as stdout:
            result = subprocess.run(command[command.index("19") + 1:], stdout=stdout, timeout=5)
        self.assertEqual(result.returncode, 0)
        _, rows = census(CensusRun("a", "1", [path]))
        self.assertEqual([float(row["sim_min"]) for row in rows], list(range(31)))
        self.assertEqual(len(path.read_text().splitlines()), len(rows) + 1)

    def test_status_prints_fill_and_move_summary(self):
        self.task()
        self.held()
        self.pool.tick()
        details = fill_profile([Task("a", ["true"], est_h=1)], self.d.inv.hosts["home"])
        self.d.store.x("INSERT INTO approvals(id,state,request) VALUES('mock','open',?)", json.dumps(details))
        with contextlib.redirect_stdout(io.StringIO()) as output:
            cli.main(["--state", str(self.paths.state), "--cache", str(self.paths.cache),
                      "--runs", str(self.paths.runs), "--hosts", str(self.paths.hosts), "status"])
        self.assertIn("fill ", output.getvalue())
        self.assertIn("idle $", output.getvalue())
        self.assertIn("tasks moved home", output.getvalue())

    def test_ab_resume_after_move_and_spot_loss_keeps_continuous_census(self):
        binary = self.root / "fake-census"
        binary.write_text(f"#!{sys.executable}\n" + '''
from pathlib import Path
import sys
run = Path(next(a.split('=', 1)[1] for a in sys.argv if a.startswith('resume=')))
minute = max(float(p.read_text()) for p in (run / 'ckpt').glob('m*/minute'))
print('sim_min,count')
for m in range(int(minute), 31): print(f'{m},1')
(run / 'status').write_text('minute=30 end=120 speed=2')
''')
        binary.chmod(0o755)
        self.cache(self.arm_key, binary.read_text())
        self.cache(self.home_key, binary.read_text())
        for reason in ("move", "spot loss"):
            with self.subTest(reason=reason):
                name = "continuation-a-s1" if reason == "move" else "continuation-b-s1"
                task = self.task(name, current=28, checkpoint=25)
                task.resume_argv = [sys.executable, str(REPO / "scripts/lab/labkit/census_resume.py"),
                                    "{input:" + self.arm_key + "}", "{out}", "2", "original"]
                self.d.store.update("tasks", dict(job="sim", name=name), spec=json.dumps(task.to_dict()))
                root = Path(task.outputs)
                path = root / "census.csv"
                path.write_text("sim_min,count\n" + "".join(f"{m},1\n" for m in range(20, 29)) + "29,")
                self.d.ctx.artifact(self.arm_key).chmod(0o755)
                self.d.ctx.artifact(self.home_key).chmod(0o755)
                if reason == "move":
                    self.held()
                    self.pool.tick()
                    self.assertEqual(self.row(name)["state"], "running")
                else:
                    self.d.store.update("tasks", dict(job="sim", name=name), attempts=1)
                    task.retries = 1
                    self.d.store.update("tasks", dict(job="sim", name=name), spec=json.dumps(task.to_dict()))
                    self.d.task_lost(self.row(name), "spot loss")
                updated = Task.from_dict(json.loads(self.row(name)["spec"]))
                argv = [a.replace("{out}", str(root)).replace("{input:" + updated.inputs[0] + "}",
                        str(self.d.ctx.artifact(updated.inputs[0]))) for a in updated.argv]
                with path.open("ab") as stdout:
                    result = subprocess.run(argv, stdout=stdout, check=False, timeout=5)
                self.assertEqual(result.returncode, 0)
                headers, rows = census(CensusRun("a", "1", [path]))
                self.assertEqual([float(row["sim_min"]) for row in rows], list(range(20, 31)))
                self.assertEqual(len(path.read_text().splitlines()), len(rows) + 1)
                self.assertEqual(len(headers), 1)
                dashboard = Dashboard(self.paths)
                try:
                    progress = dashboard.task(self.d.store.one("SELECT * FROM jobs WHERE id='sim'"),
                                              self.row(name), self.now)
                    self.assertEqual(progress.done, 10)
                finally:
                    dashboard.close()


class FillProfileTests(unittest.TestCase):
    def test_fill_integral_equals_task_work_and_shows_tail(self):
        host = Host("ec2", cpus=8, max_cpu_fraction=1, reserve_cpus=0, max_mem_gb=16,
                    speed=2, price_usd_h=3)
        tasks = [Task("long", ["true"], threads=2, est_h=8),
                 *[Task(f"short-{i}", ["true"], threads=2, est_h=2) for i in range(9)]]
        build = Task("build", ["true"], threads=2, est_h=1)
        result = fill_profile(tasks, host, [build], setup_h=.5)
        busy = sum(s["fraction"] * s["hours"] * host.usable for s in result["fill_profile"])
        self.assertAlmostEqual(busy, sum(t.threads * t.est_h / host.speed for t in [build, *tasks]))
        self.assertLessEqual(result["fill_mean"], 1)
        self.assertGreaterEqual(result["fill_mean"], 0)
        self.assertLess(result["fill_profile"][-1]["fraction"], 1)
        self.assertAlmostEqual(result["idle_usd"],
                               result["schedule_h"] * host.price_usd_h * (1 - result["fill_mean"]))

    def test_unknown_duration_stays_unknown(self):
        host = Host("ec2", cpus=8)
        result = fill_profile([Task("unknown", ["true"])], host)
        for key in ("fill_profile", "fill_mean", "idle_usd"):
            self.assertIsNone(result[key])

    def test_schedule_respects_memory(self):
        host = Host("ec2", cpus=8, max_cpu_fraction=1, reserve_cpus=0, max_mem_gb=3)
        tasks = [Task(f"t{i}", ["true"], mem_gb=2, threads=2, est_h=1) for i in range(4)]
        result = fill_profile(tasks, host)
        self.assertEqual(result["schedule_h"], sum(t.est_h for t in tasks))
        self.assertTrue(all(s["fraction"] == tasks[0].threads / host.usable for s in result["fill_profile"]))
