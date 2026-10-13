#!/usr/bin/env python3
"""Read kick_trace stdout logs (crates/cubarium-voxel-sim/examples/kick_trace.rs).

    python3 -I scripts/studies/kick-trace-read.py [--list] LOG...

Per log: the last minute printed, kicks resolved, kicks landed (the hound's wound rose, or
its target died that tick), landed kicks per hound, and the hounds that died (id@minute).
--list also prints each kick condensed: minute, kicker, its defence intent, the hound's
intent, state, distance, wound before->after and the carcass by it.
"""
import re
import sys


def read(path, listing):
    kicks = landed = 0
    dead, hit = [], {}
    last = 0.0
    for line in open(path):
        m = re.match(r"^\s*([\d.]+) ", line)
        if m:
            last = max(last, float(m.group(1)))
        if " KICK by " in line:
            kicks += 1
            w = re.search(r"wound ([\d.]+)->([\d.]+)$", line.strip())
            h = re.search(r"-> hound (\d+)|target (\d+) gone", line)
            hid = (h.group(1) or h.group(2)) if h else "?"
            if (w and float(w.group(2)) > float(w.group(1))) or " gone " in line:
                landed += 1
                hit[hid] = hit.get(hid, 0) + 1
            if listing:
                k = re.search(r"KICK by (\d+) .*? defend (\S+)", line)
                hd = re.search(r"-> hound \d+ .*? intent (\S+) eating (\S+) .*? state (\S+) .*?(?:carcass (.*?) )?dist ([\d.]+) .*?wound ([\d.]+->[\d.]+)$", line.strip())
                if hd:
                    print(f"  {line.split()[0]} ls{k.group(1)} {k.group(2)} -> h{hid} {hd.group(1)} hungry={hd.group(2)} {hd.group(3)} d={hd.group(5)} w={hd.group(6)} carcass {hd.group(4) or '-'}")
                else:
                    print(f"  {line.split()[0]} ls{k.group(1) if k else '?'} -> h{hid} {'(died this tick)' if ' gone ' in line else '?'}")
        if " DEAD " in line:
            dead.append(re.search(r"hound (\d+)", line).group(1) + "@" + line.split()[0])
    name = path.rstrip("/").split("/")
    name = name[-3] if len(name) >= 3 else path
    print(f"{name}: to m{last:.1f} kicks {kicks} landed {landed} per-hound {hit} dead {len(dead)} {dead}")


def main():
    args = sys.argv[1:]
    listing = "--list" in args
    for p in [a for a in args if a != "--list"]:
        read(p, listing)


if __name__ == "__main__":
    main()
