import contextlib
import importlib.util
import io
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch


def load(name):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(name + ".py"))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class DriverTests(unittest.TestCase):
    def test_simulation_has_one_configurable_long_timeout_and_idle_priority(self):
        d = load("run_batches")
        cmd = d.command(Path("/runner"), Path("/binary"), 7, ["run"], 12345)
        self.assertEqual(cmd[:2], ["timeout", "12345"])
        self.assertEqual(cmd.count("timeout"), 1)
        self.assertIn("chrt", cmd)
        self.assertEqual(cmd[cmd.index("-n") + 1], "19")
        self.assertEqual(cmd[-1], "run")

    def test_zero_mechanisms_complete_without_retry_or_abort(self):
        d = load("run_batches")
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "runs").mkdir()
            def run(slot, args, out, err, **kwargs):
                stage = Path(next(a[4:] for a in args if a.startswith("run=")))
                stage.mkdir()
                (stage / "DONE").touch()
                out.write_text("bwl,both,360,bw_killed_sp=0,lure_visits=0\n")
                return 0
            with patch.object(d, "run_simulation", side_effect=run) as call:
                d.one(root, Path("/binary"), Path("/runner"), 2, "both", 5, Path("/found"), 12345)
                self.assertEqual(call.call_count, 1)
                args = call.call_args.args[1]
                self.assertFalse(any(a.startswith("resume=") for a in args))

    def test_failed_founding_and_missing_seeds_still_have_table_rows(self):
        t = load("table")
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "s1.found.err").write_text("founding failed")
            with patch("sys.argv", ["table.py", str(root)]), contextlib.redirect_stdout(io.StringIO()) as captured:
                t.main()
            rows = [s for s in captured.getvalue().splitlines() if " | " in s][1:]
            self.assertEqual(len(rows), len(t.ARMS) * len(t.SEEDS))
            self.assertTrue(any(s.startswith("both s1 |") for s in rows))
            self.assertTrue(all("MISSING" in s for s in rows))

    def test_table_spaces_unique_windows_and_weighted_nectar(self):
        t = load("table")
        with tempfile.TemporaryDirectory() as tmp:
            p = Path(tmp) / "both-s1.out"
            p.write_text("stocks,both,120,bellwing=17 lanternjaw=4 seedporter=2\n"
                         "bwl,both,5,bw_hours=2,nectar_per_bwh=0.01,lure_visits=3\n"
                         "bwl,both,10,bw_hours=1,nectar_per_bwh=0.04,lure_visits=4\n"
                         "bwl,both,10,bw_hours=1,nectar_per_bwh=0.04,lure_visits=4\n")
            stocks, sums, _ = t.parse(p)
            self.assertEqual(stocks[120]["bellwing"], "17")
            self.assertEqual(sums["lure_visits"], 7)
            self.assertAlmostEqual(t.ratio(sums["nectar"], sums["bw_hours"]), 0.02)


if __name__ == "__main__":
    unittest.main()
