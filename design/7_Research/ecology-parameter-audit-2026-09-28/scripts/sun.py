import math
rise,setp=0.5-0.67/2,0.5+0.67/2
day=setp-rise;night=1-day
peak=math.radians(70)
def sun(phase):
    since=(phase-rise)%1.0
    if since<=day: el=peak*math.sin(math.pi*since/day)
    else:
        depth=min(peak*night/day,math.pi/2)
        el=-depth*math.sin(math.pi*(since-day)/night)
    tw=math.radians(6)
    x=min(1,max(0,(el+tw)/(2*tw)))
    return el,x*x*(3-2*x)
N=100000
c30=c05=c95=c_mid=0
for i in range(N):
    el,d=sun(i/N)
    if d>0.3:c30+=1
    if d>0.05:c05+=1
    if d>0.95:c95+=1
    if 0.05<d<0.95:c_mid+=1
print("daylight>0.3 frac",c30/N,"min",c30/N*120)
print(">0.05",c05/N,">0.95",c95/N,"twilight band",c_mid/N,c_mid/N*120,"min")
print("night depth deg", math.degrees(min(peak*night/day,math.pi/2)))
print("rise,set",rise,setp, "hours", rise*24,setp*24)
