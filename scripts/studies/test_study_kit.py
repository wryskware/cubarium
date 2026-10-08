"""Tiny authored readouts; tests exercise mechanisms, never tuned defaults.

Run: python3 -m unittest discover -s scripts/studies -p test_study_kit.py
"""

import copy
import csv
import io
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import tomllib
import unittest
from contextlib import redirect_stdout

sys.path.insert(0, str(Path(__file__).resolve().parent))
import batch
import score


def write_csv(path, rows):
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", newline="") as stream:
        writer = csv.DictWriter(stream, list(rows[0]))
        writer.writeheader()
        writer.writerows(rows)


def fixture(root, seed=1):
    (root / "run").mkdir(parents=True)
    (root / "run/DONE").write_text("6\n")
    census, vitals = [], []
    for index, minute in enumerate((0, 2, 4, 6)):
        census.append(dict(sim_min=minute, seed=seed, founders_frondgrazer=(2, 8, 2, 1)[index], eggs_frondgrazer=0, founders_lanternjaw=0, eggs_lanternjaw=2, stands_turf=2, stands_flower=(1, 0, 0, 0)[index], meadow_patch_max=(1, 2, 4, 8)[index], grove_patch_max=2, recruits_vaulttree=index, kills_chorister_frondgrazer=index, kills_lanternjaw_frondgrazer=0, riders_now=0))
        for lineage in ("frondgrazer", "lanternjaw"):
            browser = lineage == "frondgrazer"
            vitals.append(dict(minute=minute, lineage=lineage, n=census[-1][f"founders_{lineage}"], breeding_eligible_n=int(minute == 2 and browser), fullness_p10=(0.9, 0.8, 0.1, 0.8)[index] if browser else "", fullness_p50=0.8 if browser else "", awake_body_s=(0, 1, 1, 3)[index] if browser else 0, energy_body_hours=1 if browser else 0, net_energy_per_body_hour=index if browser else "", moving_share=(0, 1, 0, 1)[index] if browser else "", stuck_n=int(browser and minute >= 4), stuck_observed_n=(0, 0, 2, 4)[index] if browser else 0))
    write_csv(root / "census.csv", census)
    write_csv(root / "vitals/vitals.csv", vitals)
    lives = [dict(lineage="frondgrazer", native="true", origin="observed", born_minute=4, died_minute="", cause=""), dict(lineage="frondgrazer", native="true", origin="born", born_minute=4, died_minute=5, cause="killed_by_lanternjaw")]
    write_csv(root / "vitals/life.csv", lives)


class StudyKitTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        with score.DEFAULT_CONFIG.open("rb") as stream:
            self.cfg = tomllib.load(stream)
        self.cfg["population"].update(k_hi=2.0, k_peak=2.0)
        self.cfg["targets"].update(fullness_p10=0.5, moving_share=0.5, voxel_m=0.5, meadow_m2=4.0, grove_m2=2.0, vaulttree_recruits=6.0, hunts_per_hour_lo=10.0, hunts_per_hour_hi=40.0)
        self.cfg["reproduction"].update(eligible_deficit=0.2, ineligible_deficit=0.9)

    def tearDown(self):
        self.temporary.cleanup()

    def world(self, start=0, stop=6):
        fixture(self.root)
        return score.score_world(self.root, "base", 1, self.cfg, start, stop)

    def test_clipping_relative_bands_and_zero_reference(self):
        self.assertEqual([score.h(x) for x in (-2, 0, 0.25, 2)], [1, 1, 0.75, 0])
        self.assertEqual(score.d(2, 4, 8), 0.5)
        self.assertEqual(score.d(12, 4, 8), 0.5)
        self.assertEqual(score.d(100, 4, 8), 1)
        self.assertEqual(score.d(0, 0, 0), 0)
        self.assertEqual(score.d(1, 0, 0), 1)
        self.assertEqual(score.band_penalty([2, 1], 2, self.cfg["population"]["k_hi"]), 0.25)
        self.assertEqual(score.peak_penalty([2, 8, 1], 2, self.cfg["population"]["k_peak"]), 1)

    def test_reproduction_all_branches_follow_config(self):
        cfg = self.cfg["reproduction"]
        self.assertEqual(score.reproductive_penalty(1, False, cfg), 0)
        self.assertEqual(score.reproductive_penalty(0, True, cfg), cfg["eligible_deficit"])
        self.assertEqual(score.reproductive_penalty(0, False, cfg), cfg["ineligible_deficit"])
        self.assertIsNone(score.reproductive_penalty(0, None, cfg))

    def test_loss_preserves_first_roster_and_egg_bank(self):
        world = self.world(start=2)
        self.assertIsNotNone(world["P"], world["warnings"])
        self.assertEqual(world["components"]["L"]["lost"], ["plant:flower"])
        self.assertIn("lanternjaw", world["components"]["L"]["required_animals"])
        self.assertEqual(world["components"]["Z"]["weight"], 0)
        self.assertEqual(world["components"]["Z"]["contribution"], 0)

    def test_exact_transient_births_and_slow_lifecycle(self):
        world = self.world()
        j = world["components"]["J"]
        self.assertEqual(j["raw"]["frondgrazer"]["late_births"], 1)
        self.assertEqual(j["penalty"], 0)
        self.assertEqual(j["raw"]["lanternjaw"]["weight"], 0)
        self.assertEqual(j["raw"]["lanternjaw"]["flag"], "lifecycle untested")
        self.assertEqual(world["health"]["frondgrazer"]["births"], 1)
        self.assertEqual(world["health"]["frondgrazer"]["deaths"]["killed_by_lanternjaw"], 1)

    def test_observed_body_is_not_a_birth(self):
        fixture(self.root)
        life = score.read_csv(self.root / "vitals/life.csv", [])
        write_csv(self.root / "vitals/life.csv", life[:1])
        world = score.score_world(self.root, "base", 1, self.cfg)
        self.assertEqual(world["components"]["J"]["penalty"], self.cfg["reproduction"]["eligible_deficit"])
        self.assertEqual(world["health"]["frondgrazer"]["births"], 0)

    def test_exact_birth_at_midpoint_is_in_second_half(self):
        world = self.world(start=2)
        self.assertEqual(world["components"]["J"]["raw"]["frondgrazer"]["late_births"], 1)
        self.assertEqual(world["components"]["V"]["raw"], 1)
        self.assertAlmostEqual(world["components"]["F"]["penalty"], (score.h(0.1 / self.cfg["targets"]["fullness_p10"]) + score.h(0.8 / self.cfg["targets"]["fullness_p10"])) / 2)

    def test_late_weighting_condition_before_mean_and_peak(self):
        world = self.world()
        c = world["components"]
        self.assertEqual(c["A"]["raw"]["frondgrazer"]["penalty"], 0.25)
        self.assertEqual(c["K"]["raw"]["frondgrazer"]["penalty"], 1)
        self.assertEqual(c["V"]["raw"], 0.75)
        self.assertEqual(c["V"]["penalty"], score.h(0.75 / self.cfg["targets"]["moving_share"]))
        self.assertAlmostEqual(c["F"]["penalty"], (score.h(0.1 / self.cfg["targets"]["fullness_p10"]) + score.h(0.8 / self.cfg["targets"]["fullness_p10"])) / 2)
        self.assertAlmostEqual(c["S"]["penalty"], 2 / 6)
        self.assertEqual(c["M"]["raw"], 6 * self.cfg["targets"]["voxel_m"]**2)
        self.assertEqual(c["M"]["penalty"], score.h(c["M"]["raw"] / self.cfg["targets"]["meadow_m2"]))
        self.assertEqual(c["G_v"]["raw"], 3)
        self.assertEqual(c["H"]["raw"], 3 / (6 / 60))
        self.assertEqual(c["H"]["penalty"], 0)
        self.assertIsNone(c["Q"]["penalty"])
        self.assertEqual(c["Q"]["contribution"], 0)
        self.assertEqual(world["P"], sum(v["weight"] * v["penalty"] for v in c.values() if v["penalty"] is not None))

    def test_missing_failed_paused_and_bad_seed_are_invalid(self):
        self.assertIsNone(score.score_world(self.root, "base", 1, self.cfg)["P"])
        fixture(self.root)
        for marker in ("FAILED", "PAUSED"):
            (self.root / "run" / marker).touch()
            world = score.score_world(self.root, "base", 1, self.cfg)
            self.assertIsNone(world["P"])
            self.assertEqual(world["state"], marker.lower())
            (self.root / "run" / marker).unlink()
        self.assertIsNone(score.score_world(self.root, "base", 2, self.cfg)["P"])
        self.assertIsNone(score.score_world(self.root, "base", 1, self.cfg, state="failed")["P"])
        (self.root / "run/DONE").unlink()
        self.assertIsNone(score.score_world(self.root, "base", 1, self.cfg, state="done")["P"])

    def test_missing_vitals_is_explicit_partial_not_zero_evidence(self):
        fixture(self.root)
        (self.root / "vitals/vitals.csv").unlink()
        world = score.score_world(self.root, "base", 1, self.cfg)
        self.assertTrue(world["partial"])
        self.assertIsNone(world["components"]["V"]["penalty"])
        self.assertIsNone(world["components"]["F"]["penalty"])
        self.assertIn("missing vitals.csv", world["warnings"])

    def test_pairing_includes_invalid_and_missing_seeds(self):
        components = {term: dict(penalty=0.5) for term in score.COMPONENTS}
        worlds = []
        for seed in range(1, 9):
            worlds.append(dict(arm="base", seed=seed, P=3, components=components))
            candidate = copy.deepcopy(components)
            candidate["A"]["penalty"] = 0.25 if seed <= 6 else 0.75
            worlds.append(dict(arm="trial", seed=seed, P=2 if seed <= 6 else None, components=candidate))
        paired = score.pair_worlds(worlds, "base", self.cfg)["trial"]
        self.assertEqual((paired["wins"], paired["n"], paired["invalid"]), (6, 8, 2))
        self.assertEqual(paired["pairs"][0]["delta_P"], -1)
        self.assertEqual(paired["pairs"][0]["components"]["A"], -0.25)
        self.assertIsNone(paired["pairs"][-1]["delta_P"])
        self.assertEqual(paired["consistent"], ["A penalty is lower in 6/8 paired seeds."])
        worlds.pop()
        self.assertEqual(score.pair_worlds(worlds, "base", self.cfg)["trial"]["n"], 8)

    def test_report_reads_tsv_and_preserves_missing_run(self):
        fixture(self.root / "base/s1")
        (self.root / "batch.json").write_text(json.dumps(dict(arms={"base": [], "trial": []}, seeds=[1])))
        (self.root / "STATE.tsv").write_text("arm\tseed\tstate\tminute\nbase\t1\tdone\t6\ntrial\t1\tfailed\t\n")
        report = score.build_report(self.root, self.cfg, "base")
        self.assertIsNone(report["arms"]["trial"]["mean_P"])
        self.assertEqual(report["paired"]["trial"]["invalid"], 1)
        json.dumps(report, allow_nan=False)
        self.assertTrue(list(score.records(report)))
        with redirect_stdout(io.StringIO()) as stream:
            score.plain(report)
        self.assertIn("wins 0/1; invalid 1", stream.getvalue())
        self.assertIn("Health:", stream.getvalue())

    def test_seed_parser_and_checkpoint_lookup(self):
        self.assertEqual(batch.seed_list("2-4,7"), [2, 3, 4, 7])
        for bad in ("3-1", "1,1", "bad"):
            with self.assertRaises(ValueError):
                batch.seed_list(bad)
        checkpoint = self.root / "s7/run/ckpt/m000060"
        checkpoint.mkdir(parents=True)
        self.assertEqual(batch.start_seed(checkpoint), 7)

    def test_standalone_batch_queues_and_retains_failed_and_paused_runs(self):
        if len(os.sched_getaffinity(0)) < 2:
            self.skipTest("batch requires one CPU reserved")
        scripts = self.root / "scripts"
        (scripts / "studies").mkdir(parents=True)
        shutil.copyfile(batch.__file__, scripts / "studies/batch.py")
        shutil.copyfile(Path(batch.__file__).resolve().parents[1] / "run-study.py", scripts / "run-study.py")
        fake = self.root / "census"
        fake.write_text("""#!/usr/bin/env python3
import json, os, pathlib, sys
run = pathlib.Path(next(a[4:] for a in sys.argv if a.startswith('run=')))
run.mkdir(parents=True)
minute = 3
state = 'FAILED' if 'fail' in sys.argv else 'PAUSED' if 'pause' in sys.argv else 'DONE'
(run / state).write_text(str(minute))
(run / 'status').write_text(f'minute={minute} state={state.lower()}')
print(json.dumps({'cpus': sorted(os.sched_getaffinity(0))}))
sys.exit(2 if state == 'FAILED' else 0)
""")
        fake.chmod(0o755)
        config = self.root / "input.toml"
        config.touch()
        out = self.root / "out"
        completed = subprocess.run([sys.executable, str(scripts / "studies/batch.py"), str(out), "--bin", str(fake), "--arm", "base=", "--arm", "bad=fail", "--arm", "stopped=pause", "--config", str(config), "--seeds", "1-2", "--hours", "0.1", "--max-parallel", "1"], cwd=self.root, capture_output=True, text=True, timeout=10)
        self.assertEqual(completed.returncode, 0, completed.stderr)
        self.assertTrue((out / "DONE").exists())
        with (out / "STATE.tsv").open() as stream:
            states = list(csv.DictReader(stream, delimiter="\t"))
        self.assertEqual(len(states), 6)
        self.assertEqual([row["state"] for row in states], ["done", "failed", "paused"] * 2)
        manifest = json.loads((out / "batch.json").read_text())
        self.assertIsNone(manifest["commit"])
        for job in manifest["runs"]:
            observed = json.loads((out / job["arm"] / f's{job["seed"]}' / "census.csv").read_text())["cpus"]
            self.assertEqual(observed, manifest["l3_groups"][job["slot"] % len(manifest["l3_groups"])])
            self.assertEqual(job["command"][job["command"].index("chrt") + 1:job["command"].index("nice")], ["-i", "0"])


if __name__ == "__main__":
    unittest.main()
