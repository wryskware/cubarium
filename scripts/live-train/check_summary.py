import csv,glob,os,sys,statistics as st
D=sys.argv[1]
rows={}
for p in sorted(glob.glob(D+'/*.csv')):
    arm,seed=os.path.basename(p)[:-4].split('__')
    rows[(arm,seed)]=list(csv.DictReader(open(p)))
arms=sorted({a for a,_ in rows})
def at(rs,m):
    for r in rs:
        if float(r['minute'])>=m: return r
    return rs[-1]
L=['littershredder','frondgrazer']
print('animals @1..6h')
for (arm,seed),rs in sorted(rows.items()):
    print(f" {arm:9} {seed}: "+' '.join(at(rs,m)['animals'] for m in (60,120,180,240,300,360))+f"  (last min {rs[-1]['minute']})")
for arm in arms:
    sub=sorted((s,rs) for (a,s),rs in rows.items() if a==arm)
    print('##',arm)
    for l in L:
        f=lambda k,h=0: [sum(int(r[f'{l}.{k}']) for r in rs if float(r['minute'])>h) for s,rs in sub]
        mv=[st.mean([float(r[l+'.moving_share']) for r in rs if float(r['minute'])>180 and r[l+'.moving_share']!='nan'] or [float('nan')]) for s,rs in sub]
        print(f"  {l}: end {[int(rs[-1][l+'.population']) for s,rs in sub]} births {f('births')} (2nd half {f('births',180)}) starved {f('starved')} drowned {f('drowned')} aged {f('aged')} (2nd half deaths {[a+b+c for a,b,c in zip(f('starved',180),f('drowned',180),f('aged',180))]}) moving(2nd half) {[round(x,3) for x in mv]}")
    sp=[k for k in sub[0][1][0] if k.startswith('stands.')]
    for s,rs in sub:
        print('   ',s,' '.join(f"{k[7:]}:{rs[0][k]}->{rs[-1][k]}" for k in sp))
