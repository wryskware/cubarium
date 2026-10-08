"""Mechanism tests for eco_search.py's pure parts (no simulation): python3 -m unittest."""

import json
import math
from pathlib import Path
import random
import tempfile
import unittest

from eco_search import (COMPONENTS, SepCMA, candidate_order, decode, from_unit, gene_args, improvement,
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


class FakeSearch:
    """Search with the batch, scorer and census replaced: the staging logic alone."""

    @staticmethod
    def make(out, generations=3, holdout_every=2, wall_hours=10.0):
        import io
        from eco_search import Search, SepCMA
        genes = [dict(name="a", mode="scale", lo=0.5, hi=2.0, start=1.0)]
        state = dict(version=1, generation=0, wall_used=0.0, sec_per_wave=None, best=None,
                     holdout_due=None, genes=genes, es=SepCMA([0.5], 0.15, 2).to_dict(),
                     args=dict(generations=generations, wall_hours=wall_hours, max_parallel=3,
                               pool=[1, 2, 3], seeds_per_gen=1, holdout=[9], holdout_every=holdout_every,
                               rng_seed=1, antithetic=False, genome="/g.toml", **{"lambda": 2}))
        (out / "generations.tsv").write_text("")
        (out / "holdout.tsv").write_text("")
        search = Search(out, state, io.StringIO())
        search.batches = []

        def run_batch(directory, arms, seeds):
            search.batches.append((directory.name, list(arms), list(seeds)))
            state["wall_used"] += 3600.0
            state["sec_per_wave"] = 3600.0
            return {(arm, seed): "done" for arm in arms for seed in seeds}

        def score(directory, report=None):
            names = [arm for arm in search.batches[-1][1]]
            ranks = {name: float(k) for k, name in enumerate(reversed(names))}
            summary = lambda name: dict(feasible=True, rankable=True, total_losses=0, mean_P=ranks[name],
                                        sd_P=0.0, rank=ranks[name],
                                        components={t: dict(median=0.0) for t in COMPONENTS})
            return dict(ranking=sorted(names, key=ranks.get), arms={n: summary(n) for n in names},
                        paired={})

        search.run_batch, search.score = run_batch, score
        search.expand = lambda values, target: []
        return search


class StagingTest(unittest.TestCase):
    def test_stop_pauses_and_resume_continues_with_holdouts_on_schedule(self):
        with tempfile.TemporaryDirectory() as directory:
            out = Path(directory)
            search = FakeSearch.make(out)
            (out / "STOP").write_text("")
            search.run()
            self.assertTrue((out / "PAUSED").exists())
            self.assertEqual(search.batches, [])
            (out / "STOP").unlink()
            search.run()
            names = [name for name, _, _ in search.batches]
            self.assertEqual(names, ["g000", "g001", "h001", "g002"])
            self.assertTrue((out / "DONE").exists())
            self.assertEqual(search.batches[2][2], [9], "the holdout runs on the holdout seeds")
            self.assertEqual(search.batches[2][1], ["base", "mean", "best"])
            self.assertNotIn(9, [seed for name, _, seeds in search.batches if name.startswith("g") for seed in seeds])
            saved = json.loads((out / "state.json").read_text())
            self.assertEqual(saved["generation"], 3)

    def test_the_wall_budget_counts_generation_zero_and_predicts_the_next(self):
        with tempfile.TemporaryDirectory() as directory:
            out = Path(directory)
            search = FakeSearch.make(out, generations=10, holdout_every=0, wall_hours=1.5)
            search.run()
            self.assertEqual([name for name, _, _ in search.batches], ["g000"])
            self.assertIn("wall budget", (out / "DONE").read_text())


if __name__ == "__main__":
    unittest.main()
