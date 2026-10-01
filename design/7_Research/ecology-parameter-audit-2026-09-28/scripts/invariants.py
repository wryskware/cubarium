# Stay-vs-move invariants for the browser, sim vs a real 15 kg browser.
need = 1.25e-4           # foliage org-units/s at rest (upkeep 5e-5 / eff 0.4)
B = 1000.0               # body turnover, s
v, w_mouth = 0.75, 0.40  # cruise m/s, mouth swath m (footprint 0.375 + reach)
A_mouth = 3.1416*(0.1875+0.1875)**2   # ground within mouth reach of a standing body, m^2
print(f"mouth-reach area ~{A_mouth:.2f} m^2")
# springturf: edible above refuge 0.3 of P_cap 0.12 -> 0.084; income-limited regrowth
P_cap=0.12; edible=0.7*P_cap
g, mon = 0.008, 0.67
inc_full = g*P_cap*mon; maint=0.002*0.06
regrow_full = min(0.02*0.06, (inc_full-maint)/1.2)
inc_ref = g*0.3*P_cap*mon
regrow_ref = max(0,(inc_ref-maint)/1.2)
print(f"springturf: edible {edible:.3f}; regrowth at full foliage {regrow_full:.2e}/s, at refuge {regrow_ref:.2e}/s")
for n in (1,4,7):
    print(f"  {n} turfs in reach: R_park={n*regrow_full/need:.1f} (at refuge {n*regrow_ref/need:.2f}); S={n*edible/(need*B):.2f} body-turnovers")
# bloomcrown adult rosette
Wb=0.6; Pb=2*Wb; ros=0.25*Pb*(1-0.15)
reg_b = 0.002*Wb
print(f"bloomcrown rosette edible {ros:.3f}; max regrowth into it {reg_b:.2e}/s -> R={reg_b/need:.1f}, S={ros/(need*B):.2f}")
# dense turf field, one stand per 0.125 m column
dens = 1/0.125**2
print(f"dense turf: {dens:.0f} stands/m2, edible {dens*edible:.2f}/m2; walker F at 20% harvest = {dens*edible*v*w_mouth*0.2/(2*need):.0f}")
# real 15 kg browser
need_real = 0.5          # kg DM/day
standing = 0.3*3.0*0.3   # kg DM in ~3 m^2 at 0.3 kg/m2, 30% edible
regrow_real = 3.0*0.002  # kg DM/day, 3 m^2 at 2 g/m2/day
tau_real = 20            # days body turnover
print(f"real 15 kg browser: R_park={regrow_real/need_real:.4f}; S={standing/(need_real*tau_real):.3f} body-turnovers ({standing/need_real:.2f} days of food)")
for f in (30,100,300):
    print(f"animal mass x{f}: springturf-7 R={7*regrow_full/(need*f):.2f}, bloomcrown R={reg_b/(need*f):.2f}, S7={7*edible/(need*f*B):.3f}, F={dens*edible*v*w_mouth*0.2/(2*need*f):.1f}")
