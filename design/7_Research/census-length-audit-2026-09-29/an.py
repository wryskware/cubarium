import csv,sys
def an(path):
    rows=list(csv.reader(open(path)))
    h=rows[0]; data=[[float(x) if x not in('',) else 0 for x in r] for r in rows[1:] if len(r)==len(h)]
    T=len(data)-1
    print("==",path,"rows",len(data),"last min",data[-1][0])
    # dedupe duplicated column names by index
    for j,c in enumerate(h):
        if c in('sim_min',) or c.startswith(('flora_births','flora_deaths','fauna_','litter','stored','atmos','showers','resid','body_','founders_')) and 'founders' not in c: 
            pass
        col=[r[j] for r in data]
        peak=max(col); pi=col.index(peak)
        first0=next((int(data[i][0]) for i in range(pi,len(col)) if col[i]==0 and peak>0),None)
        # decline >50% from peak
        dec=next((int(data[i][0]) for i in range(pi,len(col)) if peak>0 and col[i]<0.5*peak),None)
        n=len(col); last=data[-1][0]
        tail=[r[j] for r in data if r[0]>=last-60]
        m=sum(tail)/len(tail)
        stab=None
        if m>0:
            for i in range(len(col)):
                if all(abs(x-m)<=0.2*m for x in col[i:]): stab=int(data[i][0]);break
        print(f"{j:2d} {c:28s} start={col[0]:.4g} peak={peak:.4g}@{int(data[pi][0])} min60={col[min(60,n-1)]:.4g} min120={col[min(120,n-1)]:.4g} end={col[-1]:.4g} first0={first0} halfdrop={dec} stab20={stab}")
for p in sys.argv[1:]: an(p)
