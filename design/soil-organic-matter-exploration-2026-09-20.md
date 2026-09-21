---
design_status: exploration
last_reviewed: 2026-09-20
decision_refs: []
---

# Soil organic matter: a storage and food-web proposal

Requested by Wrysk through `handoffs/voxel-soil-pool-brief-2026-09-20.md`. This note proposes no accepted
decision and changes no crate. D-0001–D-0003 in `0_Canon/DECISIONS.md` establish authority, product intent
and working rules; none chooses a soil or nutrient mechanism. The ecology contract, terrain proposal and
voxel sketch remain explorations. Their older flat-world formulas are context, not evidence of the voxel
implementation.

## 1. Flows today: mineral recycling already exists

**The world is not simply mining a nonrenewable soluble mineral reserve.** Litter, wood and carrion decay
release their mineral companions to the site's `Ground::mineral`, while organic matter is respired and
energy becomes heat (`crates/cubarium-voxel-flora/src/step.rs:1179`, `:1237`). This is recycling, not
creation of new mineral. Total mineral remains finite after external inputs cease, but the same mineral can
fund successive generations.

Diagram key: O = organic matter, N = mineral, E = usable energy; arrows are transfers except the named
external inputs/outputs. Evidence follows each group.

```text
external light/CO2 boundary -> fixed O + E -> plant foliage / wood / reserve
site free N -----------------------------> new tissue N
reserve -> paid parcel -> seed bank -> new stand
foliage senescence -----------------------> litter (O,N,E)
dead foliage / reserve / parcel ----------> litter (O,N,E)
wood dieback / stand death ---------------> dead wood (O,N,E)
seed attrition / expiry ------------------> litter (O,N,E)
```

Income and construction: `crates/cubarium-voxel-flora/src/step.rs:792`; senescence/dieback: `:1024`; death:
`:1088`; paid parcels: `:1639`; seed-bank recruitment: `:1296`; attrition/expiry: `:1502`. The living
inventory has wood, foliage, reserve and parcel, with one mineral inventory; it has no separate root biomass
(`crates/cubarium-voxel-flora/src/lib.rs:205`).

```text
dead wood -> glowcap tissue + respiration/heat + excess N to site
foliage -> browser body/reserve; litter -> littershredder body/reserve
animal reserve -> offspring body/reserve (N,E travel with it)
animal death -> carrion (O,N,E)
animal digestion -> respiration/heat + excess N directly to site
litter / dead wood / carrion -> respiration/heat + N to site
```

Glowcap withdrawal/assimilation: `crates/cubarium-voxel-flora/src/step.rs:691`, `:887`, `:1001`. Founder
feeding: `crates/cubarium-voxel-fauna/src/step.rs:699`; assimilation: `:821`; paid births: `:960`; death:
`:1017`. Dung here contains excess mineral and **zero organic matter**; although sent as a litter deposit,
it reaches free N immediately through the zero-organic deposit branch
(`crates/cubarium-voxel-fauna/src/step.rs:856`, `crates/cubarium-voxel-flora/src/lib.rs:2439`). It is not a
continuing supply of edible litter. The blind founder eats litter, not carrion, in this checkout
(`crates/cubarium-voxel-fauna/src/step.rs:729`); the broader diet in `voxel-ecology-sketch-2026-09-16.md` §1
is a proposal.

**S2 baseline:** this exploration assumes glowcap also eats litter and can establish on litter, as requested
in `handoffs/voxel-decomposers-and-defaults-2026-09-20.md` §S2. That is not yet the inspected
implementation: `feed` withdraws dead wood only (`crates/cubarium-voxel-flora/src/step.rs:691`). Soil would
extend that S2 baseline; litter feeding should not be credited to soil.

Sources of free N today are initial provisioning, the three detrital decay flows, fungal surplus mineral,
and mineral-only deposits. External mineral also arrives inside seeded founders and caller-supplied deposits
(`crates/cubarium-voxel-flora/src/lib.rs:2200`, `:2401`). Default provisioning is **lazy**: first
colonisation/deposition imports a booked `initial_mineral`; `AtCreation` instead lays it on every support
face at startup (`crates/cubarium-voxel-flora/src/lib.rs:1349`, `:1395`, `:2122`;
`crates/cubarium-voxel-flora/src/step.rs:1701`). Thus the opening stock is not necessarily the last external
input. No weathering or waterborne nutrient source appears in these ground-pool rules or their boundary
ledger (`crates/cubarium-voxel-flora/src/lib.rs:1473`, `:1572`).

The flora ledger closes O as seeded + fixed + deposited − respired − removed − consumed; N as seeded +
deposited − removed − consumed; E as seeded + light + deposited − heat − removed − consumed. Stored totals
include stands, detrital stocks and seed banks (`crates/cubarium-voxel-flora/src/lib.rs:1565`, `:1739`,
`:1916`). Animal eating/deposition crosses the flora/fauna boundary; the same transferred amounts cancel
when their books are combined. Fungal uptake is internal to flora, not `consumed_*_out`
(`crates/cubarium-voxel-flora/src/lib.rs:1497`, `:1548`).

Water is a separate currency. In the open budget rain enters; evaporation, transpiration and the outlet
leave. In the closed budget those losses enter the atmosphere store and can rain back; they do not carry N
in this ledger (`crates/cubarium-voxel/src/ledger.rs:7`, `:52`). Plant pore withdrawals book transpiration
(`crates/cubarium-voxel/src/water.rs:1673`). Organic respiration and heat remain external ecological losses;
closed water does not close carbon or energy. Terrain edits/removal commands can explicitly export mineral
and organic stocks (`crates/cubarium-voxel-flora/src/step.rs:220`,
`crates/cubarium-voxel-flora/src/lib.rs:1488`).

## 2. Where a soil pool could sit

**Soil nutrients remain available to plants.** Wrysk's follow-up correctly separates accessible soil
nutrients from minerals locked in rock. The proposed organic pool is only one part of soil: keep available
soil N alongside it, represented by `Ground::mineral` (`crates/cubarium-voxel-flora/src/lib.rs:306`). It is
not a rock inventory. Available N should remain directly usable without first passing through a fungus.
Nutrients bound inside organic compounds need release; nutrients dissolved in soil water, or exchangeable
from soil surfaces, are a different category. Rock-bound nutrients require weathering before joining that
supply. For example, [UMN Extension's phosphorus
account](https://extension.umn.edu/agriculture/crop-production/nutrient-management-for-minnesota-crops/understanding-phosphorus-in-minnesota-soils)
distinguishes solution uptake, organic mineralisation and rock weathering; [its soil-fertility
text](https://open.lib.umn.edu/horticulture/chapter/12-1-soils-fertility-and-plant-growth/) also describes
nutrient retention by clay and humus. This model could aggregate solution/exchangeable nutrients as
available N without simulating that chemistry. Adding soil must not reclassify the existing available N as
inaccessible organic N; only nutrients actually transferred with organic tissue enter the bound pool.

Recommend an aggregate O/N/E pool **per support face beside Ground**, initially zero unless an explicitly
booked opening allocation supplies it. Existing ground stocks already have this identity and separate
currency companions (`crates/cubarium-voxel-flora/src/lib.rs:304`). A single `(x,z)` column pool would
conflate sheltered floor and roof; the terrain proposal §2 and voxel sketch §2 explicitly distinguish these
surfaces. Per-soil-voxel storage would represent depth and excavation better, but would require new
ownership and root/mycelium access rules. A face pool is an aggregate soil horizon, not a claim that all
soil carbon lies on its surface. Restrict initial soil storage to soil-supported faces; retain ordinary
litter on rock and declare any later organic-mat-on-rock extension separately.

Candidate feed paths: divert a declared fraction of litter, carrion and dead-wood decay into soil, retaining
that fraction's O/N/E; the remainder follows direct respiration/heat/mineral return. Then a slow soil-decay
path releases its own N to free N and loses O/E to respiration/heat. Saprotrophs could draw soil through the
same bounded substrate budget as litter/wood. Plants would draw available N, including released N; animals would not draw soil
in this first proposal. No automatic soil replenishment or new microbial population is implied.

Root turnover on stand death is a possible additional input **only by reallocating existing tissue**. There
is no root stock to harvest: death currently sends wood to dead wood and other organic stocks to litter
(`crates/cubarium-voxel-flora/src/lib.rs:205`, `crates/cubarium-voxel-flora/src/step.rs:1088`). Defer a root
fraction unless Wrysk wants this semantic split; never add inferred roots on top of those totals.

The ecology contract §5 suggests bounded pre-tick withdrawals, distinct decay clocks, proportional
companions and one-way energy loss. Its flat-world `material -> N` formula is **not** the voxel rule: voxel
organic matter respires while its separate mineral companion returns
(`crates/cubarium-voxel-flora/src/step.rs:1237`). Current rates are litter 0.001/s, wood 0.0001/s, carrion
0.005/s, not measured soil rates (`crates/cubarium-voxel-flora/src/lib.rs:1395`). Humification fractions,
soil decay rate, fungal soil accessibility and any moisture response would be new named placeholders,
documented under `backlog.md` §1 if implemented. No existing contract supplies numerical values for them.

## 3. Effects across the ecology and double-counting hazards

**Nutrition:** soil adds residence time to an existing loop. It can preserve mineral locally for later
release, but initially immobilises mineral that direct decomposition would already have released. This may
reduce producer income: all photosynthetic income, including maintenance funding, is gated by free site N;
mineral retained inside a stand is not a reusable reserve (`crates/cubarium-voxel-flora/src/step.rs:835`,
`crates/cubarium-voxel-flora/src/lib.rs:235`). Soil does not fix that separate physiology limitation or move
a neighbour's free N into the stand's own pool.

**Competition:** under S2, shredder and glowcap share litter. Giving only glowcap soil offers it an
alternative resource, but converting litter to soil also removes accessible shredder food. Preserve litter's
distinct identity and only divert a bounded decay fraction; do not rename every litter tile soil. Current
founder feeding depends on a mouth-accessible litter site (`crates/cubarium-voxel-fauna/src/step.rs:729`).
The proposal could ease direct competition or favour fungi at the shredder's expense; neither outcome is
established.

**No-death floor:** living foliage still senesces and seed cohorts still attrit, so stand death is not
required for detrital income (`crates/cubarium-voxel-flora/src/step.rs:1024`, `:1502`). Soil could buffer
those inputs. A finite soil stock with no replenishment must eventually run down under positive
uptake/decay; it is not a permanent decomposer subsidy. Water, establishment, spatial access and usable
energy must still permit growth.

**Boom and crash:** a slow store may spread a mortality pulse over time, but it may also delay mineral
return enough to deepen a producer trough or starve shredders sooner. This is a testable hypothesis, not a
stability claim; the same initial total O/N/E must be used when comparing direct versus stored return, so
adding a richer opening world cannot masquerade as damping.

Required accounting changes if implemented:

- Debit each detrital withdrawal once, split its O/N/E between retained soil
  and immediate losses; never retain soil and also book its O/E as lost or N as free.
- Fund fungal soil uptake from the remaining stock, once; include new soil in
  the pre-tick eligibility snapshot so arrivals cannot traverse two decay stages
  immediately. Existing snapshot/order: `crates/cubarium-voxel-flora/src/step.rs:168`.
- Add soil to stored totals, removal accounting and save/load validation;
  existing totals/pruning: `crates/cubarium-voxel-flora/src/lib.rs:1739`,
  `crates/cubarium-voxel-flora/src/step.rs:220`. Book player imports/exports explicitly.
- Keep mineral-only dung's direct N return; do not turn it into edible soil.
  Existing terminal rule: `crates/cubarium-voxel-flora/src/lib.rs:2439`.

## 4. Simulation cost and water coupling

Three f64 companions cost **24 bytes per represented site**, excluding padding or indexing. At the profile's
3,072 columns that is 72 KiB for one support each; additional supports multiply by their actual count. Three
dense arrays over 147,456 voxels would cost 3.375 MiB, including nonsoil cells if not compacted. Those
dimensions are the measured fixture, not a current-world census
(`7_Research/voxel-tick-profile-2026-09-18.md:13`).

Run local soil decay in flora's existing ground decomposition pass, retaining its snapshot semantics
(`crates/cubarium-voxel-flora/src/step.rs:168`, `:1179`). No lateral diffusion or dissolved-nutrient
transport is needed for a first local store. Water can initially remain an environmental read, not an O/N/E
carrier. Even vertical leaching would require a dissolved N stock, exchange rules and nutrient export
accounting; defer it rather than attaching nutrient loss to water's outlet counter. Later depth-resolved
soil would need vertical exchange explicitly, with lateral transport a further choice.

The requested profile reports 1.44 µs for three-pool decomposition with 61 ground sites, and later 42.2 µs
flora / 1.735 ms total at 16 threads; its final rerun is 1.750 ± 0.089 ms/tick
(`7_Research/voxel-tick-profile-2026-09-18.md:62`, `:161`, `:395`, `:498`). Rough arithmetic: one extra
local pool is about a third of the earlier decomposition work, ~0.5 µs at 61 sites; scaling linearly to
3,072 sites gives ~25 µs. Allow **25–75 µs/tick at ~3k sites** for diversion and extra snapshot work,
~1.4–4.3% of that 1.75 ms historical tick. This is an unmeasured engineering estimate, excludes new fungal
reach searches and water transport, and is not a forecast for today's populated world.

Use the existing represented ground sites rather than scan the voxel volume. There is no need yet for a
separate water-style active set: `Ground` is already site-based, though `AtCreation` populates all support
faces (`crates/cubarium-voxel-flora/src/lib.rs:2122`). Measure the extra pass before adding another index;
nonempty-soil tracking becomes useful only if empty-site work dominates. No benchmark or capture was
launched for this read-only note.

## 5. Presentation and game

Soil O per represented area could drive an artist-selected colour/texture state, with moisture a separate
input. This proposes data, not a palette or look. Follow `art-direction/Cubarium_Art_Direction_v0.1.md`
(working agreement and “Ecology first”): production art is selected by Wrysk, and diagnostics stay out of
the normal world. Total O and concentration should not be conflated if soil depth is represented later.

Mushrooms could appear in rich soil through paid establishment and substrate access, rather than decorative
spawning. Richness would be one opportunity, not a guarantee: viable propagules and habitat gates still
matter. The current seed-bank lottery checks packages and establishment
(`crates/cubarium-voxel-flora/src/step.rs:1296`); the proposed soil path should extend that mechanism and
leave its visual treatment to the art direction.

A player could enrich or remove local organic matter alongside the requested water lever, choosing between
immediate litter food and slower nutrient storage. Both actions would transfer declared O/N/E through
explicit boundaries; moving compost inside the world would debit its donor. Water already has named user
input accounting (`crates/cubarium-voxel/src/ledger.rs:9`). Costs, rewards, UI and automatic fertility
targets remain undecided game design.

## 6. Bounded packages and choices for Wrysk

1. **Storage and mineral return:** add support-face soil O/N/E, bounded detrital
   diversion and local mineralisation, totals and explicit removal handling.
   Test: a two-tick fixture conserves all currencies, delays new soil's decay,
   and releases exactly the mineral that leaves its soil companion.
2. **S2 fungal integration:** add soil to the shared litter/wood substrate budget
   and establishment query, keeping shredder litter-only and soil initially finite.
   Test: one fungus and one shredder share a small mixed site for ≤100 ticks
   without overdrawing any stock or breaking combined ledgers.
3. **Player/data exposure:** expose explicit soil addition/removal and a read-only
   richness value for subsequent Wrysk-directed art; retain ordinary dev controls.
   Test: one addition and removal change only the intended site's inventory
   and named external totals by the accepted amounts.

Wrysk would be deciding:

- Support-face aggregate, whole column, or depth-resolved voxel granularity.
- Whether soil mineralisation supplements direct detrital return (recommended)
  or replaces it as the principal recycled source of plants' free N.
- Whether soil is exclusive to saprotrophs or also edible by shredders.
- The diverted fractions, soil residence/access rules and opening inventory;
  whether any existing tissue is reclassified as roots, and whether waterborne
  nutrient transport is wanted at all. None is selected by this note.
