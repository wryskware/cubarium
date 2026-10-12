"""Quick lane: own overcommit pool, no reservation wait, local first, time cap, `lab quick`.

Fake launches and isolated state, never the live daemon. Pool sizes and the time cap come
from this test's own host config, never from config/lab/hosts.toml.
"""

import contextlib
import io
import json
from pathlib import Path
from types import SimpleNamespace
import unittest
from unittest.mock import patch

import test_fair
from labkit import cli
from labkit.core import Task
from labkit.daemon import QUICK_LIMIT
from labkit.hosts import Usage, could_ever_fit, fits, prefers_local, quick_timeout_h
from labkit.kinds.command import Command
from labkit.store import read_events


class QuickLaneTests(unittest.TestCase):
    setUp = test_fair.FairTests.setUp
    launch = test_fair.FairTests.launch
    task = test_fair.FairTests.task
    remote = test_fair.FairTests.remote

    def pools(self, build=8, quick=6):
        host = self.d.inv.hosts["desktop"]
        host.build_overcommit_threads = build
        host.quick_overcommit_threads = quick
        return host

    def full_desktop(self):
        host = self.pools()
        self.task("busy", "sim", state="running", threads=host.usable)
        return host

    def quick(self, job="quick", threads=None, **kw):
        host = self.d.inv.hosts["desktop"]
        return self.task(job, "q", threads=threads or host.quick_overcommit_threads, quick=True, **kw)

    def build(self, job="build", threads=None, **kw):
        host = self.d.inv.hosts["desktop"]
        return self.task(job, "build", threads=threads or host.build_overcommit_threads, priority=10,
                         meta=dict(kind="build"), **kw)

    def state(self, job):
        return self.d.store.one("SELECT state, reason FROM tasks WHERE job=?", job)

    # (a) ------------------------------------------------------------------
    def test_full_desktop_launches_quick_from_its_pool_only(self):
        host = self.full_desktop()
        task = self.quick()
        self.task("normal", "sim", submitted=2, threads=1)
        self.d.schedule()
        self.assertEqual(self.launched, [("quick", "q", "desktop")])
        usage = self.d.usage()[host.name]
        self.assertEqual(usage.threads, host.usable + task.threads)
        self.assertEqual(usage.quick, host.quick_overcommit_threads)
        self.assertEqual(self.state("normal")["state"], "pending")
        # The pool is full: one more quick thread does not fit, and nothing wider ever could.
        more = Task(name="more", argv=["true"], quick=True)
        self.assertIsNone(fits(more, host, usage))
        self.assertFalse(could_ever_fit(Task(name="wide", argv=["true"], quick=True,
                                             threads=host.usable + host.quick_overcommit_threads + 1), host))

    def test_normal_task_never_uses_quick_pool(self):
        host = self.full_desktop()
        self.assertIsNone(fits(Task(name="sim", argv=["true"]), host, self.d.usage()[host.name]))
        self.assertFalse(could_ever_fit(Task(name="wide", argv=["true"], threads=host.usable + 1), host))

    # (b) ------------------------------------------------------------------
    def test_build_and_quick_pools_are_independent(self):
        for first, second in (("build", "quick"), ("quick", "build")):
            with self.subTest(first=first):
                host = self.full_desktop()
                getattr(self, first)(first, state="running")
                usage = self.d.usage()[host.name]
                self.assertEqual(usage.threads, host.usable + host.pool(first))
                # Its own pool is spent: another task of the first lane waits ...
                again = Task(name="again", argv=["true"], quick=first == "quick",
                             meta=dict(kind="build") if first == "build" else {})
                self.assertIsNone(fits(again, host, usage))
                # ... while the other lane still gets its whole pool, and not one thread more.
                getattr(self, second)(second)
                self.d.schedule()
                self.assertEqual(self.launched, [(second, second if second == "build" else "q", "desktop")])
                usage = self.d.usage()[host.name]
                self.assertEqual(usage.threads, host.usable + host.pool("build") + host.pool("quick"))
                self.assertIsNone(fits(Task(name="extra", argv=["true"], quick=True), host, usage))
                self.assertIsNone(fits(Task(name="extra", argv=["true"], meta=dict(kind="build")), host, usage))
                self.d.store.x("DELETE FROM tasks")
                self.d.store.x("DELETE FROM jobs")
                self.launched.clear()

    def test_a_lane_cannot_borrow_the_other_pool(self):
        host = self.full_desktop()
        # One quick thread runs above usable: most of the quick pool is idle, but a build
        # may still overflow only by its own pool.
        self.quick("q1", threads=1, state="running")
        wide = Task(name="wide", argv=["true"], meta=dict(kind="build"),
                    threads=host.build_overcommit_threads + 1)
        self.assertIsNone(fits(wide, host, self.d.usage()[host.name]))
        wide.threads = host.build_overcommit_threads
        self.assertEqual(fits(wide, host, self.d.usage()[host.name]), -1)

    def test_label_splits_build_and_quick(self):
        host = self.full_desktop()
        self.build(state="running")
        self.quick(threads=1, state="running")
        self.paths.hosts.write_text(
            f'[hosts.desktop]\ncpus=8\nmax_cpu_fraction=1\nreserve_cpus=0\ngroups=[8]\n'
            f'build_overcommit_threads={host.build_overcommit_threads}\n'
            f'quick_overcommit_threads={host.quick_overcommit_threads}\n')
        output = io.StringIO()
        with contextlib.redirect_stdout(output):
            cli.print_hosts(self.d.store, self.paths, brief=True)
        self.assertIn(f"{host.usable}/{host.usable} +{host.build_overcommit_threads} build +1 quick",
                      output.getvalue())

    # (c) ------------------------------------------------------------------
    def test_quick_prefers_local_and_normal_keeps_most_free(self):
        host = self.pools()
        self.remote()                       # same size as the desktop, kind ssh
        self.task("busy", "sim", state="running", threads=2)   # far now has more free threads
        self.quick("quick", threads=1)
        self.task("normal", "sim", submitted=2, threads=1)
        self.task("localin", "sim", submitted=3, threads=1, meta=dict(local_inputs=True))
        self.d.schedule()
        self.assertEqual(self.launched, [("quick", "q", "desktop"), ("normal", "sim", "far"),
                                         ("localin", "sim", "desktop")])
        self.assertLess(self.d.usage()["far"].threads, self.d.usage()[host.name].threads)

    def test_quick_falls_to_remote_only_when_no_local_fit(self):
        host = self.full_desktop()
        self.remote()
        self.quick("hog", state="running")    # the local quick pool is spent
        self.quick("quick", threads=1)
        self.d.schedule()
        self.assertEqual(self.launched, [("quick", "q", "far")])

    def test_command_marks_local_inputs(self):
        ctx = self.d.ctx
        repo = Path(self.temp.name)
        (repo / "bin").mkdir()
        (repo / "bin/tool").write_text("")
        cases = [(["true"], {}, False),
                 (["bin/tool", "x"], dict(cwd=str(repo)), True),
                 (["bin/tool"], {}, True),                     # relative to the local default cwd
                 ([str(repo / "bin/tool")], dict(hosts=["far"]), True),
                 (["tool", f"--config={repo / 'bin/tool'}"], dict(hosts=["far"]), True),
                 (["{out}/x", "seed=3"], dict(hosts=["far"]), False)]
        for argv, extra, expected in cases:
            with self.subTest(argv=argv), patch.object(ctx, "repo", repo), \
                    patch("labkit.kinds.command.Path.home", return_value=repo):
                spec = dict(kind="command", argv=argv, _cwd=str(repo), **extra)
                task = Command().plan(SimpleNamespace(spec=spec, state={}), ctx)[0]
                self.assertEqual(prefers_local(task), expected)

    # (d) ------------------------------------------------------------------
    def test_reservation_blocks_normal_but_not_quick(self):
        host = self.pools()
        self.task("busy", "sim", state="running", threads=host.usable - 1)
        self.task("large", "big", threads=host.usable, jp=5)
        self.quick("quick", threads=1, submitted=2)
        self.task("normal", "sim", submitted=3, threads=1)
        self.d.schedule()
        self.assertEqual(self.launched, [("quick", "q", "desktop")])
        self.assertIn("(reserved)", self.state("large")["reason"])
        self.assertEqual(self.state("normal")["state"], "pending")

    def test_waiting_quick_holds_no_reservation(self):
        host = self.full_desktop()
        self.quick("hog", state="running")
        self.quick("quick", threads=1, jp=5)
        self.task("short", "sim", submitted=2, threads=1, est_h=0.01)
        self.d.store.update("tasks", dict(job="busy", name="sim"), state="done")
        self.d.schedule()
        self.assertEqual(self.state("quick")["state"], "running")
        self.assertEqual(self.state("short")["state"], "running")

    # (e) ------------------------------------------------------------------
    def test_timeout_is_capped(self):
        limit_min = 7
        self.assertEqual(quick_timeout_h(Task(name="a", argv=["true"]), limit_min), limit_min / 60)
        self.assertEqual(quick_timeout_h(Task(name="a", argv=["true"], timeout_h=5), limit_min), limit_min / 60)
        shorter = limit_min / 120
        self.assertEqual(quick_timeout_h(Task(name="a", argv=["true"], timeout_h=shorter), limit_min), shorter)

    def test_scheduler_caps_and_job_spec_quick_applies(self):
        self.pools()
        self.d.inv.lab["quick_timeout_min"] = 7
        self.quick("own", threads=1, timeout_h=5)
        self.task("byjob", "sim", submitted=2, threads=1)
        self.d.store.update("jobs", dict(id="byjob"), spec=json.dumps(dict(kind="command", quick=True)))
        launched = {}
        self.d.launch = lambda row, task, host, group: launched.setdefault(row["job"], task) and False
        self.d.schedule()
        self.assertEqual(set(launched), {"own", "byjob"})
        for task in launched.values():
            self.assertTrue(task.quick)
            self.assertEqual(task.timeout_h, 7 / 60)

    def running_quick(self, job, quick=True):
        cap = 7 / 60
        self.task(job, "q", state="running", threads=1, quick=quick, timeout_h=cap)
        self.d.store.update("tasks", dict(job=job, name="q"), unit=f"lab-{job}")
        return cap

    def test_hitting_the_cap_says_it_was_not_quick(self):
        cap = self.running_quick("slow")
        self.running_quick("fast")
        self.d.store.update("tasks", dict(job="fast", name="q"), started=1000 + cap * 1800)
        self.d.clock = lambda: 1000 + cap * 3600 + 1
        with patch.object(self.d.launchers["desktop"], "stop") as stop:
            self.d.quick_timeouts()
        stop.assert_called_once()
        row = self.state("slow")
        self.assertEqual(row["state"], "failed")
        self.assertTrue(row["reason"].startswith(QUICK_LIMIT))
        self.assertEqual(self.state("fast")["state"], "running")
        events = [e for e in read_events(self.paths.events)[0] if e["event"] == "task-failed"]
        self.assertTrue(any(e.get("task") == "q" and QUICK_LIMIT in e.get("reason", "") for e in events))

    def test_systemd_kill_at_the_cap_is_reported_as_quick_limit(self):
        for quick in (True, False):
            with self.subTest(quick=quick):
                job = f"killed-{quick}"
                cap = self.running_quick(job, quick=quick)
                row = self.d.store.one("SELECT * FROM tasks WHERE job=?", job)
                self.d.task_ended(row, dict(rc=-15, cancelled=True, ended=1000 + cap * 3600))
                state = self.state(job)
                if quick:
                    self.assertEqual(state["state"], "failed")
                    self.assertTrue(state["reason"].startswith(QUICK_LIMIT))
                else:
                    self.assertEqual(state["state"], "cancelled")
                    self.assertNotIn(QUICK_LIMIT, state["reason"])

    # (f) ------------------------------------------------------------------
    def test_quick_spec_and_command(self):
        cwd = Path(self.temp.name)
        spec = cli.quick_spec(["--", "python3", "-c", "print(1)"], cwd, name="probe")
        self.assertEqual(spec["kind"], "command")
        self.assertTrue(spec["quick"])
        self.assertEqual(spec["argv"], ["python3", "-c", "print(1)"])
        self.assertEqual(spec["cwd"], str(cwd))
        self.assertEqual(spec["threads"], 1)
        task = Command().plan(SimpleNamespace(spec=Command().validate(dict(spec), self.d.ctx), state={}),
                              self.d.ctx)[0]
        self.assertTrue(task.quick)
        self.assertEqual(task.cwd, str(cwd))
        self.assertEqual(cli.quick_spec(["true"], cwd, out="o")["out"], str(cwd / "o"))
        with self.assertRaises(Exception):
            cli.quick_spec(["--"], cwd)

    def test_lab_quick_spools_without_a_daemon(self):
        cwd = Path(self.temp.name) / "work"
        cwd.mkdir()
        output = io.StringIO()
        with contextlib.chdir(cwd), contextlib.redirect_stdout(output):
            rc = cli.main(["--state", str(self.paths.state), "--cache", str(self.paths.cache),
                           "--runs", str(self.paths.runs), "--hosts", str(self.paths.hosts),
                           "quick", "--name", "probe", "--threads", "2", "--", "echo", "--flag", "hi"])
        self.assertEqual(rc, 0)
        job_id, out = output.getvalue().splitlines()
        payload = json.loads((self.paths.state / "incoming" / f"{job_id}.json").read_text())
        spec = payload["spec"]
        self.assertEqual((spec["kind"], spec["quick"], spec["argv"], spec["cwd"], spec["threads"]),
                         ("command", True, ["echo", "--flag", "hi"], str(cwd), 2))
        self.assertEqual(out, f"out {self.paths.runs / job_id}")


if __name__ == "__main__":
    unittest.main()
