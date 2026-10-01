import math
base=dict(alpha=2.0,W=0.6,q=0.5,m=0.0002,sen=0.001,rp=0.002,rw=0.001,cg=0.2,alive=0.02,donor=0.3,floor=0.5,prop=0.0002,wfrac=0.4,g=0.004,KN=0.5,fmax=0.01,lh=0.5,refuge=0.15,smass=1,H=0.75,R=0.375,trans=1e-6,seedage=28800)
S={
'bloomcrown':dict(lh=0.8,smass=2,prop=0.0004,H=1.0,R=0.3125,band=0.25),
'umbrellafrond':dict(lh=0.15,H=2.0,R=0.75,band=0.0),
'springturf':dict(lh=1.0,W=0.06,alive=0.006,donor=0.03,m=0.002,rp=0.02,rw=0.01,prop=0.002,g=0.008,H=0.1875,R=0.25,refuge=0.3,band=1.0),
'stonecushion':dict(lh=0.3,W=0.1,alive=0.01,donor=0.05,m=0.00005,sen=0.0003,rp=0.0005,rw=0.0002,prop=0.00005,H=0.1875,R=0.15625,refuge=0.4,band=1.0),
'velvetpad':dict(lh=0.1,W=0.2,alive=0.015,donor=0.1,sen=0.0005,rp=0.004,rw=0.002,prop=0.0005,H=0.125,R=0.375,refuge=0.3,band=1.0),
'glowcap':dict(g=0.0,W=0.1,alive=0.01,donor=0.05,rp=0.004,rw=0.002,prop=0.0005,H=0.25,R=0.0625,band=0.0,trans=0.0),
'vaulttree':dict(lh=0.6,W=5.0,alive=0.02,donor=2.5,q=1.5,m=0.0001,sen=0.0005,rp=0.001,rw=0.0003,prop=0.0008,smass=4,H=3.5,R=1.0,band=0.0),
'lanternberry':dict(lh=0.4,W=1.0,alive=0.02,donor=0.4,floor=0.7,prop=0.0012,smass=3,H=1.125,R=0.4375,band=3/14),
'siphonreed':dict(lh=0.8,W=0.3,alive=0.02,donor=0.15,m=0.0004,prop=0.0003,H=1.5,R=0.125,band=0.0),
}
NEED=1.25e-4
N=1.0
for name,o in S.items():
    p=dict(base);p.update(o)
    a,W=p['alpha'],p['W']
    monod=N/(N+p['KN'])
    Pcap=a*W
    def A(P): return min(p['g']*P*monod, p['fmax']*N)
    inc_full=A(Pcap); uncapped=p['g']*Pcap*monod
    upkeep=p['m']*W+(1+p['cg'])*p['sen']*Pcap
    margin=inc_full/upkeep if upkeep>0 else float('nan')
    be=upkeep/(p['g']*Pcap*monod) if p['g']>0 else float('nan')
    alc=p['g']*a/p['m']
    pkg=p['smass']*p['alive']/p['wfrac']; w0=p['wfrac']*pkg
    t_pkg=pkg*(1+p['cg'])/p['prop']
    t_donor=math.log(p['donor']/w0)/p['rw']; t_max=math.log(W/w0)/p['rw']
    res_s=p['q']/p['m']; res_s2=p['q']*W/upkeep
    edible=p['band']*(1-p['refuge'])*Pcap
    # regrowth into band
    if p['band']>=1.0:
        best=0;bestP=0
        for i in range(1001):
            P=Pcap*(p['refuge']+(1-p['refuge'])*i/1000)
            G=min(p['rp']*W, max(0,(A(P)-p['m']*W)/(1+p['cg'])))
            Y=G-p['sen']*P
            if Y>best: best,bestP=Y,P
        regrow_cap=p['rp']*W; msy=best
    elif p['band']>0:
        P=Pcap*(1-p['band']*(1-p['refuge']))  # band layer at its floor, crown full
        G=min(p['rp']*W, max(0,(A(P)-p['m']*W)/(1+p['cg'])))
        regrow_cap=min(p['rp']*W, p['band']*(1-p['refuge'])*Pcap/1.0)  # ceiling
        msy=G-p['sen']*P; bestP=P
    else:
        regrow_cap=0;msy=0;bestP=0
    foliage_DM=math.pi*p['R']**2*3*0.05
    trans_mm_day=p['trans']*Pcap/(math.pi*p['R']**2)*1000*86400
    print(f"{name:14s} Pcap={Pcap:.3g} inc={inc_full:.3g}(uncapped {uncapped:.3g}) upkeep={upkeep:.3g} margin={margin:.2f} breakevenLmu={be:.3f} alc={alc:.1f} pkg={pkg:.3g} w0={w0:.3g} t_pkg={t_pkg:.0f}s t_donor={t_donor:.0f}s t_Wmax={t_max:.0f}s resQ/m={res_s:.0f}s resQ/upkeep={res_s2:.0f}s regrowfull={a/p['rp']:.0f}s leaflife={1/p['sen']:.0f}s edible={edible:.3g} ({edible/NEED:.0f} s need) regrowcap={regrow_cap:.3g} ({regrow_cap/NEED:.2f} need) msy={msy:.3g} ({msy/NEED:.2f} need) at P={bestP:.3g} foliageDMkg~{foliage_DM:.3g} transp_mm_per_s_day={trans_mm_day:.0f} Hfull={p['H']}")
print("---- regrowth into band (open sky, mu=1, N=1)")
for name,o in S.items():
    p=dict(base);p.update(o)
    a,W=p['alpha'],p['W'];Pcap=a*W;monod=N/(N+p['KN'])
    if p['band']<=0: print(f"{name:14s} not in band at full size");continue
    def A(P): return min(p['g']*P*monod, p['fmax']*N)
    funded=min(p['rp']*W, max(0,(A(Pcap)-p['m']*W)/(1+p['cg'])))
    band_floor=p['band']*p['refuge']*Pcap
    lo=band_floor if p['band']<1 else p['refuge']*Pcap
    best=(0,0)
    for i in range(2001):
        P=lo+(Pcap-lo)*i/2000
        G=min(p['rp']*W, max(0,(A(P)-p['m']*W)/(1+p['cg'])))
        Y=G-p['sen']*P
        if Y>best[0]: best=(Y,P)
    print(f"{name:14s} ceiling rpW={p['rp']*W:.3g} ({p['rp']*W/NEED:.1f} need); funded gross at full canopy={funded:.3g} ({funded/NEED:.1f} need); MSY={best[0]:.3g} ({best[0]/NEED:.2f} need) at P={best[1]:.3g} of Pcap {Pcap}; band cap={p['band']*Pcap:.3g} band edible={p['band']*(1-p['refuge'])*Pcap:.3g}; time to refill band edible at funded rate={p['band']*(1-p['refuge'])*Pcap/funded:.0f}s")
# defence equilibrium
for b in (1,3,4):
    r=b*6e-4; k=math.log(2)/5400
    d=r/(r+k); print(f"bites/s={b}: defence eq={d:.3f}, bite keeps {1-d:.3f}, approach tau={1/(r+k):.0f}s")
