import csv,glob,os,sys,statistics as st
# arms.py DIR... : each DIR holds <world>__seed<N>.csv from voxel-ecology-check
L='frondgrazer'
def fnum(x):
    try: return float(x)
    except: return float('nan')
def med(v):
    v=sorted(x for x in v if x==x)
    return f"{st.median(v):.3g} [{v[0]:.3g}-{v[-1]:.3g}]" if v else 'nan'
for d in sys.argv[1:]:
    groups={}
    for p in sorted(glob.glob(d+'/*.csv')):
        w=os.path.basename(p).split('__')[0]
        groups.setdefault(w,[]).append(list(csv.DictReader(open(p))))
    for w,runs in groups.items():
        hours=float(runs[0][-1]['minute'])/60
        # body-weighted over windows: walked and intake per body-hour, moving share
        walked=[st.mean(fnum(r[L+'.walked_m_per_body_h']) for r in rs) for rs in runs]
        intake=[st.mean(fnum(r[L+'.intake_per_body_h']) for r in rs) for rs in runs]
        mov=[st.mean(fnum(r[L+'.moving_share']) for r in rs) for rs in runs]
        nn=[st.mean([fnum(r[L+'.nn_m']) for r in rs if fnum(r[L+'.nn_m'])==fnum(r[L+'.nn_m'])] or [float('nan')]) for rs in runs]
        alive=[int(rs[-1][L+'.population']) for rs in runs]
        births=sum(sum(int(r[L+'.births']) for r in rs) for rs in runs)
        starved=sum(sum(int(r[L+'.starved']) for r in rs) for rs in runs)
        aged=sum(sum(int(r[L+'.aged']) for r in rs) for rs in runs)
        extra=''
        if hours>2:
            extra=' | animals@6h '+','.join(rs[-1]['animals'] for rs in runs)+' shr@6h '+','.join(rs[-1]['littershredder.population'] for rs in runs)
        print(f"{os.path.basename(d.rstrip('/'))}/{w} ({len(runs)}x{hours:.0f}h): walked m/body-h {med(walked)} | moving {med(mov)} | intake/body-h {med(intake)} | nn m {med(nn)} | browsers@end {med(alive)} | births {births} starved {starved} aged {aged}{extra}")
