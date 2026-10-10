"""Outside load comes from CPU accounting, not loadavg: fake /proc/stat and cgroup files only."""

import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from labkit.daemon import Daemon


class Probe:
    """Just enough of a Daemon for `external_cpu`."""
    external_cpu = Daemon.external_cpu

    def __init__(self, root):
        self.PROC_STAT = str(root / "stat")
        self.LAB_CGROUPS = str(root / "cg" / "*" / "cpu.stat")


class CpuLoadTests(unittest.TestCase):
    def setUp(self):
        self.root = Path(tempfile.mkdtemp())
        (self.root / "cg").mkdir()
        self.probe = Probe(self.root)
        self.tick = os.sysconf("SC_CLK_TCK")

    def write(self, busy_s, units_s):
        # user nice system idle iowait irq softirq steal: all busy time in `nice`.
        ticks = int(busy_s * self.tick)
        (self.root / "stat").write_text(f"cpu  0 {ticks} 0 999999 0 0 0 0 0 0\n")
        for name, used in units_s.items():
            d = self.root / "cg" / name
            d.mkdir(exist_ok=True)
            (d / "cpu.stat").write_text(f"usage_usec {int(used * 1e6)}\nuser_usec 0\n")

    def sample(self, at):
        with patch("labkit.daemon.time.monotonic", return_value=at):
            return self.probe.external_cpu()

    def test_labd_own_cpu_is_not_outside_load(self):
        self.write(0, {"lab-a.service": 0, "lab-b.service": 0})
        self.assertIsNone(self.sample(0.0))
        # 10 s: 20 core-seconds busy, all of it in labd's two units.
        self.write(20, {"lab-a.service": 10, "lab-b.service": 10})
        self.assertAlmostEqual(self.sample(10.0), 0.0, places=6)

    def test_cpu_outside_labd_units_is_counted(self):
        self.write(0, {"lab-a.service": 0})
        self.sample(0.0)
        self.write(30, {"lab-a.service": 10})
        self.assertAlmostEqual(self.sample(10.0), 2.0, places=6)

    def test_an_ended_unit_does_not_go_negative_and_a_new_one_counts(self):
        self.write(0, {"lab-a.service": 50})
        self.sample(0.0)
        (self.root / "cg" / "lab-a.service" / "cpu.stat").unlink()
        (self.root / "cg" / "lab-a.service").rmdir()
        self.write(10, {"lab-b.service": 10})
        self.assertGreaterEqual(self.sample(10.0), 0.0)

    def test_unreadable_stat_falls_back(self):
        self.assertIsNone(self.probe.external_cpu())


if __name__ == "__main__":
    unittest.main()
