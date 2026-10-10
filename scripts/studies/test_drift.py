"""Tiny hand-built run fixtures for the drift reducer."""

import csv
import json
import math
from pathlib import Path
import tempfile
import unittest

import drift


def fixture(rho, energy, offtake, kills):
    return dict(from_minute=10, to_minute=40, hours=.5, clipped=False,
                lineages=[dict(lineage='predator', rho=rho, rho_E=energy),
                          dict(lineage='prey', rho=None, rho_E=None)],
                world=dict(leaf_offtake=offtake, hunts=[
                    dict(predator='predator', prey='prey', kills_per_hour=kills / 2),
                    dict(predator='predator', prey='other', kills_per_hour=kills / 2)]))


class DriftReducerTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)

    def put(self, arm, run, data):
        path = self.root / arm / run / 'drift.json'
        path.parent.mkdir(parents=True)
        path.write_text(json.dumps(data))
        return path

    def test_fixture_means_sample_sd_and_outputs(self):
        self.put('candidate', 's1', fixture(-1, 2, .25, 2))
        self.put('candidate', 's2', fixture(3, 6, .75, 6))
        self.put('control', 's1', fixture(0, None, None, 0))
        rows = drift.aggregate(self.root)
        predator = next(row for row in rows if row['arm'] == 'candidate' and row['lineage'] == 'predator')
        self.assertEqual(predator['rho_mean'], 1)
        self.assertAlmostEqual(predator['rho_sd'], math.sqrt(8))
        self.assertEqual(predator['rho_E_mean'], 4)
        self.assertEqual(predator['offtake_mean'], .5)
        self.assertEqual(predator['kills_per_world_hour_mean'], 4)
        prey = next(row for row in rows if row['arm'] == 'candidate' and row['lineage'] == 'prey')
        self.assertIsNone(prey['rho_mean'])
        self.assertEqual(prey['rho_n'], 0)
        self.assertEqual(prey['kills_per_world_hour_mean'], 0)
        self.assertEqual(prey['offtake_mean'], predator['offtake_mean'])
        control = next(row for row in rows if row['arm'] == 'control' and row['lineage'] == 'predator')
        self.assertIsNone(control['rho_sd'])
        self.assertIsNone(control['offtake_mean'])
        drift.main([str(self.root)])
        with (self.root / 'drift.csv').open(newline='') as stream:
            csv_rows = list(csv.DictReader(stream))
        self.assertEqual(len(csv_rows), 4)
        self.assertIn('candidate predator 2', (self.root / 'drift.txt').read_text())

    def test_rejects_mixed_or_clipped_windows(self):
        self.put('arm', 's1', fixture(0, 0, None, 0))
        other = fixture(0, 0, None, 0)
        other.update(to_minute=70, hours=1)
        path = self.put('arm', 's2', other)
        with self.assertRaisesRegex(ValueError, 'window differs'):
            drift.aggregate(self.root)
        other = fixture(0, 0, None, 0)
        other['clipped'] = True
        path.write_text(json.dumps(other))
        with self.assertRaisesRegex(ValueError, 'clipped'):
            drift.aggregate(self.root)

    def test_rejects_nonfinite_metric(self):
        self.put('arm', 's1', fixture(float('nan'), 0, None, 0))
        with self.assertRaisesRegex(ValueError, 'finite number'):
            drift.aggregate(self.root)


if __name__ == '__main__':
    unittest.main()
