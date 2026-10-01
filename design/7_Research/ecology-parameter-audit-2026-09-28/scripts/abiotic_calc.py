import math
vm=0.125; A=vm*vm; V=vm**3
K_soil=0.2*0.35*0.25; K_rock=0.001*0.02*0.25
print("K soil",K_soil,"m/s =",K_soil*3600*1000,"mm/h; rock",K_rock, K_rock*3.6e6)
for real in (1e-4,1e-5,1e-6):
    print("ratio to",real,K_soil/real,"after K=12:",K_soil/real/12)
R0=3.5e-5
print("R0 mm/h",R0*3.6e6,"drizzle",0.1*R0*3.6e6,"downpour",2*R0*3.6e6)
print("R0/K_soil",R0/K_soil, "downpour/K",2*R0/K_soil)
E=1e-5
print("evap mm/h",E*3.6e6,"mm/day(24h)",E*86400*1000,"mm per sim day (7200s)",E*7200*1000)
print("ratio to 5.5mm/d real",E*86400*1000/5.5,"after 12:",E*86400*1000/5.5/12)
print("default evap mm/day",3e-5*86400*1000)
pc=0.35;fc=0.65;wp=0.28
print("FC vol",fc*pc,"WP vol",wp*pc,"avail",(fc-wp)*pc)
for d in (0.25,0.75):
    print("root depth",d,"avail mm",(fc-wp)*pc*d*1000)
L=120;dl=0.67
print("day min",L*dl,"night min",L*(1-dl), "real-equiv night h",24*(1-dl))
print("weather speed", (1440/L)**0.65)
foot=256*48*vm*vm
C=6.5
print("footprint",foot,"capacity mm",C/foot*1000,"total m3",C/0.03,"m", C/0.03/foot)
for name,r,lo,rate,lm in (("drizzle",0.98,0.8,0.1,90),("shower",1.0,0.7,1.0,30),("downpour",1.13,0.5,2.0,50)):
    dm=(r-lo)*C
    depth=dm/foot*1000
    rate_mmh=rate*R0*3.6e6
    tmin=depth/rate_mmh*60
    cap=lm/5*rate_mmh/60
    print(name,"volume m3",round(dm,2),"mm over footprint",round(depth,1),"rate mm/h",round(rate_mmh,1),"min to rain",round(tmin,1),"longest_min@5x",lm/5,"cap depth mm",round(cap,1))
avail_m3=(fc-wp)*pc*V
n=18
print("avail per voxel m3",avail_m3,"box(3x3x2)",avail_m3*n)
transp=1e-6
print("transp L per sim day (7200 s)",transp*7200*1000)
print("time to wilt from FC, F=1 s",avail_m3*n/transp,"min",avail_m3*n/transp/60)
foot9=9*A
print("mm per sim day per unit foliage over 3x3 footprint",transp*7200/foot9*1000)
for depth in (10.2,21):
    print("shower mm",depth,"share of box avail",depth/1000*foot9/(avail_m3*n))
print("drain flux per tick soil",K_soil*A*0.05,"pore cap per voxel",V*pc,"ratio",K_soil*A*0.05/(V*pc))
excess=(1-fc)*pc*V
print("drain time ticks",excess/(K_soil*A*0.05),"s",excess/(K_soil*A*0.05)*0.05)
print("aquifer m3 per m head desk",foot*0.35)
print("stream desk L/s",8e-3*1000,"peak",8e-3*5*1000,"m3/h",8e-3*3600,"peak m3/h",8e-3*5*3600)
print("95% gone after s",-50*math.log(0.05))
print("cc per 10C",math.exp(0.7),1/math.exp(0.7),"diurnal +-3C",math.exp(0.21),math.exp(-0.21))
n=39.6*60
print("night s",n,"animal basal used",0.001*n)
for mm in (30,40,52):
    print(mm,"mm/sim-day -> m/yr",mm*365/1000, "x3m/yr",mm*365/3000)
print("real 3 m/yr mm/day", 3000/365)
