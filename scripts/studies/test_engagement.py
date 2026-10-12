"""Small hand-built reducer checks, without running a simulation."""
import importlib.util
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location('engagement_task', Path(__file__).with_name('engagement-task.py'))
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class Reduction(unittest.TestCase):
    def test_counts_shares_median_and_missing_rows(self):
        import json
        cfg = {'episodes': 2, 'arms': [{'name': 'fixture'}]}
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.assertFalse(module.reduce(root, cfg))
            for seed, outcome, time in [(1, 'timeout', 4), (2, 'calf_killed', 2)]:
                row = dict(seed=seed, arm='fixture', outcome=outcome, time_s=time,
                           hound_wounds=0.2, hound_deaths=0, calf_wounds=0.5,
                           adult_wounds=0, encounters=1, tests=0, chases=1,
                           kills=int(outcome == 'calf_killed'), family_flee_ticks=2)
                path = root / 'fixture' / f's{seed}' / 'outcome.json'
                path.parent.mkdir(parents=True)
                path.write_text(json.dumps(row))
            self.assertTrue(module.reduce(root, cfg))
            header, values = [line.split('\t') for line in (root / 'summary.tsv').read_text().splitlines()]
            table = dict(zip(header, values))
            self.assertEqual(table['episodes'], '2')
            self.assertEqual(table['calf_killed_n'], '1')
            self.assertEqual(table['calf_killed_share'], '0.5')
            self.assertEqual(table['median_time_s'], '3.0')
            self.assertEqual(table['hound_wounds'], '0.4')
            self.assertEqual(len((root / 'fixture.jsonl').read_text().splitlines()), 2)


if __name__ == '__main__':
    unittest.main()
