#!/usr/bin/env python3
"""MIMIC interval table. Counts use both spaces and commas; resumed summaries
are partial, so totals come from unique five-minute ledger windows, keyed by minute.
Clock starts 08:00 and cycles every120 min. All intake is swallowed organic.
"""
from collections import defaultdict
from pathlib import Path
import math
import re
import sys

def kv(line):
 return dict(re.findall(r"([A-Za-z0-9_-]+)=([^,\s]+)",line))
def num(d,k):
 try:return float(d.get(k,0))
 except (TypeError,ValueError):return 0.0
def ratio(a,b):return a/b if b>0 else float("nan")
def hour(m):return int(math.floor((8+(m-2.5)/120*24)%24))
def parse(path):
 stocks={}; windows={}; meets={}
 for line in path.read_text(errors="replace").splitlines():
  p=line.split(",")
  if len(p)<3:continue
  try:m=int(p[2])
  except ValueError:continue
  d=kv(line)
  if p[0]=="stocks":stocks[m]=d
  elif p[0]=="bwl" and m>0:windows[m]=d
  elif p[0]=="meet" and m>0:meets[m]=d
 sums=defaultdict(float); nectar=0.0
 visits={k:defaultdict(float) for k in ("flower_visits","glowcap_visits","lure_visits")}
 for m,d in windows.items():
  for k,v in d.items():
   try:sums[k]+=float(v)
   except ValueError:pass
  bh=num(d,"bw_hours"); rate=num(d,"nectar_per_bwh")
  if math.isfinite(rate):nectar+=bh*rate
  for k in visits:visits[k][hour(m)]+=num(d,k)
 sums["nectar"]=nectar
 sums["lj_day_s"]=sum(num(d,"lj_day_s") for d in meets.values())
 sums["lj_exposed_s"]=sum(num(d,"lj_exposed_s") for d in meets.values())
 return stocks,sums,visits

ARMS = ("main", "mimic", "both")
SEEDS = range(1, 5)

def main():
 runs=Path(sys.argv[1] if len(sys.argv)>1 else "runs")
 grouped=defaultdict(list)
 print("arm seed | BW +2h +4h +6h | BW deaths LJ/VH/SP/starve/age/fall/drown/dry | lure visits/kills | kills LJh/SPh | intake LJh/SPh | end LJ/SP | flower/glow | nectar/BWh | LJ day exposure | status")
 for arm, seed in ((a, str(s)) for a in ARMS for s in SEEDS):
  path = runs / f"{arm}-s{seed}.out"
  if not path.exists():
   d = runs / f"{arm}-s{seed}"
   detail = (d / "FAILED").read_text().strip() if (d / "FAILED").exists() else "no readouts"
   print(f"{arm} s{seed} | - - - | - | - | - | - | - | - | - | - | MISSING: {detail}")
   continue
  stocks,s,v=parse(path); grouped[arm].append((s,v))
  end=max(stocks,default=0); latest=stocks.get(end,{})
  def count(m):return stocks.get(m,{}).get("bellwing","-")
  deaths="/".join(str(int(s[k])) for k in ("bw_killed_lj","bw_killed_vh","bw_killed_sp","bw_starved","bw_aged","bw_fell","bw_drowned","bw_dry"))
  d=runs/f"{arm}-s{seed}"
  status="DONE" if (d/"DONE").exists() else "FAILED" if (d/"FAILED").exists() else "PARTIAL"
  print(f"{arm} s{seed} | {count(120)} {count(240)} {count(360)} | {deaths} | {s['lure_visits']:.0f}/{s['lure_kills']:.0f} | {ratio(s['bw_killed_lj'],s['lj_hours']):.4f}/{ratio(s['bw_killed_sp'],s['sp_hours']):.4f} | {ratio(s['lj_intake'],s['lj_hours']):.5f}/{ratio(s['sp_intake'],s['sp_hours']):.5f} | {latest.get('lanternjaw','-')}/{latest.get('seedporter','-')} | {s['flower_visits']:.0f}/{s['glowcap_visits']:.0f} | {ratio(s['nectar'],s['bw_hours']):.5f} | {ratio(s['lj_exposed_s'],s['lj_day_s']):.3f} | {status} m{end}")
 for arm,rows in sorted(grouped.items()):
  for k in ("lure_visits","flower_visits","glowcap_visits"):
   bins=defaultdict(float)
   for s,v in rows:
    for h,n in v[k].items():bins[h]+=n
   print(f"{arm}: {k} by hour (all seeds): "+" ".join(f"{h:02d}:{bins[h]:.0f}" for h in range(24)))
 if not grouped:print("No run readouts yet.")
if __name__=="__main__":main()
