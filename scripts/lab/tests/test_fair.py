"""Fair placement with fake launches and isolated sqlite state."""

import contextlib
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from labkit import cli
from labkit.core import Task
from labkit.daemon import Daemon
from labkit.hosts import Inventory
from labkit.store import Paths


class FairTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix=".fair-test-", dir=Path(__file__).resolve().parents[3])
        self.addCleanup(self.temp.cleanup)
        root = Path(self.temp.name)
        self.paths = Paths(root / "state", root / "cache", root / "runs", root / "hosts.toml")
        self.paths.hosts.write_text('[hosts.desktop]\ncpus=8\n')
        inventory = Inventory(dict(hosts=dict(desktop=dict(cpus=8, groups=[8], max_cpu_fraction=1,
                                                           reserve_cpus=0, max_mem_gb=16, launcher="setsid"))))
        self.d = Daemon(self.paths, inventory, kinds={}, clock=lambda: 1000)
        self.addCleanup(self.d.store.db.close)
        self.addCleanup(self.d.pool.shutdown)
        self.d.local_load = lambda: None
        self.launched = []
        self.d.launch = self.launch
        memory = patch("labkit.daemon.mem_available_gb", return_value=100)
        memory.start()
        self.addCleanup(memory.stop)

    def launch(self, row, task, host, group):
        self.launched.append((row["job"], task.name, host.name))
        self.d.store.update("tasks", dict(job=row["job"], name=task.name), state="running",
                            host=host.name, grp=group, started=1000)
        return True

    def task(self, job, name, submitted=1, jp=0, state="pending", host="desktop", group=-1, started=1000, **kw):
        self.d.store.x("INSERT OR IGNORE INTO jobs(id,kind,state,spec,out,priority,submitted) VALUES(?,?,?,?,?,?,?)",
                       job, "command", "running", "{}", str(self.paths.runs / job), jp, submitted)
        seq = len(self.d.store.q("SELECT name FROM tasks WHERE job=?", job))
        task = Task(name=name, argv=["true"], **kw)
        self.d.store.x("INSERT INTO tasks(job,name,seq,state,spec,host,grp,started) VALUES(?,?,?,?,?,?,?,?)",
                       job, name, seq, state, json.dumps(task.to_dict()), host, group, started)
        return task

    def remote(self, **kw):
        host = self.d.inv.hosts["desktop"]
        from dataclasses import replace
        self.d.inv.hosts["far"] = replace(host, name="far", kind="ssh", **kw)

    def test_build_priority_wins_when_room_frees(self):
        self.task("busy", "busy", state="running", threads=6)
        self.task("sims", "sim", threads=2)
        self.task("build", "build", submitted=2, priority=10, threads=8)
        self.d.schedule()
        self.assertEqual(self.launched, [])
        self.d.store.update("tasks", dict(job="busy", name="busy"), state="done")
        self.d.schedule()
        self.assertEqual(self.launched, [("build", "build", "desktop")])
        self.assertIsNone(self.d.store.one("SELECT reason FROM tasks WHERE job='build'")["reason"])

    def test_reserved_host_blocks_sim_but_other_host_runs(self):
        self.remote(cpus=2, groups=[[0, 1]])
        self.task("busy", "busy", state="running", threads=6)
        self.task("build", "build", priority=10, threads=8)
        self.task("sims", "sim", submitted=2, threads=2)
        self.d.schedule()
        self.assertEqual(self.launched, [("sims", "sim", "far")])

    def test_equal_priorities_keep_fifo(self):
        self.task("busy", "busy", state="running", threads=6, est_h=1)
        self.task("first", "first", priority=2, jp=3, threads=8)
        self.task("later", "later", submitted=2, priority=5, threads=2, est_h=.1)
        self.d.schedule()
        self.assertEqual(self.launched, [])
        self.d.store.update("tasks", dict(job="busy", name="busy"), state="done")
        self.d.schedule()
        self.assertEqual(self.launched, [("first", "first", "desktop")])

    def test_same_job_sequence_is_fifo(self):
        self.task("job", "first", threads=8)
        self.task("job", "second", threads=2)
        self.d.schedule()
        self.assertEqual(self.launched, [("job", "first", "desktop")])

    def test_waiting_reason_in_status(self):
        self.task("busy", "busy", state="running", threads=6)
        self.task("build", "build", priority=10, threads=8)
        self.d.schedule()
        output = io.StringIO()
        with contextlib.redirect_stdout(output):
            cli.main(["--state", str(self.paths.state), "--cache", str(self.paths.cache),
                      "--runs", str(self.paths.runs), "--hosts", str(self.paths.hosts), "status"])
        self.assertIn("build/build: waiting for 8 threads on desktop (reserved)", output.getvalue())

    def test_backfill_requires_finish_before_start(self):
        self.task("busy", "busy", state="running", threads=6, est_h=1)
        self.task("build", "build", priority=10, threads=8)
        self.task("sims", "unknown", submitted=2, priority=0, threads=2)
        self.task("sims", "long", priority=1, threads=2, est_h=2)
        self.task("sims", "short", priority=2, threads=2, est_h=.5)
        self.d.schedule()
        self.assertEqual(self.launched, [("sims", "short", "desktop")])

    def test_unknown_or_overdue_running_estimate_blocks_backfill(self):
        for estimate in (None, .1):
            with self.subTest(estimate=estimate):
                self.d.store.x("DELETE FROM tasks")
                self.launched.clear()
                self.task("busy", "busy", state="running", threads=6, est_h=estimate, started=0)
                self.task("build", "build", priority=10, threads=8)
                self.task("sims", "short", threads=2, est_h=.01)
                self.d.schedule()
                self.assertEqual(self.launched, [])

    def test_reservation_respects_host_constraints_and_l3(self):
        self.remote(cpus=16, groups=[list(range(16))], arch="aarch64")
        self.task("busy", "busy", state="running", threads=6)
        self.task("build", "build", priority=10, threads=8, local_only=True, arch="x86_64", l3=True)
        self.task("sims", "sim", submitted=2, threads=2)
        self.d.schedule()
        self.assertEqual(self.launched, [("sims", "sim", "far")])
        self.assertIn("on desktop (reserved)", self.d.store.one("SELECT reason FROM tasks WHERE job='build'")["reason"])

    def test_unready_dependencies_do_not_reserve(self):
        self.task("build", "build", priority=10, threads=8, deps=["missing"])
        self.task("sims", "sim", submitted=2, threads=2)
        self.d.schedule()
        self.assertEqual(self.launched, [("sims", "sim", "desktop")])

    def test_hosts_whitelist_limits_reservation(self):
        self.remote(cpus=16, groups=[list(range(16))])
        self.task("busy", "busy", state="running", threads=6)
        self.task("build", "build", priority=10, threads=8, hosts=["desktop"])
        self.task("sims", "sim", submitted=2, threads=2)
        self.d.schedule()
        self.assertEqual(self.launched, [("sims", "sim", "far")])
        self.assertIn("on desktop (reserved)", self.d.store.one("SELECT reason FROM tasks WHERE job='build'")["reason"])

    def test_l3_deadline_requires_a_group_to_clear(self):
        self.d.inv.hosts["desktop"].groups = [[0, 1, 2, 3], [4, 5, 6, 7]]
        self.task("busy", "short", state="running", threads=2, l3=True, group=0, est_h=.25)
        self.task("busy", "long", state="running", threads=2, l3=True, group=0, est_h=1)
        self.task("busy", "unknown", state="running", threads=2, l3=True, group=1)
        self.task("build", "build", priority=10, threads=4, l3=True)
        self.task("sims", "sim", threads=2, est_h=.5)
        self.d.schedule()
        self.assertEqual(self.launched, [("sims", "sim", "desktop")])

    def test_disabled_and_unopted_ec2_hosts_are_not_reserved(self):
        self.remote(cpus=16, groups=[list(range(16))], enabled=False)
        from dataclasses import replace
        self.d.inv.hosts["ec2"] = replace(self.d.inv.hosts["far"], name="ec2", kind="ec2", enabled=True)
        self.task("busy", "busy", state="running", threads=6)
        self.task("build", "build", priority=10, threads=8)
        self.d.schedule()
        self.assertEqual(self.launched, [])
        self.assertIn("on desktop (reserved)", self.d.store.one("SELECT reason FROM tasks WHERE job='build'")["reason"])

    def test_fifo_also_applies_below_an_existing_reservation(self):
        self.task("busy", "busy", state="running", threads=6, est_h=1)
        self.task("build", "build", priority=10, threads=8)
        self.task("sims", "first", threads=2, est_h=2)
        self.task("sims", "second", threads=2, est_h=.1)
        self.d.schedule()
        self.assertEqual(self.launched, [])
