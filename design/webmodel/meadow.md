---
design_status: exploration
last_reviewed: 2026-10-05
decision_refs: []
---

# A lasting niche for grass: the meadow candidates, in the model first

**Status:** exploration, not canon. The levers are scenario levers of the whole-web model's
plant half (`crates/cubarium-webmodel/src/flora/meadow.rs`); none is a sim knob and no sim
code changed. The long batch is launched (below); its table is the evidence this document
still owes.

**The ask (Wrysk, 2026-10-05):** "i want vault trees to spread for sure. and definitely groves
of lanternberries. i guess itd be nice if we came up with a way for grass to have its own
niche so that meadows are also present in the end state." Succession moss → grass → woody
stays; woody beating grass is intended; springturf is the drought-tolerant California hill
grass and the main grazing crop. Balance through roles (WORKING_POLICY): a role or behaviour
before a number.

**Working target (the parent's, Wrysk may change it):** at the end state grass holds about
20–30 % of the open drained ground as meadow patches, vaulttree and lanternberry most of the
rest, each of the three with a viable, spatially separate share on seeds 5, 6 and 1.

## 1. The sim today: what decides woody establishment and tree death

All lines on main at 6f8610f4 (`crates/cubarium-voxel-flora/src/`). Numbers are the resolved
terrarium preset (`config/scenarios/terrarium.toml` over `config/desktop/terrarium.toml`,
rescaled to the world's cell by `Params::flora_for_world`).

**Establishment of a woody seed** (`seeds.rs:466` `germinate`, gates `step.rs:3816`
`establishes`):
- **Water gate:** root-box mean available water ≥ `establish_water_min`
  (`step.rs:4170`): lanternberry 0.4, vaulttree 0.2, bloomcrown 0.1; springturf 0.3.
- **Light gate:** terrain sky ≥ `establish_light_min` (`step.rs:4173`): lanternberry 0.35,
  bloomcrown 0.6, vaulttree 0.9 (springturf 0.75). Photo species germinate only by day
  (`step.rs:3827`).
- **Canopy gate** (`step.rs:3899` `canopy_pays`, `:3872` `break_even_cover`): the newborn's
  income under the light through every other stand's taller foliage, at the face's water and
  site mineral, must reach `establish_light_margin` (1.0) times its upkeep. The model's
  calibration found this is the binding gate for woody newborns: seeds pile under their
  parents, in their shade, on mineral they drained (`flora.md`, finding 1).
- **Through a ground cover:** a woody species may come up through any ground cover
  (`overtop_covers` empty; `seeds.rs:90` `may_overtop`), springturf included, at
  `overtop_per_check` 0.1 per check after winning the weighted lottery (`seeds.rs:80`,
  `:183`). When it sprouts, the cover dies at once as `Overtopped` (`seeds.rs:635`): the
  seedling never has to survive beside the grass it replaces (Astra's point 1).
- **Grazers and seedlings:** a bite takes foliage above the layer's grazing floor only
  (`lib.rs:5347`, `:5394` `withdraw_foliage`); it never takes wood, so "taking a stand's whole
  canopy does not kill it" (`lib.rs:5341`). Diets are by food class (`Diet::accepts`,
  fauna `lib.rs:2402`), so a frondgrazer in reach of a woody seedling's low layers
  (`band_crown_layers`, fauna `encounter.rs:120`) eats its foliage like turf's, and the
  seedling can starve by dieback afterwards; nothing removes a seedling outright.

**Death** (`lib.rs:4660` `DeathCause`: Starved, Drowned, Overtopped, Displaced, Drought):
- **No lifespan, no senescence of the individual.** `Stand` (`lib.rs:284`) carries no birth
  tick; no rule ends an old stand. An adult tree dies only when its wood falls under
  `alive_min` (`step.rs:3372`), by dieback of unpaid upkeep (`step.rs:3354`), by drowning
  (`step.rs:695`), or, for a curing species, past `cure_max_s` (`step.rs:3433`). Overtopped
  and Displaced apply to ground covers only.
- **Drought:** only springturf cures (`cure_after_s` 1,098 s below `cure_water` 0.1, then
  dormant at `cure_upkeep` 0.1 for up to `cure_max_s` 13,174 s = 1.83 world days;
  `step.rs:3083–3099`, `:3405`). Woody species have no cure: at zero income a stand's wood
  decays at its maintenance rate once its reserve is spent. Lanternberry's maintenance is
  3.3e-4 /s with a reserve cap of 0.5 W: a newborn starves in about 25 min of zero income, an
  adult (W = 1) in about 3.7 h; vaulttree (1.6e-4 /s, reserve cap 1.5 W) in about 12 h.
- **Overtopping of trees by trees:** none. Woody stands only shade each other (dieback).
- **Hence an absorbing state:** once a woody adult holds a face, nothing in the sim returns
  that face to grass short of starvation. With any woody recruitment at all, the Levins
  arithmetic (occupancy `p* = 1 − e/c`) gives `p* → 1` as `e → 0`: grass keeps only the
  faces where woody recruitment is exactly zero. This is why the model's end state is
  lanternberry 84–100 % of drained ground and vaulttree 77–99 % of the summit, and why
  slowing recruitment only delays it.

**Roots and soil:** a root box is `rooting_depth` voxels down and `rooting_radius` around,
counting soil voxels only (`step.rs:1883–1935`); water is their mean (`step.rs:1977`). Rescaled
depths: springturf, velvetpad, stonecushion, bloomcrown 4; lanternberry 6; umbrellafrond 8;
vaulttree 12 (Astra quotes the unscaled 2/3/6). Nothing requires a minimum soil volume: a
tree on two voxels of soil reads the same water as the grass beside it.

**Measured soil depth** (new census field `soil_depth`, soil voxels from the face down,
unbroken), the m370 saves: the terrarium generator lays soil in two depths. On the drained
soil zones, the share of faces with only 2 voxels of soil is:

| zone | s5 | s6 | s1 |
|---|---|---|---|
| mid-drained | 0.52 | 0.46 | 0.44 |
| bright-drained | 0.41 | 0.48 | 0.53 |
| open-drained | 0.46 | 0.44 | 0.60 |
| summit-drained | 0.39 | 0.31 | 0.35 |

Faces with ≥ 6 voxels: 0.33–0.50 of mid/bright/open drained, 0.60–0.67 of the summit; ≥ 8:
0.02–0.33. A thin-soil niche exists in the terrain already; no rule reads it.

## 2. The candidates, with the literature

| | mechanism | literature | role or rule |
|---|---|---|---|
| a | grazers crop woody seedlings as they graze the sward, at a rate set by grazing pressure | the demographic bottleneck: savanna trees are held in a "trap" below an escape height by repeated top-kill (Higgins, Bond & Trollope 2000, *J. Ecol.* 88:213); browsing as a seedling filter (Archibald et al. 2021, *J. Ecol.*, via Astra); grazing lawns, where grazers keep returning to the swards they keep short (McNaughton 1984, *Am. Nat.* 124:863). **Caution** (Donaldson et al. 2022, via Astra): grazing grass *alone* releases tree seedlings from grass competition | a role: the frondgrazer already in the sward |
| b | a drought bottleneck: dry spells kill woody seedlings on exposed drained ground; turf cures and regrows | recruitment limited to rare wet windows (Higgins et al. 2000; Sankaran et al. 2005, *Nature* 438:846, rainfall caps woody cover below ~650 mm MAP); the temporal storage effect (Chesson 2000, *Annu. Rev. Ecol. Syst.* 31:343) | climate (a weather preset), no new rule |
| c | sward competition: thatch and the turf's topsoil water suppress woody germination and early growth | Walter's two-layer hypothesis (Walter 1971; revisited by Ward, Wiegand & Getzin 2013, *Oecologia* 172:617: seedlings share the grass's topsoil layer, only adults escape it); grass competition suppresses savanna tree growth across stages (Riginos 2009, *Ecology* 90:335); annual grasses suppress blue-oak seedling emergence and growth in California (Gordon & Rice 2000, *Am. J. Bot.* 87:986) | a flora rule |
| d | gap dynamics: adult trees age and die; grass recolonises the gaps first | pattern and process, the gap-phase mosaic (Watt 1947, *J. Ecol.* 35:1); shifting mosaics in grazed wood-pasture (Olff et al. 1999, *Plant Biology* 1:127; Vera 2000, *Grazing Ecology and Forest History*) | a flora rule (a lifespan) |
| e | an edaphic niche: thin soil over rock where tree roots fail and grass holds | soil depth and texture set tree–grass balance where rainfall alone does not (Scholes & Archer 1997, *Annu. Rev. Ecol. Syst.* 28:517; Sankaran et al. 2005); Walter's root layers (Ward et al. 2013) | a flora rule (a rooting-soil requirement), on terrain that already has it |
| f | a contested establishment stage: the woody recruit stands in the grass until it escapes above it | Astra's review §1 (main 54f7ffbe); the escape-height bottleneck (Higgins et al. 2000); grass competition on seedlings (Riginos 2009) | a flora rule (a recruit stage) |

Tree–grass coexistence in general: Scholes & Archer 1997 (review: competition vs
demographic bottlenecks); Sankaran et al. 2005 (rainfall sets the cap, disturbance holds
cover under it); Staver, Archibald & Levin 2011 (*Science* 334:230: savanna and forest as
alternative states held by feedbacks). The common thread: **coexistence comes from what limits
the trees' passage from seedling to adult, or from where trees cannot grow, not from slower
growth.** The judge (Astra, Chesson 2000) is **recovery from rarity**: grass started rare must
come back, not merely decline slowly.

## 3. The levers (the model)

Every number is a lever on `model::Params::meadow` (`flora/meadow.rs`), off by default; the
driver's flags are in `flora_model`'s usage line. With every lever off the model reproduces the
calibrated model's output exactly (checked: the 30-day base differs only in the new readout
lines).

| | lever | flag | form |
|---|---|---|---|
| a | `crop_per_graze`, `crop_wood_max` | `crop=K` `cropw=` | hazard on each woody juvenile with `W < crop_wood_max · donor_min`: `K · (sward foliage grazed in its zone per s) / (zone faces)`. The sward is turf, pad, stonecushion. Killed seedlings go to the dead pools (trampled, uprooted): the animal half's intake stays the logged one |
| b | `dry_every_s`, `dry_len_s`, `dry_water`, `dry_phase_s`, `dry_wet_zones` | `dry=EVERY_D:LEN_D:WATER[:PHASE_D]` | in a spell the drained zones' root-box water reads `dry_water` of the replayed value (each μ mean moved by the species' ramp); the establishment gate shuts under `establish_water_min`. Everything else is the sim's own law: turf cures, woody starves |
| c | `sward_germ`, `sward_draw` | `thatch=` `draw=` | woody overtopping through turf divided by `1 + sward_germ · fill` (turf foliage over its cap); a woody juvenile's water times `1 − sward_draw · (turf stands / zone faces)` |
| d | `lifespan_s[species]` | `life=DAYS` | adults (`W ≥ donor_min`) die at `1 / lifespan`; the face goes vacant, the stand to the dead pools |
| e | `root_soil_min[species]` | `soilmin=SP:N,..` | woody newborns (bare faces and through covers) only on the share of the zone's non-woody faces with at least N soil voxels (`Meadow::deep_share`) |
| f | `contest_escape`, `contest_tau` | `contest=ESC[:TAU]` | a woody seed coming up through turf joins `ZoneState::contest`, not the turf's face: it grows under the sward's shade `exp(−TAU · fill)`, in lever c's water, on the turf faces' mineral, cropped by lever a; at `W ≥ ESC · donor_min` it overtops its turf stand (an `Overtopped` death then) and joins the stands; if it dies first the turf keeps the face |
| | recovery from rarity | `rare=springturf:KEEP` | the turf's stands and bank thinned to KEEP at the start |

**Grazing pressure is open loop.** The take is the soak log's `foliage_taken` per 2 h window,
held at the mean of the last three windows past the log (`take_hold=1`): 64 per 2 h on s5
(57 frondgrazers at the end), 587 on s6 (361). Seed 1's log stops before its first flux line:
its take is its 414 logged frondgrazers (m600) × 1.59 per grazer per 2 h (the mean of s5's and
s6's last windows), 660 per 2 h (`take_const=660:3`). The take is spread over the species and
zones by edible stock, as the validated open loop does.

## 4. The 30-day checks (seed 6, from m370, world stands at day 30)

| arm | turf | lanternberry | vaulttree | what moved |
|---|---|---|---|---|
| base | 267 | 4,197 | 1,191 | |
| a `crop=100` | 970 | 397 | 57 | 15,664 lanternberry and 1,400 vaulttree juveniles cropped; woody held near its start |
| b `dry=10:0.5:0.2:5` | 815 | 2,245 | 1,058 | spells cost the turf 16–34 % each; woody growth halved |
| c `thatch=10 draw=0.5` | 265 | 4,068 | 1,233 | a delay only: turf 1,861 against 1,202 at day 10, the same by day 30 |
| d `life=30` | 238 | 3,878 | 1,245 | 1,433 lanternberry and 270 vaulttree dead of age; no turf gain yet (the turf is gone before the gaps open) |
| e `soilmin=L:3,B:3,V:6` | 354 | 2,697 | 659 | woody kept off the thin faces; the turf still falls |
| f `contest=0.25:1` | 271 | 4,135 | 1,208 | 2,110 lanternberry recruits came up in the turf, 1,881 died there, 192 escaped; **the turf fell as fast as in base** |
| f `contest=0.5:2` | 327 | 4,073 | 1,287 | the same |
| f + a `contest=0.25:1 crop=30` | 852 | 1,494 | 489 | |

Each lever does what it says. Two findings already:
- **The contest (f) alone does not hold the turf**, although 89 % of the woody recruits that
  came up through it died there. The woody take the drained ground mostly through bare faces
  (the turf's own turnover: its juveniles starve in shade, `flora.md`) and then shade the turf
  out. Removing the instant overtopping is necessary for a sward contest but not sufficient; it
  needs a lever that limits woody recruitment on bare ground too (a, b, e).
- **Thatch (c) only delays**, as Astra predicted for a lowered `overtop_per_check`.

## 5. The batch (launched, not waited on)

- **Where:** eidolon, `~/webmodel-meadow/` (source at ac45982d, its own `target/`), 10 runs at a
  time at `nice -n 10`. Script `~/webmodel-meadow/run.sh` (copy in the session scratchpad
  `meadow/batch/`), log `~/webmodel-meadow/run.log`, per-run `runs/log`.
- **Arms** (`~/webmodel-meadow/arms.txt`, 30): base; crop 30/100/300; dry A
  (`10:0.5:0.2:5`), B (`10:1:0.2:5`), C (`5:0.5:0.2:2.5`); sward 10 (`thatch=10 draw=0.5`) and
  30 (`thatch=30 draw=0.8`); contest 0.25:1 and 0.5:2; life 20/40/80; soil 3/3/3 and 3/3/6
  (lanternberry/bloomcrown/vaulttree); pairs crop30+life40, crop100+life40, dryA+life40,
  sward10+life40, contest25+life40, soil336+life40, contest25+crop30, crop30+dryA; the triple
  contest25+crop30+life40; **recovery from rarity** (`rare=springturf:0.05`) on base,
  crop100+life40, dryA+life40, soil336+life40 and contest25+crop30+life40.
- **Runs:** each arm × seeds 5, 6, 1, 300 model days (36,000 min) from the soak v2 m370 saves,
  replay weather seed 1, `zocc` every 5 days. 90 runs, about 6–8 min each.
- **Outputs:** `~/webmodel-meadow/runs/ARM-sSEED.txt`; the table
  `~/webmodel-meadow/runs/table.txt`; the marker **`~/webmodel-meadow/runs/DONE`**
  (`runs/FAILED` beside it if the build failed).
- **The table** (`table.py RUNS_DIR`): per run, the end state (mean of the last 50 days) as
  shares of the drained soil ground (dim/mid/bright/open/summit-drained) — meadow
  (springturf), grove (lanternberry), forest (vaulttree), other plants, bare — the days to
  settle (all three within 0.03 of the end for good), meadow at days 30 and 100, the turf's
  fall across each dry spell in the last 100 days, the turf's range, and each drained zone's
  leading species; then per arm the target check over the three seeds (`TARGET`: meadow
  0.20–0.30, grove and forest ≥ 0.05 on every seed).

**What is left once DONE lands:**
1. Read `table.txt`: which arms reach the target on all three seeds, which come near, which
   overshoot (meadow > 0.30, or grove or forest lost).
2. The rarity arms against their counterparts: does the meadow share recover to the same
   end from 5 %? An arm that holds grass only from a large start is not a niche.
3. Spatial separation: the leading-species column; a lasting meadow should lead its own
   zones or thin-soil share, not sit as a thin mix everywhere.
4. Fill section 6 with the table, then pick the smallest set for Wrysk and Astra.

## 6. Results

Pending the batch.

## 7. What each lever would mean as a sim change

| | rule or role it adds | presets it touches | fauna? |
|---|---|---|---|
| a | a frondgrazer bite on a woody stand under a band size kills it with a chance (or removes its growing tip: W to `alive_min`, the "trap"), as it grazes the sward. A role the frondgrazer already fills; the rule is new. Donaldson et al.'s caution applies: check in the sim that the grazer suppresses woody recruits and does not only release them by eating the grass | a fauna knob (`seedling_kill_per_bite` or a crop chance), a flora species knob (the band size) | **yes**: the bite outcome lives in the fauna feeding step; the flora needs a take call that removes a stand |
| b | none: longer, deeper dry spells in the drained ground through the weather preset. The sim's woody have no cure; the turf cures for up to 1.83 days. Risk: at zero income an adult lanternberry starves in ~3.7 h, so a spell deep and long enough to kill seedlings can also open gaps among adults (which the turf can use) | `[world.weather]`/climate in the terrarium presets | no |
| c | thatch on overtopping through turf (`overtop_per_check / (1 + g · fill)`), and woody juveniles reading the topsoil layer the turf drinks from | flora species knobs (`overtop_thatch`, `juvenile_sward_draw`) | no |
| d | a stand lifespan: a birth tick on `Stand` (schema bump: fresh worlds, no migration), a per-species median life with spread, a new `DeathCause::Aged` | flora species knobs (`lifespan_median_s`, spread) | no |
| e | a rooting-soil requirement: a woody newborn (and, per Astra, a growing woody stand, as its size asks more) needs N soil voxels under its face; a new gate in `establishment_gates_with_sky` beside `pore_ok` | flora species knob (`root_soil_min`) | no |
| f | a recruit stage: a woody seedling coming up through turf becomes a recruit beside the turf (a second place on the face, as the fungus has its own `FaceLayer`), shaded by it and drinking its water, and overtops it only on escape | flora species knobs (`recruit_escape`, the sward's shade over a recruit) | no (unless a is taken too: the crop then reaches recruits) |

## Roles (balance through roles)

| stock growing or shrinking without check | the role that should answer |
|---|---|
| woody occupancy of drained ground, absorbing (no adult ever leaves) | a lifespan (d) or a disturbance role; Astra suggests a large browser damaging crowded woody patches, deferred until the recruitment levers are measured |
| woody recruitment on the turf's own turnover gaps | the frondgrazer cropping seedlings (a); dry spells (b); thin soil (e) |
| the turf, gone by about day 25 on s5/s6 | a refuge it wins: thin soil (e), a grazing lawn (a), or recurring drought (b) |
