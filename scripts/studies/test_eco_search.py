"""Mechanism tests for eco_search.py's pure parts (no simulation): python3 -m unittest."""

import json
import math
from pathlib import Path
import random
import tempfile
import unittest

from eco_search import (SepCMA, candidate_order, decode, from_unit, gene_args, improvement,
                        read_genes, to_unit)


def quadratic(target):
    return lambda x: sum((a - b) ** 2 for a, b in zip(x, target))


def order_by(f, xs):
    return sorted(range(len(xs)), key=lambda k: f(xs[k]))


class SepCMATest(unittest.TestCase):
    def test_one_update_moves_the_mean_toward_the_better_samples(self):
        f = quadratic([0.9, 0.1, 0.6])
        es = SepCMA([0.5, 0.5, 0.5], 0.1, 8)
        xs = es.ask(random.Random(3))
        order = order_by(f, xs)
        before = list(es.s["mean"])
        mu = es.s["mu"]
        best_mean = [sum(xs[k][d] for k in order[:mu]) / mu for d in range(3)]
        worst_mean = [sum(xs[k][d] for k in order[-mu:]) / mu for d in range(3)]
        es.tell(xs, order)
        step = [a - b for a, b in zip(es.s["mean"], before)]
        toward = [a - b for a, b in zip(best_mean, worst_mean)]
        self.assertGreater(sum(a * b for a, b in zip(step, toward)), 0)

    def test_it_descends_a_toy_quadratic(self):
        target = [0.8, 0.2, 0.65, 0.35]
        f = quadratic(target)
        es = SepCMA([0.5] * 4, 0.15, 8)
        start = f(es.s["mean"])
        for generation in range(40):
            xs = es.ask(random.Random(generation))
            es.tell(xs, order_by(f, xs))
        self.assertLess(f(es.s["mean"]), start / 10)
        self.assertTrue(all(0.0 <= m <= 1.0 for m in es.s["mean"]))

    def test_lambda_two_and_antithetic_pairs_work(self):
        es = SepCMA([0.5, 0.5], 0.1, 2)
        xs = es.ask(random.Random(1), antithetic=True)
        self.assertAlmostEqual(xs[0][0] - 0.5, -(xs[1][0] - 0.5))
        es.tell(xs, [1, 0])
        self.assertTrue(all(math.isfinite(v) for v in es.s["mean"] + es.s["C"] + [es.s["sigma"]]))

    def test_resume_restores_the_state(self):
        f = quadratic([0.3, 0.7])
        es = SepCMA([0.5, 0.5], 0.2, 6)
        for generation in range(3):
            xs = es.ask(random.Random(generation))
            es.tell(xs, order_by(f, xs))
        saved = json.loads(json.dumps(es.to_dict()))
        back = SepCMA.from_dict(saved)
        a, b = es.ask(random.Random(99)), back.ask(random.Random(99))
        self.assertEqual(a, b)
        es.tell(a, order_by(f, a))
        back.tell(b, order_by(f, b))
        self.assertEqual(es.to_dict(), back.to_dict())


GENOME = '''
[[gene]]
name = "growth@turf"
path = ["flora.springturf.foliage_rate"]
range = [0.5, 2.0]
value = 1.0

[[gene]]
name = "upkeep@frondgrazer"
path = ["fauna.founders.frondgrazer.core.maintenance_per_s"]
range = [0.25, 4.0]
value = 1.0

[[gene]]
name = "pinned"
path = ["flora.decomposition"]
range = [0.1, 4.0]
value = 2.0
frozen = true

[[gene]]
name = "level"
path = ["flora.x"]
mode = "value"
range = [10.0, 20.0]
value = 12.0
'''


class GeneTest(unittest.TestCase):
    def genes(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "g.toml"
            path.write_text(GENOME)
            return read_genes(path)

    def test_an_expanded_genome_reads_its_searched_genes(self):
        genes = self.genes()
        self.assertEqual([g["name"] for g in genes], ["growth@turf", "upkeep@frondgrazer", "level"])
        self.assertEqual(genes[2]["mode"], "value")

    def test_multipliers_are_log_coordinates_and_round_trip(self):
        turf, upkeep, level = self.genes()
        self.assertAlmostEqual(to_unit(turf, 1.0), 0.5)  # x1 is the geometric centre of [0.5, 2]
        self.assertAlmostEqual(to_unit(upkeep, 1.0), 0.5)
        self.assertAlmostEqual(to_unit(level, 15.0), 0.5)
        for gene in (turf, upkeep, level):
            for u in (0.0, 0.25, 0.9, 1.0):
                self.assertAlmostEqual(to_unit(gene, from_unit(gene, u)), u)
        self.assertEqual(from_unit(turf, 1.7), 2.0)  # clamped into the box
        self.assertEqual(from_unit(turf, -1), 0.5)

    def test_decode_names_every_gene_for_the_census(self):
        genes = self.genes()
        values = decode(genes, [0.5, 1.0, 0.0])
        self.assertEqual(set(values), {"growth@turf", "upkeep@frondgrazer", "level"})
        args = gene_args("/g.toml", values)
        self.assertEqual(args[0], "genome=/g.toml")
        self.assertIn("gene=upkeep@frondgrazer=4.0", args)


def report(ranking, arms):
    return dict(ranking=ranking, arms=arms)


def arm(feasible, rank):
    return dict(feasible=feasible, rank=rank)


class ScorerAdapterTest(unittest.TestCase):
    def test_the_order_is_the_scorers_ranking_without_base(self):
        r = report(["c2", "base", "c0", "c1"], {})
        self.assertEqual(candidate_order(r, ["c0", "c1", "c2"]), [2, 0, 1])
        with self.assertRaises(ValueError):
            candidate_order(report(["c0", "base"], {}), ["c0", "c1"])

    def test_improvement_is_paired_against_base_and_needs_feasibility(self):
        r = report([], {"base": arm(True, 5.0), "c0": arm(True, 3.5), "c1": arm(False, 1.0),
                        "c2": arm(True, None)})
        self.assertEqual(improvement(r, "c0"), -1.5)
        self.assertIsNone(improvement(r, "c1"))
        self.assertIsNone(improvement(r, "c2"))


if __name__ == "__main__":
    unittest.main()
