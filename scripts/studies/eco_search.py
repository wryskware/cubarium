#!/usr/bin/env python3
"""Ecology search: a separable CMA-ES over a genome, scored by score.py (stdlib only).

The search is a thin outer loop over the tools that already run and score worlds:
each generation is one batch.py batch (arm `base` = the unchanged world, plus lambda
candidates, on that generation's seeds: common random numbers within a generation), and
score.py --json --baseline base orders the arms by the objective profile it reports.
The optimiser sees only that order: feasible arms by soft rank, infeasible ones after
every feasible one by losses then soft rank, unrankable ones last (score.py's ranking).

Genes are the search files' `[[gene]]` multipliers (config/training/search/*.toml). The
census binary expands and decodes them over each run's own configs
(`voxel_census genome=FILE gene=NAME=VALUE`, `expand=OUT`), so wildcards, groups, ties and
inverse paths mean exactly what they mean in Rust. The search coordinates are each gene's
box normalised to [0, 1] (log for a multiplier), so --sigma0 is a fraction of the box.

Staged: OUT/status each generation; touch OUT/STOP to pause at the next generation
boundary (PAUSED); --resume continues from OUT/state.json (clearing STOP/PAUSED); DONE or
FAILED (with the reason) at the end. The wall budget counts every batch, generation 0
included; later batches are predicted from the measured time per wave of runs.
"""

import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import random
import re
import shlex
import shutil
import subprocess
import sys
import time
import tomllib

HERE = Path(__file__).resolve().parent
COMPONENTS = ("L", "J", "Q", "A", "K", "V", "Z", "M", "G_b", "G_v", "H", "F", "S")
STATE_VERSION = 1


def atomic(path, content):
    temporary = path.with_suffix(path.suffix + ".tmp")
    temporary.write_text(content)
    temporary.replace(path)


def seed_list(value):
    seeds = []
    for part in value.split(","):
        match = re.fullmatch(r"(\d+)(?:-(\d+))?", part.strip())
        if not match:
            raise ValueError(f"bad seed list: {value}")
        first, last = int(match[1]), int(match[2] or match[1])
        if last < first:
            raise ValueError("seed range is reversed")
        seeds.extend(range(first, last + 1))
    if len(set(seeds)) != len(seeds):
        raise ValueError("duplicate seed")
    return seeds


# ---------------------------------------------------------------- genes


def read_genes(path):
    """The searched genes of an expanded genome file: name, mode, box, x1 value."""
    with Path(path).open("rb") as stream:
        document = tomllib.load(stream)
    genes = []
    for gene in document.get("gene", []):
        lo, hi = (float(v) for v in gene["range"])
        mode = gene.get("mode", "scale")
        if gene.get("frozen", False) or hi <= lo:
            continue
        if mode == "scale" and lo <= 0:
            raise ValueError(f"gene {gene['name']}: a multiplier box must be positive")
        genes.append(dict(name=gene["name"], mode=mode, lo=lo, hi=hi,
                          start=float(gene.get("value", 1.0))))
    if not genes:
        raise ValueError(f"{path}: no searched genes")
    return genes


def to_unit(gene, value):
    value = min(gene["hi"], max(gene["lo"], value))
    if gene["mode"] == "scale":
        return (math.log(value) - math.log(gene["lo"])) / (math.log(gene["hi"]) - math.log(gene["lo"]))
    return (value - gene["lo"]) / (gene["hi"] - gene["lo"])


def from_unit(gene, u):
    u = min(1.0, max(0.0, u))
    if gene["mode"] == "scale":
        value = math.exp(math.log(gene["lo"]) + u * (math.log(gene["hi"]) - math.log(gene["lo"])))
    else:
        value = gene["lo"] + u * (gene["hi"] - gene["lo"])
    return min(gene["hi"], max(gene["lo"], value))


def decode(genes, x):
    return {gene["name"]: from_unit(gene, u) for gene, u in zip(genes, x)}


def gene_args(genome_file, values):
    return [f"genome={genome_file}"] + [f"gene={name}={value!r}" for name, value in values.items()]


# ---------------------------------------------------------------- sep-CMA-ES


class SepCMA:
    """Separable CMA-ES (Ros & Hansen 2008): diagonal covariance, standard recombination
    weights, cumulative step-size adaptation. Minimises; candidates live in [0, 1]^n and
    are repaired by clipping (the update uses the repaired steps)."""

    def __init__(self, mean, sigma, lam):
        n = len(mean)
        if lam < 2 or n < 1 or not sigma > 0:
            raise ValueError("need lambda >= 2, one dimension and sigma > 0")
        mu = lam // 2
        raw = [math.log(mu + 0.5) - math.log(i + 1) for i in range(mu)]
        total = sum(raw)
        weights = [w / total for w in raw]
        mueff = 1.0 / sum(w * w for w in weights)
        cs = (mueff + 2) / (n + mueff + 5)
        c1 = 2 / ((n + 1.3) ** 2 + mueff)
        cmu = min(1 - c1, 2 * (mueff - 2 + 1 / mueff) / ((n + 2) ** 2 + mueff))
        scale = (n + 2) / 3  # the separable variant's faster learning rates
        c1, cmu = c1 * scale, cmu * scale
        if c1 + cmu > 1:
            c1, cmu = c1 / (c1 + cmu), cmu / (c1 + cmu)
        self.s = dict(n=n, lam=lam, mu=mu, weights=weights, mueff=mueff, cs=cs,
                      cc=(4 + mueff / n) / (n + 4 + 2 * mueff / n), c1=c1, cmu=cmu,
                      damps=1 + 2 * max(0.0, math.sqrt((mueff - 1) / (n + 1)) - 1) + cs,
                      chiN=math.sqrt(n) * (1 - 1 / (4 * n) + 1 / (21 * n * n)),
                      mean=[min(1.0, max(0.0, m)) for m in mean], sigma=sigma, C=[1.0] * n,
                      pc=[0.0] * n, ps=[0.0] * n, updates=0)

    def to_dict(self):
        return json.loads(json.dumps(self.s))

    @classmethod
    def from_dict(cls, state):
        es = cls.__new__(cls)
        es.s = json.loads(json.dumps(state))
        return es

    def ask(self, rng, antithetic=False):
        s = self.s
        out = []
        for k in range(s["lam"]):
            if antithetic and k % 2 == 1:
                z = [-v for v in previous]
            else:
                z = [rng.gauss(0.0, 1.0) for _ in range(s["n"])]
            previous = z
            out.append([min(1.0, max(0.0, m + s["sigma"] * math.sqrt(c) * zi))
                        for m, c, zi in zip(s["mean"], s["C"], z)])
        return out

    def tell(self, candidates, order):
        """`order`: candidate indices best first (all of them, or at least mu)."""
        s = self.s
        n, mu, w = s["n"], s["mu"], s["weights"]
        if len(order) < mu:
            raise ValueError("need at least mu ranked candidates")
        old = s["mean"]
        steps = [[(candidates[i][d] - old[d]) / s["sigma"] for d in range(n)] for i in order[:mu]]
        yw = [sum(w[k] * steps[k][d] for k in range(mu)) for d in range(n)]
        s["mean"] = [min(1.0, max(0.0, old[d] + s["sigma"] * yw[d])) for d in range(n)]
        cs, cc, mueff = s["cs"], s["cc"], s["mueff"]
        s["ps"] = [(1 - cs) * s["ps"][d] + math.sqrt(cs * (2 - cs) * mueff) * yw[d] / math.sqrt(s["C"][d])
                   for d in range(n)]
        norm = math.sqrt(sum(v * v for v in s["ps"]))
        s["updates"] += 1
        hsig = norm / math.sqrt(1 - (1 - cs) ** (2 * s["updates"])) / s["chiN"] < 1.4 + 2 / (n + 1)
        s["pc"] = [(1 - cc) * s["pc"][d] + (math.sqrt(cc * (2 - cc) * mueff) * yw[d] if hsig else 0.0)
                   for d in range(n)]
        c1, cmu = s["c1"], s["cmu"]
        s["C"] = [(1 - c1 - cmu) * s["C"][d]
                  + c1 * (s["pc"][d] ** 2 + (0 if hsig else cc * (2 - cc)) * s["C"][d])
                  + cmu * sum(w[k] * steps[k][d] ** 2 for k in range(mu))
                  for d in range(n)]
        s["C"] = [max(c, 1e-12) for c in s["C"]]
        s["sigma"] *= math.exp((cs / s["damps"]) * (norm / s["chiN"] - 1))
        s["sigma"] = min(s["sigma"], 1.0)


# ---------------------------------------------------------------- scorer adapter


def candidate_order(report, names):
    """The scorer's ranking (feasibility, then losses, then soft rank; unrankable last)
    restricted to the candidates, as indices into `names`."""
    index = {name: k for k, name in enumerate(names)}
    order = [index[arm] for arm in report["ranking"] if arm in index]
    if sorted(order) != list(range(len(names))):
        raise ValueError("the score report does not rank every candidate")
    return order


def arm_row(report, arm):
    summary = report["arms"][arm]
    paired = report.get("paired", {}).get(arm, {})
    return dict(feasible=summary["feasible"], rankable=summary["rankable"],
                total_losses=summary["total_losses"], mean_soft_P=summary["mean_P"],
                sd_soft_P=summary["sd_P"], rank=summary["rank"],
                median_delta=paired.get("median_delta"), wins=paired.get("wins"), n=paired.get("n"),
                components={term: summary["components"][term]["median"] for term in COMPONENTS})


def improvement(report, arm, baseline="base"):
    """A feasible arm's soft rank minus the baseline's on the same seeds (lower is better);
    None when either is not rankable or the arm is infeasible."""
    a, b = report["arms"].get(arm), report["arms"].get(baseline)
    if not a or not b or not a["feasible"] or a["rank"] is None or b["rank"] is None:
        return None
    return a["rank"] - b["rank"]


def fmt(value):
    if value is None:
        return "NA"
    if isinstance(value, bool):
        return str(value).lower()
    if isinstance(value, float):
        return f"{value:.6g}"
    return str(value)


# ---------------------------------------------------------------- the search


class Failed(Exception):
    pass


class Search:
    def __init__(self, out, state, log):
        self.out, self.state, self.log = out, state, log
        self.a = state["args"]

    # -- plumbing

    def say(self, text):
        self.log.write(f"{time.strftime('%Y-%m-%d %H:%M:%S')} {text}\n")
        self.log.flush()

    def status(self, phase, extra=""):
        st, es = self.state, self.state["es"] or dict(sigma=float("nan"))
        best = st.get("best")
        line = (f"phase={phase} generation={st['generation']}/{self.a['generations']} "
                f"sigma={es['sigma']:.4g} wall_used_h={st['wall_used'] / 3600:.3f}/{self.a['wall_hours']} "
                f"best={best['arm'] + '@g' + str(best['generation']) + ' delta=' + fmt(best['delta']) if best else 'none'}"
                f"{' ' + extra if extra else ''}\n")
        atomic(self.out / "status", line)

    def save(self):
        atomic(self.out / "state.json", json.dumps(self.state, indent=1) + "\n")

    def seeds_for(self, generation):
        pool, k = self.a["pool"], self.a["seeds_per_gen"]
        return [pool[(generation * k + i) % len(pool)] for i in range(k)]

    def predicted(self, runs):
        per_wave = self.state.get("sec_per_wave")
        if per_wave is None:
            return 0.0
        return per_wave * math.ceil(runs / self.a["max_parallel"])

    def budget_allows(self, runs):
        return self.state["wall_used"] + self.predicted(runs) <= self.a["wall_hours"] * 3600

    def run_batch(self, directory, arms, seeds):
        """One batch.py batch in the foreground; returns its STATE rows."""
        resuming = (directory / "batch.json").is_file()
        directory.parent.mkdir(parents=True, exist_ok=True)
        a = self.a
        command = [sys.executable, str(HERE / "batch.py"), str(directory), "--bin", a["bin"],
                   "--hours", repr(a["hours"]), "--threads", str(a["threads"]),
                   "--max-parallel", str(a["max_parallel"]), "--required", a["required"],
                   "--stage", a["stage"]]
        if resuming:
            command = [sys.executable, str(HERE / "batch.py"), str(directory), "--bin", a["bin"],
                       "--resume", "--max-parallel", str(a["max_parallel"])]
        else:
            for name, extra in arms.items():
                command += ["--arm", f"{name}={shlex.join(extra)}"]
        if resuming:
            pass
        elif a["config"]:
            command += ["--config", a["config"], "--seeds", ",".join(map(str, seeds))]
        else:
            links = directory.parent / (directory.name + "-starts")
            if links.exists():
                shutil.rmtree(links)
            links.mkdir()
            for seed in seeds:
                (links / f"s{seed}").symlink_to(a["starts"][str(seed)])
            command += ["--starts", str(links / "s*")]
        started = time.monotonic()
        with (self.out / "batches.log").open("a") as stream:
            stream.write(f"\n# {directory.name}: {shlex.join(command)}\n")
            stream.flush()
            done = subprocess.run(command, stdout=stream, stderr=stream, stdin=subprocess.DEVNULL)
        elapsed = time.monotonic() - started
        self.state["wall_used"] += elapsed
        runs = len(arms) * len(seeds)
        self.state["sec_per_wave"] = elapsed / math.ceil(runs / a["max_parallel"])
        if done.returncode:
            raise Failed(f"batch.py refused or failed for {directory.name} (exit {done.returncode}); see batches.log")
        rows = {}
        for line in (directory / "STATE.tsv").read_text().splitlines()[1:]:
            arm, seed, state, _ = line.split("\t")
            rows[(arm, int(seed))] = state
        return rows

    def reason(self, directory, arm, seed):
        run = directory / arm / f"s{seed}"
        for name in ("run/FAILED", "run/PAUSED"):
            if (run / name).exists():
                return (run / name).read_text().strip()[:300]
        try:
            return (run / "err.log").read_text().strip().splitlines()[-1][:300]
        except (OSError, IndexError):
            return "no FAILED marker or log"

    def score(self, directory, report=None):
        a = self.a
        command = [sys.executable, str(HERE / "score.py"), str(directory), "--json", "--baseline", "base",
                   "--profile", a["profile"], "--config", a["fitness"]]
        if report:
            command += ["--report", str(report)]
        done = subprocess.run(command, capture_output=True, text=True, stdin=subprocess.DEVNULL)
        if done.returncode:
            raise Failed(f"score.py failed on {directory.name}: {done.stderr.strip()[-400:]}")
        return json.loads(done.stdout)

    def expand(self, values, target):
        """The census's own decode of `values`: the expanded genome to `target`, the set= lines back."""
        a = self.a
        source = f"config={a['config']}" if a["config"] else f"load={next(iter(a['starts'].values()))}"
        command = [a["bin"], "1", source, *gene_args(a["genome_source"], values), f"expand={target}"]
        done = subprocess.run(command, capture_output=True, text=True, stdin=subprocess.DEVNULL, timeout=600)
        if done.returncode:
            raise Failed(f"genome expansion failed: {done.stderr.strip()[-400:]}")
        return [line for line in done.stdout.splitlines() if line.startswith("set=")]

    # -- the loop

    def generation(self):
        st, a = self.state, self.a
        g = st["generation"]
        genes = st["genes"]
        es = SepCMA.from_dict(st["es"])
        rng = random.Random(f"{a['rng_seed']}:{g}")
        xs = es.ask(rng, a["antithetic"])
        values = [decode(genes, x) for x in xs]
        # The coordinates the runs actually used (the clamped values, back in the unit box).
        xs = [[to_unit(gene, v[gene["name"]]) for gene in genes] for v in values]
        names = [f"c{k}" for k in range(len(xs))]
        arms = {"base": []}
        for name, v in zip(names, values):
            arms[name] = gene_args(a["genome"], v)
        seeds = self.seeds_for(g)
        directory = self.out / "gens" / f"g{g:03d}"
        self.say(f"generation {g}: seeds {seeds}, {len(arms)} arms")
        self.status("running", f"seeds={','.join(map(str, seeds))}")
        rows = self.run_batch(directory, arms, seeds)
        bad = {key: state for key, state in rows.items() if state != "done"}
        if g == 0 and bad:
            # The staged liveness check runs inside each census; any first-generation run
            # that did not finish means the setup is wrong, not the candidate.
            detail = "; ".join(f"{arm}/s{seed} {state}: {self.reason(directory, arm, seed)}"
                               for (arm, seed), state in sorted(bad.items()))
            raise Failed(f"generation 0 failed the staged liveness check: {detail}")
        base_bad = [seed for (arm, seed) in bad if arm == "base"]
        if base_bad:
            raise Failed(f"generation {g}: base did not finish on seeds {base_bad}: "
                         + self.reason(directory, "base", base_bad[0]))
        report = self.score(directory)
        order = candidate_order(report, names)
        es.tell(xs, order)
        st["es"] = es.to_dict()
        # The log: one row per arm, base first.
        with (self.out / "generations.tsv").open("a") as stream:
            for arm in ["base"] + names:
                row = arm_row(report, arm)
                v = values[names.index(arm)] if arm != "base" else {gene["name"]: 1.0 for gene in genes}
                place = order.index(names.index(arm)) if arm != "base" else ""
                fields = [g, arm, ",".join(map(str, seeds)), place] + [fmt(v[gene["name"]]) for gene in genes]
                fields += [fmt(row[key]) for key in ("feasible", "rankable", "total_losses", "mean_soft_P",
                                                     "sd_soft_P", "rank", "median_delta", "wins")]
                fields += [fmt(row["components"][term]) for term in COMPONENTS]
                stream.write("\t".join(map(str, fields)) + "\n")
        # Best so far: the largest paired improvement over base among feasible candidates.
        improved = False
        for k, arm in enumerate(names):
            delta = improvement(report, arm)
            if delta is not None and (st["best"] is None or delta < st["best"]["delta"]):
                st["best"] = dict(generation=g, arm=arm, delta=delta, seeds=seeds,
                                  rank=report["arms"][arm]["rank"], genes=values[k])
                improved = True
        best_place = report["ranking"][0]
        self.say(f"generation {g} done: order {[names[i] for i in order]}, top {best_place}, "
                 f"sigma {es.s['sigma']:.4g}")
        st["generation"] = g + 1
        k = a["holdout_every"]
        if k and a["holdout"] and (g + 1) % k == 0:
            st["holdout_due"] = g
        self.save()
        if improved:
            self.write_best()

    def holdout(self):
        st, a = self.state, self.a
        g = st["holdout_due"]
        genes = st["genes"]
        arms = {"base": [], "mean": gene_args(a["genome"], decode(genes, st["es"]["mean"]))}
        if st["best"]:
            arms["best"] = gene_args(a["genome"], st["best"]["genes"])
        directory = self.out / "holdout" / f"h{g:03d}"
        self.say(f"holdout after generation {g}: seeds {a['holdout']}, arms {list(arms)}")
        self.status("holdout")
        rows = self.run_batch(directory, arms, a["holdout"])
        if any(state != "done" for (arm, _), state in rows.items() if arm == "base"):
            self.say(f"holdout {g}: base did not finish; no paired report")
        else:
            report = self.score(directory, self.out / "holdout" / f"h{g:03d}.md")
            with (self.out / "holdout.tsv").open("a") as stream:
                for arm in arms:
                    row = arm_row(report, arm)
                    stream.write("\t".join(map(str, [g, arm] + [fmt(row[key]) for key in (
                        "feasible", "rankable", "total_losses", "mean_soft_P", "sd_soft_P", "rank",
                        "median_delta", "wins", "n")])) + "\n")
        st["holdout_due"] = None
        self.save()

    def write_best(self):
        best = self.state["best"]
        sets = self.expand(best["genes"], self.out / "best.genome.toml")
        lines = ["# The best candidate so far: the largest paired soft-rank improvement over base",
                 "# among feasible candidates, on its generation's seeds (a screen, not a verdict).",
                 "# `set` is what the census resolved the genes to on this search's world.",
                 f"generation = {best['generation']}", f"arm = {json.dumps(best['arm'])}",
                 f"delta_rank_vs_base = {best['delta']!r}", f"seeds = {json.dumps(best['seeds'])}",
                 "set = [", *[f"  {json.dumps(s)}," for s in sets], "]", "", "[genes]"]
        lines += [f"{json.dumps(name)} = {value!r}" for name, value in best["genes"].items()]
        atomic(self.out / "best.toml", "\n".join(lines) + "\n")

    def run(self):
        st, a = self.state, self.a
        while True:
            if (self.out / "STOP").exists():
                atomic(self.out / "PAUSED", f"paused before generation {st['generation']}; --resume continues\n")
                self.status("paused")
                self.say("paused (STOP)")
                return
            if st.get("holdout_due") is not None:
                if self.budget_allows(len(a["holdout"]) * 3):
                    self.holdout()
                    continue
                st["holdout_due"] = None
                self.say("holdout skipped: wall budget")
            if st["generation"] >= a["generations"]:
                return self.finish("generations done")
            if st["generation"] > 0 and not self.budget_allows((a["lambda"] + 1) * a["seeds_per_gen"]):
                return self.finish("wall budget reached")
            self.generation()

    def finish(self, why):
        self.status("done", why.replace(" ", "_"))
        atomic(self.out / "DONE", why + "\n")
        self.say(f"DONE: {why}")


# ---------------------------------------------------------------- setup


def parse(argv):
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("out", type=Path)
    parser.add_argument("--resume", action="store_true", help="continue OUT/state.json")
    parser.add_argument("--genome", type=Path, help="a search file, config/training/search/*.toml")
    parser.add_argument("--profile", help="fitness-v2.toml profile, e.g. screen-2h")
    parser.add_argument("--fitness", type=Path, default=HERE.parents[1] / "config/training/fitness-v2.toml")
    parser.add_argument("--bin", type=Path, help="the voxel_census binary (with genome=/gene=)")
    source = parser.add_mutually_exclusive_group()
    source.add_argument("--config", type=Path)
    source.add_argument("--starts", metavar="GLOB")
    parser.add_argument("--required", type=Path)
    parser.add_argument("--lambda", dest="lam", type=int, default=8)
    parser.add_argument("--seeds-per-gen", type=int, default=2)
    parser.add_argument("--seed-pool", default="1-16")
    parser.add_argument("--holdout", default="", help="holdout seeds, never searched (e.g. 9-16)")
    parser.add_argument("--holdout-every", type=int, default=3)
    parser.add_argument("--generations", type=int)
    parser.add_argument("--wall-hours", type=float)
    parser.add_argument("--max-parallel", type=int)
    parser.add_argument("--threads", type=int, default=1)
    parser.add_argument("--stage", default="check_min=3 segment_min=15 keep=3")
    parser.add_argument("--sigma0", type=float, default=0.15, help="step size as a fraction of each gene's box")
    parser.add_argument("--antithetic", action="store_true")
    parser.add_argument("--rng-seed", type=int, default=1)
    parser.add_argument("--detach", action="store_true")
    return parser, parser.parse_args(argv)


def fresh_state(args, out):
    for name in ("genome", "profile", "bin", "required", "generations", "wall_hours", "max_parallel"):
        if getattr(args, name) is None:
            raise ValueError(f"--{name.replace('_', '-')} is required for a new search")
    if not (args.config or args.starts):
        raise ValueError("--config or --starts is required for a new search")
    if args.lam < 2 or args.seeds_per_gen < 1 or args.generations < 1 or args.wall_hours <= 0 or args.sigma0 <= 0:
        raise ValueError("need lambda >= 2, seeds-per-gen >= 1, generations >= 1, positive wall-hours and sigma0")
    pool, holdout = seed_list(args.seed_pool), seed_list(args.holdout) if args.holdout else []
    if set(holdout) & set(pool):
        pool = [seed for seed in pool if seed not in holdout]  # the holdout is never searched
    if len(pool) < args.seeds_per_gen:
        raise ValueError("the seed pool (minus the holdout) is smaller than seeds-per-gen")
    fitness_text = args.fitness.read_bytes()
    profile = tomllib.loads(fitness_text.decode())["profiles"].get(args.profile)
    if profile is None:
        raise ValueError(f"no profile {args.profile} in {args.fitness}")
    starts = None
    if args.starts:
        sys.path.insert(0, str(HERE))
        from batch import start_seed
        import glob
        starts = {}
        for matched in sorted(glob.glob(args.starts)):
            path = Path(matched).resolve(strict=True)
            starts[str(start_seed(path))] = str(path)
        missing = [seed for seed in pool + holdout if str(seed) not in starts]
        if missing:
            raise ValueError(f"no saved start for seeds {missing}")
    genome_source = args.genome.resolve(strict=True)
    a = dict(bin=str(args.bin.resolve(strict=True)), genome_source=str(genome_source),
             genome=str(out / "genome.toml"), profile=args.profile, fitness=str(args.fitness.resolve(strict=True)),
             fitness_sha256=hashlib.sha256(fitness_text).hexdigest(),
             config=str(args.config.resolve(strict=True)) if args.config else None, starts=starts,
             required=str(args.required.resolve(strict=True)), **{"lambda": args.lam},
             seeds_per_gen=args.seeds_per_gen, pool=pool, holdout=holdout, holdout_every=args.holdout_every,
             generations=args.generations, wall_hours=args.wall_hours, max_parallel=args.max_parallel,
             threads=args.threads, stage=args.stage, sigma0=args.sigma0, antithetic=args.antithetic,
             rng_seed=args.rng_seed, hours=profile["to_minute"] / 60)
    return dict(version=STATE_VERSION, args=a, generation=0, wall_used=0.0, sec_per_wave=None,
                best=None, holdout_due=None, genes=None, es=None)


def detach(out):
    if os.fork():
        return False
    os.setsid()
    if os.fork():
        os._exit(0)
    with open(os.devnull, "rb") as source, (out / "search.log").open("ab", buffering=0) as log:
        os.dup2(source.fileno(), 0)
        os.dup2(log.fileno(), 1)
        os.dup2(log.fileno(), 2)
    return True


def main(argv=None):
    parser, args = parse(argv)
    out = args.out.resolve()
    try:
        if args.resume:
            state = json.loads((out / "state.json").read_text())
            if state.get("version") != STATE_VERSION:
                raise ValueError("state.json is from another version of this script")
            for name in ("generations", "wall_hours", "max_parallel"):
                if getattr(args, name) is not None:
                    state["args"][name] = getattr(args, name)
            for marker in ("STOP", "PAUSED", "DONE", "FAILED"):
                (out / marker).unlink(missing_ok=True)
        else:
            if out.exists() and any(out.iterdir()):
                raise ValueError("OUT must be new or empty (or pass --resume)")
            out.mkdir(parents=True, exist_ok=True)
            state = fresh_state(args, out)
            header = ["generation", "arm", "seeds", "place"]
            state["pending_header"] = header
        if args.detach and not detach(out):
            print(f"detached search: {out} (status in {out / 'status'})")
            return
        with (out / "search.log").open("a") as log:
            search = Search(out, state, log)
            try:
                if state["genes"] is None:
                    # The census expands the genome over this search's world: wildcards,
                    # groups and tie = "each" resolved once, the gene list fixed for the run.
                    search.expand({}, Path(state["args"]["genome"]))
                    genes = read_genes(state["args"]["genome"])
                    state["genes"] = genes
                    mean = [to_unit(gene, gene["start"]) for gene in genes]
                    state["es"] = SepCMA(mean, state["args"]["sigma0"], state["args"]["lambda"]).to_dict()
                    columns = (state.pop("pending_header") + [gene["name"] for gene in genes]
                               + ["feasible", "rankable", "total_losses", "mean_soft_P", "sd_soft_P", "rank",
                                  "median_delta_vs_base", "wins"] + [f"{term}_median" for term in COMPONENTS])
                    atomic(out / "generations.tsv", "\t".join(columns) + "\n")
                    atomic(out / "holdout.tsv", "\t".join(["after_generation", "arm", "feasible", "rankable",
                                                           "total_losses", "mean_soft_P", "sd_soft_P", "rank",
                                                           "median_delta_vs_base", "wins", "n"]) + "\n")
                    search.save()
                    search.say(f"genes: {[gene['name'] for gene in genes]}")
                search.run()
            except Failed as error:
                search.say(f"FAILED: {error}")
                search.status("failed")
                atomic(out / "FAILED", f"{error}\n")
            except Exception as error:  # noqa: BLE001 - every exit leaves a marker
                search.say(f"FAILED: {type(error).__name__}: {error}")
                atomic(out / "FAILED", f"{type(error).__name__}: {error}\n")
                raise
    except (OSError, ValueError) as error:
        parser.exit(1, f"eco_search: {error}\n")


if __name__ == "__main__":
    main()
