import csv,glob,os,sys,statistics as st
# gate_summary.py DIR... : per arm and lineage over seeds: walked m/body-h and moving share
# in the second half (median over seeds), alive at the end (sum and median), births,
# deaths by cause (sums).
def f(x):
    try: return float(x)
    except: return float('nan')
for d in sys.argv[1:]:
    runs=[]
    for p in sorted(glob.glob(d+'/*.csv')):
        rows=[r for r in csv.DictReader(open(p)) if f(r['minute'])>0]
        rows=[r for i,r in enumerate(rows) if i==0 or r['minute']!=rows[i-1]['minute']]
        runs.append(rows)
    H=f(runs[0][-1]['minute'])
    for L in ('frondgrazer','littershredder'):
        def half(rs,c):
            v=[f(r[L+'.'+c]) for r in rs if f(r['minute'])>H/2]
            v=[x for x in v if x==x]
            return st.mean(v) if v else float('nan')
        med=lambda v: (lambda w: f"{st.median(w):.3g}" if w else '-')([x for x in v if x==x])
        tot=lambda c: sum(sum(int(r[L+'.'+c]) for r in rs) for rs in runs)
        end=[int(rs[-1][L+'.population']) for rs in runs]
        print(f"{os.path.basename(d):9} {L[:6]} n={len(runs)} walked2 {med([half(rs,'walked_m_per_body_h') for rs in runs]):>5} moving2 {med([half(rs,'moving_share') for rs in runs]):>6} alive {sum(end)} (med {st.median(end)}) births {tot('births')} starved {tot('starved')} drowned {tot('drowned')} aged {tot('aged')}")
