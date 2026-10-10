"""Build overcommit uses fake launches and isolated state, never the live daemon."""

import contextlib
import io
import json
from types import SimpleNamespace
import unittest
from unittest.mock import patch

import test_fair
from labkit import cli
from labkit.core import Task
from labkit.hosts import Inventory, Usage, could_ever_fit, fits
from labkit.kinds.build import Build
from labkit.kinds.cargo_test import CargoTest
from labkit.top import Dashboard


class BuildLaneTests(unittest.TestCase):
    setUp = test_fair.FairTests.setUp
    launch = test_fair.FairTests.launch
    task = test_fair.FairTests.task

    def full_desktop(self, **kw):
        host = self.d.inv.hosts["desktop"]
        host.build_overcommit_threads = 8
        for key, value in kw.items():
            setattr(host, key, value)
        self.task("busy", "sim", state="running", threads=host.usable)
        return host

    def build(self, job="build", kind="build", **kw):
        return self.task(job, "build", threads=8, priority=10, meta=dict(kind=kind), **kw)

    def test_full_desktop_places_build_and_cargo_test(self):
        for kind in ("build", "cargo-test"):
            with self.subTest(kind=kind):
                host = self.full_desktop()
                task = self.build(kind=kind)
                self.d.schedule()
                self.assertEqual(self.launched, [("build", "build", "desktop")])
                self.assertEqual(self.d.usage()[host.name].threads, host.usable + task.threads)
                self.d.store.x("DELETE FROM tasks")
                self.d.store.x("DELETE FROM jobs")
                self.launched.clear()

    def test_second_build_waits_reserved(self):
        self.full_desktop()
        self.build("first")
        self.build("second", submitted=2)
        self.d.schedule()
        self.assertEqual(self.launched, [("first", "build", "desktop")])
        row = self.d.store.one("SELECT state, reason FROM tasks WHERE job='second'")
        self.assertEqual(row["state"], "pending")
        self.assertIn("(reserved)", row["reason"])

    def test_sim_never_uses_overcommit(self):
        host = self.full_desktop()
        usage = self.d.usage()[host.name]
        for kind in ("ab", "batch", "command"):
            with self.subTest(kind=kind):
                sim = Task(name="sim", argv=["true"], meta=dict(kind=kind))
                self.assertIsNone(fits(sim, host, usage))
                self.assertFalse(could_ever_fit(Task(name="wide", argv=["true"],
                                                   threads=host.usable + 1, meta=sim.meta), host))
        self.task("waiting", "sim", threads=1)
        self.d.schedule()
        self.assertEqual(self.launched, [])

    def test_memory_limits_block_overcommitted_build(self):
        host = self.full_desktop()
        task = self.build(mem_gb=host.max_mem_gb)
        self.d.schedule()
        self.assertEqual(self.launched, [])
        task.mem_gb = 2
        self.d.store.update("tasks", dict(job="build", name="build"), spec=json.dumps(task.to_dict()))
        host.reserve_mem_gb = 3
        with patch("labkit.daemon.mem_available_gb", return_value=4):
            self.d.schedule()
        self.assertEqual(self.launched, [])
        self.assertIsNone(fits(task, host, self.d.usage()[host.name], live_mem=4))

    def test_unconfigured_host_keeps_original_capacity(self):
        host = Inventory(dict(hosts=dict(desktop=dict(cpus=8, max_cpu_fraction=1,
                                                     reserve_cpus=0)))).hosts["desktop"]
        usage = Usage(threads=host.usable)
        task = self.build()
        self.assertIsNone(fits(task, host, usage))
        task.threads = host.usable + 1
        self.assertFalse(could_ever_fit(task, host))
        self.assertIsNone(fits(task, host, Usage()))

    def test_allowance_is_shared_and_external_load_is_charged(self):
        host = self.full_desktop(external_load=True, external_threads=2)
        usage = self.d.usage()[host.name]
        task = self.build()
        self.assertIsNone(fits(task, host, usage))
        task.threads = 3
        self.assertEqual(fits(task, host, usage), -1)
        usage.add(task, -1)
        task = Task.from_dict(task.to_dict())
        task.meta["kind"] = "cargo-test"
        self.assertEqual(fits(task, host, usage), -1)
        usage.add(task, -1)
        self.assertIsNone(fits(task, host, usage))

    def test_plan_time_capacity_and_l3_limits(self):
        host = self.full_desktop()
        task = self.build()
        task.threads = host.usable + host.build_overcommit_threads
        self.assertTrue(could_ever_fit(task, host))
        self.assertEqual(fits(task, host, Usage()), -1)
        task.threads += 1
        self.assertFalse(could_ever_fit(task, host))
        task.threads = host.usable + 1
        task.l3 = True
        self.assertFalse(could_ever_fit(task, host))
        self.assertIsNone(fits(task, host, Usage()))

    def test_hosts_and_top_show_overcommit_in_use(self):
        host = self.full_desktop()
        self.build()
        self.d.schedule()
        self.paths.hosts.write_text('[hosts.desktop]\ncpus=8\nmax_cpu_fraction=1\n'
                                    'reserve_cpus=0\ngroups=[8]\nbuild_overcommit_threads=8\n')
        for brief in (False, True):
            output = io.StringIO()
            with contextlib.redirect_stdout(output):
                cli.print_hosts(self.d.store, self.paths, brief=brief)
            self.assertIn(f"{host.usable}/{host.usable} +8 build", output.getvalue())
        dashboard = Dashboard(self.paths)
        self.addCleanup(dashboard.close)
        rows = self.d.store.q("SELECT * FROM tasks")
        self.assertIn(f"thr {host.usable}/{host.usable} +8 build",
                      "\n".join(dashboard.hosts(rows, None)))

    def test_build_kinds_mark_serialized_tasks(self):
        ctx = self.d.ctx
        spec = dict(repo=str(self.paths.repo), commit="a" * 40, target="fixture",
                    arch=ctx.arch, package="fixture", bin="fixture", threads=8, mem_gb=1,
                    profile="debug", command=["cargo", "test"])
        for kind in (Build(), CargoTest()):
            with self.subTest(kind=kind.name), patch.object(ctx, "artifact", return_value=None), \
                    patch("labkit.kinds.cargo_test.source_key", return_value="source"):
                task = kind.plan(SimpleNamespace(spec=spec, state={}), ctx)[0]
                self.assertEqual(Task.from_dict(json.loads(json.dumps(task.to_dict()))).meta["kind"],
                                 kind.name)


if __name__ == "__main__":
    unittest.main()
