#!/usr/bin/env python3
"""Launch one simulation study within one shared L3, respecting existing affinity.

Batch example (zero-based slots alternate between available L3 groups):
  scripts/run-study.py --slot 0 -- target/release/examples/voxel_census ...
  scripts/run-study.py --slot 1 -- target/release/examples/voxel_census ...
Run this wrapper on the execution host, including over SSH. No window protection
is added: study commands must be headless, or use scripts/hidden.sh.
"""

import argparse
import os
from pathlib import Path
import sys


def cpu_list(text):
    cpus = set()
    for part in text.strip().split(","):
        ends = [int(n) for n in part.split("-")]
        if len(ends) == 1:
            cpus.add(ends[0])
        elif len(ends) == 2 and ends[0] <= ends[1]:
            cpus.update(range(ends[0], ends[1] + 1))
        else:
            raise ValueError(f"invalid CPU range: {part}")
    return cpus


def l3_groups(allowed):
    groups = set()
    for cpu in sorted(allowed):
        caches = Path(f"/sys/devices/system/cpu/cpu{cpu}/cache")
        for index in caches.glob("index*"):
            if (index / "level").read_text().strip() != "3":
                continue
            shared = cpu_list((index / "shared_cpu_list").read_text())
            groups.add(tuple(sorted(shared & allowed)))
    covered = {cpu for group in groups for cpu in group}
    if not groups or covered != allowed:
        raise RuntimeError("cannot determine L3 groups for every allowed CPU; refusing unconfined launch")
    return sorted(groups)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--slot", type=int, default=0,
                        help="zero-based batch run index; selects slot modulo L3 group count")
    parser.add_argument("--list", action="store_true", help="print available groups and exit")
    parser.add_argument("command", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    if args.slot < 0:
        parser.error("--slot must be nonnegative")
    try:
        groups = l3_groups(os.sched_getaffinity(0))
        if args.list:
            for i, group in enumerate(groups):
                print(f"group {i}: CPUs {','.join(map(str, group))} ({len(group)} logical CPUs)")
            return
        command = args.command
        if command[:1] == ["--"]:
            command = command[1:]
        if not command:
            parser.error("provide a command after -- (or use --list)")
        index = args.slot % len(groups)
        cpus = groups[index]
        os.sched_setaffinity(0, cpus)
        print(f"study slot {args.slot}: L3 group {index}, CPUs {','.join(map(str, cpus))}",
              file=sys.stderr, flush=True)
        os.execvp(command[0], command)
    except (OSError, RuntimeError, ValueError) as error:
        parser.exit(1, f"run-study: {error}\n")


if __name__ == "__main__":
    main()
