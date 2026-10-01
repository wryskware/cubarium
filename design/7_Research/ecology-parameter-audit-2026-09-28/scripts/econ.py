# Browser/shredder energy economics straight from the shipped tables (3ccc6daf).
import math
MJ=1e6
def kleiber_W(M): return 3.4*M**0.75
def cot_net(M): return 10.7*M**-0.316          # J/kg/m, Taylor et al 1982
def cot_icpt(M): return 6.03*M**-0.303         # W/kg zero-speed intercept
E_TISSUE=7.5*MJ                                 # J/kg fresh

sim = dict(
 browser=dict(maint=0.001, motor=0.001, bite=0.002, K=0.005, yld=0.5, ntis=0.05, nfood=0.02,
   bmax=0.05, bmin=0.005, bbirth=0.03, bcost=0.01, rcap=0.5, grow=9.5e-6, floor=0.005, hold=900,
   gest=180, interval=1800, life=4*3600, L=0.75, W=0.375, H=0.375),
 shredder=dict(maint=0.001, motor=0.001, bite=0.0005, K=0.00125, yld=0.5, ntis=0.02, nfood=0.02,
   bmax=0.0125, bmin=0.003125, bbirth=0.0125, bcost=0.004, rcap=0.5, grow=3.4e-6, floor=0.000625, hold=900,
   gest=0, interval=1800, life=4*3600, L=0.375, W=0.125, H=0.125),
)
for name,s in sim.items():
    eff = min(s['yld'], s['nfood']/s['ntis'])
    up = s['maint']*s['bmax']
    need = up/eff
    cruise = s['L']
    r = s['W']/2
    print(f"== {name}")
    print(f" effective yield {eff:.2f}; basal upkeep at body_max {up:.3g}/s; food need {need:.3g}/s")
    print(f" max assimilation / upkeep = {s['bite']*eff/up:.1f}x ; body turnover 1/maint = {1/s['maint']:.0f} s")
    print(f" motor at cruise / basal = {s['motor']/s['maint']:.2f}; max turn in place / basal = {s['motor']/s['maint']*r*2.0/cruise:.2f}")
    res = s['rcap']*s['bmax']
    print(f" reserve cap {res:.4g} = {res/up:.0f} s at rest, {res/(2*up):.0f} s at cruise, {res/(0.5*up):.0f} s asleep")
    per_m = s['motor']*s['bmax']/cruise
    print(f" motor per metre {per_m:.3g} org-units = {per_m/s['bmax']*100:.3f}% of body/m; food per metre {per_m/eff:.3g}")
    # breeding margin at body_max
    elig = s['bcost']+s['floor']
    print(f" breeding: needs reserve {elig:.4g} of cap {res:.4g} ({elig/res:.0%} full, hunger<={1-elig/res:.2f}); "
          f"buffer above floor {res-elig:.4g} = {(res-elig)/up:.0f} s rest / {(res-elig)/(2*up):.0f} s cruise without food")
    # equilibrium hunger at an undefended plant: bite*h*E/(E+K)*eff = up
    print(f" equilibrium hunger parked at rich food (E>>K, no defence): {up/(s['bite']*eff):.3f}")
    for d in (0.85,0.96):
        print(f"   with defence {d}: hunger {up/(s['bite']*eff*(1-d)):.2f}")
# Real comparison
print("== real mammals at 1 BL/s")
for M,L in ((15,0.75),(50,0.75),(500,2.4)):
    v=L
    bmr=kleiber_W(M)/M
    print(f" M={M} kg v={v} m/s: net COT {cot_net(M):.2f} J/kg/m -> {cot_net(M)*v:.2f} W/kg = {cot_net(M)*v/bmr:.2f}x BMR, "
          f"{cot_net(M)*v/cot_icpt(M):.2f}x standing; body turnover at FMR(2.5 BMR) {M*E_TISSUE/(2.5*kleiber_W(M))/86400:.1f} d "
          f"-> K={M*E_TISSUE/(2.5*kleiber_W(M))/1000:.0f}")
# cow at normal walk 1.1 m/s
M=500; print(f" cow 500 kg at 1.1 m/s: {cot_net(M)*1.1:.2f} W/kg vs BMR {kleiber_W(M)/M:.2f} W/kg = {cot_net(M)*1.1/(kleiber_W(M)/M):.1f}x")
# ectotherm 1 kg at 1 BL/s = 0.375 m/s
M=1; rest=0.15*kleiber_W(M)
print(f" 1 kg ectotherm at 0.375 m/s: {cot_net(M)*0.375:.2f} W/kg vs resting {rest:.2f} W/kg = {cot_net(M)*0.375/rest:.1f}x")
