import csv,glob,os,sys,statistics as st
# mass_summary.py DIR... : each DIR holds x<k>__seed<N>.csv from voxel-ecology-check.
# Per arm and lineage: medians over seeds of body-weighted walked m/body-h, moving share,
# intake per body-h in x1 units (intake / k); totals of births and starved; animals at end.
def f(x):
    try: return float(x)
    except: return float('nan')
def med(v):
    v=sorted(x for x in v if x==x)
    return f"{st.median(v):.3g}" if v else '-'
for d in sys.argv[1:]:
    groups={}
    for p in sorted(glob.glob(d+'/*.csv')):
        w=os.path.basename(p).split('__')[0]
        rows=[r for r in csv.DictReader(open(p)) if f(r['minute'])>0]
        rows=[r for i,r in enumerate(rows) if i==0 or r['minute']!=rows[i-1]['minute']]
        groups.setdefault(w,[]).append(rows)
    for w in sorted(groups,key=lambda s:float(s[1:])):
        runs=groups[w]; k=float(w[1:])
        for L,short in (('frondgrazer','B'),('littershredder','S')):
            def wmean(rs,c):
                ws=[(f(r[c]),f(r[L+'.moving_share'])) for r in rs]
                v=[x for x,_ in ws if x==x]
                return st.mean(v) if v else float('nan')
            walked=[wmean(rs,L+'.walked_m_per_body_h') for rs in runs]
            mov=[wmean(rs,L+'.moving_share') for rs in runs]
            intake=[wmean(rs,L+'.intake_per_body_h')/k for rs in runs]
            births=sum(sum(int(r[L+'.births']) for r in rs) for rs in runs)
            starved=sum(sum(int(r[L+'.starved']) for r in rs) for rs in runs)
            end=[int(rs[-1][L+'.population']) for rs in runs]
            print(f"{os.path.basename(d.rstrip('/')):7} {w:5} {short} n={len(runs)} h={f(runs[0][-1]['minute'])/60:.0f} walked {med(walked):>6} moving {med(mov):>6} intake/k {med(intake):>7} births {births:4} starved {starved:4} end med {med(end):>4} sum {sum(end)}")
