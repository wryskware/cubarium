#!/usr/bin/env python3
"""Reduce the voxel replacement-control study: scripts/reduce_replacement.py [directory]

Reads the raw study outputs `full-<resident>-resident-seed<N>-noise<M>.txt` and prints
one markdown table row per direction (the resident species), collapsed over seeds,
noise seeds and sites. Standard library only.

The files live in `design/7_Research/assets/voxel-replacement/` by default; pass
another directory as the one argument.

What one file contains
----------------------

A file is one (resident, seed, noise) run of
`crates/cubarium-voxel-flora/examples/replacement.rs`. It holds one `resident only`
control arm, then for each of three predeclared introduction sites a
`resident plus newcomer` arm and a `newcomer with the resident excluded` arm. The
study arm is `resident plus newcomer`; the other two are its controls, so this
reducer counts only the study arm: 10 files x 3 sites = 30 arms per direction,
which is what `design/7_Research/voxel-round3-experiment-2026-09-16.md`
("Replacement study - 2026-09-18") reports (bloomcrown resident 30/30, umbrellafrond
resident 7/30).

Outcome lines
-------------

An arm ends in exactly one of these, quoted verbatim:

* completed: a line containing `**replacement completed**`, which carries the
  replacement time as `delivered its own package at t <s> s`.
* unresolved: a line containing `**unresolved at the stopping budget**`
  (also covers `recruitment NOT OBSERVED within <cap> s`). This is the only
  non-completion outcome the committed files print; the phrases `possible refuge`
  and a refusal line exist in the study's prose but not as per-arm result lines, so
  those counters stand at zero here rather than being inferred.
* possible refuge / refusal: counted only if a literal such line appears.

Nothing here adjusts a number: it reports what the files say.
"""
import glob
import os
import re
import statistics
import sys

DEFAULT_DIR = os.path.join(
    os.path.dirname(os.path.dirname(os.path.abspath(__file__))),
    "design",
    "7_Research",
    "assets",
    "voxel-replacement",
)

# The study arm; the two controls are not counted (see the module docstring).
STUDY_ARM = "resident plus newcomer"

DIRECTION_RE = re.compile(r"^full-(?P<resident>[a-z]+)-resident-seed\d+-noise\d+\.txt$")
TIME_RE = re.compile(r"delivered its own package at t ([\d.]+) s")


def classify(section):
    """Return (outcome, replacement_time_or_None) for one arm section."""
    if "**replacement completed**" in section:
        match = TIME_RE.search(section)
        return "completed", float(match.group(1)) if match else None
    if "**unresolved at the stopping budget**" in section or "unresolved at cap" in section:
        return "unresolved", None
    if "possible refuge" in section:
        return "possible refuge", None
    if re.search(r"\brefus", section, re.IGNORECASE):
        return "refusal", None
    return "unknown", None


def reduce_directory(directory):
    """direction -> summary dict, over every full-*.txt study arm in `directory`."""
    results = {}
    for path in sorted(glob.glob(os.path.join(directory, "full-*.txt"))):
        match = DIRECTION_RE.match(os.path.basename(path))
        if not match:
            continue
        resident = match.group("resident")
        with open(path) as handle:
            text = handle.read()
        summary = results.setdefault(
            resident,
            {"arms": 0, "completed": 0, "times": [], "outcomes": {}},
        )
        # Split on arm headers; [1:] drops the preamble before the first arm.
        for section in re.split(r"^-- arm:", text, flags=re.MULTILINE)[1:]:
            header = section.splitlines()[0].strip() if section.splitlines() else ""
            if not header.startswith(STUDY_ARM):
                continue
            outcome, time = classify(section)
            summary["arms"] += 1
            summary["outcomes"][outcome] = summary["outcomes"].get(outcome, 0) + 1
            if outcome == "completed":
                summary["completed"] += 1
                if time is not None:
                    summary["times"].append(time)
    return results


def fmt(value):
    return "-" if value is None else f"{value:.2f}"


def main(argv):
    directory = argv[1] if len(argv) > 1 else DEFAULT_DIR
    if not os.path.isdir(directory):
        print(f"not a directory: {directory}", file=sys.stderr)
        return 2

    results = reduce_directory(directory)
    if not results:
        print(f"no full-*.txt study files in {directory}", file=sys.stderr)
        return 1

    order = sorted(results)
    print("| resident | arms | completed | median (s) | min (s) | max (s) "
          "| unresolved | possible refuge | refusal | other |")
    print("| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |")

    def count(outcomes, name):
        return outcomes.get(name, 0)

    for resident in order:
        summary = results[resident]
        times = summary["times"]
        outcomes = summary["outcomes"]
        median = statistics.median(times) if times else None
        other = sum(
            n for name, n in outcomes.items()
            if name not in ("completed", "unresolved", "possible refuge", "refusal")
        )
        print(
            f"| {resident} | {summary['arms']} | {summary['completed']} "
            f"| {fmt(median)} | {fmt(min(times) if times else None)} "
            f"| {fmt(max(times) if times else None)} "
            f"| {count(outcomes, 'unresolved')} "
            f"| {count(outcomes, 'possible refuge')} "
            f"| {count(outcomes, 'refusal')} | {other} |"
        )
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
