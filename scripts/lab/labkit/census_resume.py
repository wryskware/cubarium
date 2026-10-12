"""Resume a census and append its stdout as one continuous CSV series."""

import csv
import math
from pathlib import Path
import subprocess
import sys


def latest_checkpoint(run):
    candidates = []
    for path in (Path(run) / "ckpt").glob("m*/minute"):
        try:
            minute = float(path.read_text())
        except (OSError, ValueError):
            continue
        if math.isfinite(minute) and minute >= 0:
            candidates.append((minute, path.parent))
    return max(candidates, key=lambda item: item[0], default=(None, None))


def prepare_census(path, minute):
    """Discard uncheckpointed/partial rows; keep the inode used by taskwrap's append fd."""
    try:
        text = path.read_text()
    except FileNotFoundError:
        text = ""
    rows = list(csv.reader(text[:text.rfind("\n") + 1].splitlines()))
    if minute is None or not rows:
        path.write_text("")
        return None, False
    header = rows[0]
    column = header.index("sim_min" if "sim_min" in header else "minute")
    kept, boundary = [header], False
    for row in rows[1:]:
        if len(row) != len(header):
            continue
        try:
            value = float(row[column])
        except ValueError:
            continue
        if math.isfinite(value) and value <= minute:
            kept.append(row)
            boundary |= value == minute
    with path.open("w", newline="") as stream:
        csv.writer(stream, lineterminator="\n").writerows(kept)
    return header, boundary


def main():
    binary, root, threads, *original = sys.argv[1:]
    root = Path(root)
    minute, checkpoint = latest_checkpoint(root / "run")
    header, boundary = prepare_census(root / "census.csv", minute)
    command = ([binary, f"resume={root / 'run'}", f"threads={threads}"]
               if checkpoint else [binary, *original])
    with subprocess.Popen(command, stdout=subprocess.PIPE, text=True) as child:
        incoming = next(child.stdout, "")
        columns = next(csv.reader([incoming]), [])
        if header and columns != header:
            child.terminate()
            raise ValueError("resumed census header differs from existing census")
        if not header:
            sys.stdout.write(incoming)
        column = columns.index("sim_min" if "sim_min" in columns else "minute")
        for line in child.stdout:
            row = next(csv.reader([line]))
            value = float(row[column])
            if minute is not None and (value < minute or (boundary and value == minute)):
                continue
            sys.stdout.write(line)
            sys.stdout.flush()
        return child.wait()


if __name__ == "__main__":
    raise SystemExit(main())
