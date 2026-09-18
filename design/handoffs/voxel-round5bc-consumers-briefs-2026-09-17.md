---
status: leaning
date: 2026-09-17
owner: Fable
---

# Voxel rounds 5b and 5c: both first consumers, in parallel

Wrysk (2026-09-17): "idc which goes first, but i kinda want both." Round 5a
(`voxel-round5a-transfers-briefs-2026-09-17.md`, package M) gives the flora
layer bounded withdrawals (`take_foliage`, `take_dead_wood`, `take_litter`),
deposits (carrion, litter) and a reach query. On top of it, two packages run in
parallel worktrees because they live in different crates:

- **5b glowcap** (package N) — a heterotrophic wood fungus as a *stand* in the
  flora crate: the biosphere's "decomposer grove" (§5 branch 2) and its
  substrate request "non-photosynthetic stand metabolism: reuse stand
  location/lifecycle structure, replace income and substrate rules; no light
  income" (§6).
- **5c frondgrazer** (package O) — a paid ground browser in a new crate
  `cubarium-voxel-fauna`: the "grazed meadow" (§5 branch 1) and the substrate
  request "geometric feeding access: standable supports, bounded travel, mouth
  reach" (§6).

Astra's round 7 frames both: explicit bounded transfers and the
organic/mineral/energy ledger; **population targets and carrying capacity stay
unclaimed**. Direction: ambient piece, no game hooks.

**The look is not the workers' call.** The art direction is
`design/art-direction/Cubarium_Art_Direction_v0.1.md` (Wrysk, 2026-09-17). The
current voxel presenter and its interim glyphs are **dev mode** in that
document's terms: a diagnostic view kept available, not the production look.
Packages N and O shipped interim glyphs named as interim; the production look
(stage 2, graphic pixel ecology: organic terrain contours, sprite organisms) is
its own render round citing that document. The study at 68a8215 is paused and
not canon.

## 5b — glowcap (package N, flora crate)

A new `Species::Glowcap` whose `SpeciesConfig` carries a `trophic` mode
(`Photo` for the five plants, `Saprotroph` for this one). What changes by mode,
and nothing else:

- **Income.** No light. Each tick a saprotroph withdraws from the dead-wood
  pools of the sites in its **mycelium box** (the existing root box, over
  `rooting_depth`/`rooting_radius`, read as support sites rather than soil
  voxels) at most `substrate_uptake_per_s · wood · dt` organic matter, pro rata
  across those pools, using `take_dead_wood` so the flow is the same one a
  consumer would use. The organic taken becomes tissue at `yield` (placeholder),
  the rest is respired (`respired_out`, heat). Mineral arrives with the wood at
  the pool's own ratio; what exceeds `n_tissue · built` is released to the site
  pool, what falls short is drawn from the site pool under the existing
  mineral cap. Moisture (`μ`) multiplies uptake exactly as it multiplies
  assimilation, so a drying log starves the fungus.
- **Gates.** Germination needs dead wood in the box of at least
  `establish_substrate_min` (placeholder), mean pore between the species'
  floor and its saturation ceiling (aerated attachment), and **no** light gate.
  Everything else (bank, lottery, one package, newborn at `alive_min`) is
  unchanged.
- **Stocks.** `wood` is mycelium, `foliage` is fruiting caps (senescence sends
  them to litter as today), `reserve` as today. Propagules are spores through
  the same paid parcel and seed bank; `hop` 1.
- **Death.** Food exhaustion or drying is unpaid maintenance, dieback, death by
  the existing rules; remains go to litter. No new kill switch.

Harness: `community` gains glowcap founders on sites holding dead wood; since a
fresh world has none, the harness lays down declared **log deposits** with 5a's
`deposit` (add `Deposit::DeadWood` to 5a's kinds if M did not; say so). The
smoke run is the existing `community 400` with six species. Backlog rows for
every placeholder (`substrate_uptake_per_s`, `yield`, `establish_substrate_min`,
and the preset's own thresholds).

Tests (`tests/round5b.rs`, short): a glowcap on a dark site with a log earns and
a lit site without one does not; uptake never exceeds the pool and stops when it
is empty; mineral is conserved along wood → fungus → litter → site pool with
residuals at noise; germination is refused without substrate and passes with it
under otherwise identical conditions; one spore package births at exactly
`alive_min`; a glowcap on a small finite log has a non-increasing pool, income
falling to zero and a falling reserve over 200 ticks (direction, not a death
claim); `validate()` covers the new fields; presenter test: the interim glyph
stamps one cell with a distinct palette.

## 5c — frondgrazer (package O, new crate `cubarium-voxel-fauna`)

A fixed-step, deterministic animal layer that borrows the world and the flora
per tick, in the same shape as the flora borrows the world. Not neural, not the
cube's hunter: a **heuristic** browser whose every quantity is paid.

- **Animal** `{ id, species: Frondgrazer, site: Site (the support face it
  stands on), body: f64 (organic structure), reserve: f64, mineral: f64,
  age_ticks, state: Cropping | Walking | Resting }`. Species config
  (placeholders, backlog rows): `maintenance_per_s`, `bite_per_s`, `yield`,
  `n_tissue`, `body_max`, `body_min` (dies below), `birth_body` (adult),
  `birth_cost`, `reserve_cap`, `reach: Reach { horizontal: 1, up: 1 }`,
  `climb` (max face height difference per step, voxels), `wade_depth_m`,
  `drown_depth_m`, `step_period_s`, `sense_radius`.
- **Tick.** (1) maintenance: `maintenance_per_s · body · dt` organic respired
  from reserve, then body when reserve is empty (dieback), mineral kept;
  (2) sense: `reachable_foliage` from the current site, and the best site
  within `sense_radius` support faces by foliage in reach; (3) act: if reach
  holds ≥ one bite, crop (`take_foliage(site, bite_per_s · dt)`), assimilate
  at `yield`, respire the rest, mineral pro rata with excess over `n_tissue`
  excreted as a litter deposit (dung is litter this round); else step one
  support face toward the best site every `step_period_s`, only across faces
  whose height difference is ≤ `climb` and whose water depth is ≤
  `wade_depth_m`, wrapped in `x`, ties broken by a keyed splitmix64 stream
  `(domain, world seed, animal id, tick)`; if nothing is in sense, rest;
  (4) births: an adult (`body ≥ birth_body`) with `reserve ≥ birth_cost` pays
  it and a newborn appears on the same site at `body_min` body with the
  remainder as reserve, mineral by the same fraction rule; **assumption, stated
  in the crate doc:** no mating system this round, one parent pays (the
  biosphere says a single specimen is not a population; that is a later
  contract); (5) death: `body < body_min` or water over the site deeper than
  `drown_depth_m` → carrion deposit of body + reserve with mineral, `deaths`
  += 1. Order per tick: maintenance, sense/act, births, death; say so in one
  place.
- **Ledger** `FaunaLedger { eaten_organic_in, eaten_mineral_in,
  eaten_energy_in, respired_out, deposited_organic_out, deposited_mineral_out,
  deposited_energy_out, births, deaths }` with its own residuals. A **union**
  test asserts flora `consumed_*` equals fauna `eaten_*` and flora
  `deposited_*` equals fauna `deposited_*`, so the two layers close together.
- **Commands** `Introduce { x, z, body }` on the highest support face, `Remove
  { x, z }`; the host's stdin gets `g X Z frondgrazer [body]` in
  `crates/cubarium/src/voxel/mod.rs`.
- **Presenter** `crates/cubarium/src/voxel/animal.rs`: the interim 2×1×2 block
  stamped through the same part/style path as plants (a new part id range),
  so the CPU presenter and the GPU voxel texture both draw it; sub-tick motion
  interpolation is **not** this round (the art thread decides how a body moves
  between faces; `pace-in-body-lengths` applies later).
- **Snapshot**: the fauna serializes beside the flora under the existing
  always-fresh rule (a new schema refuses old worlds).

Harness: a `grazed <s> <n> [seed] [noise]` mode in the fauna crate's own
example (or in `two_producers.rs` if sharing the placement helpers is
simpler; the flora example would then depend on the fauna crate as a
dev-dependency): two arms from one conditioned springturf-plus-bloomcrown
world, plant-only versus `n` grazers introduced on gate-passing open-soil
sites at the halfway point; report per 100 s the animals' count, mean body
and reserve, bites taken, distance walked, births and deaths, the producers'
foliage/reserve/wood trajectories in both arms, and the union residuals. 400
coupled seconds in `--release`, wall time stated. No viability claim: the
question is whether every quantity is paid and observable.

Tests (`tests/round5c.rs`, short): a grazer next to reachable foliage crops
and the stand loses exactly the bite; foliage two voxels up is not eaten;
maintenance drains reserve then body; a starving grazer dies at `body_min`
and its carrion appears on its site with its mineral; a step never crosses a
face higher than `climb` or water deeper than `wade_depth_m`; a birth pays
`birth_cost` and the newborn is at `body_min`; the union ledger closes over 100
coupled ticks with two grazers on a fixture; the keyed stream makes two runs
identical and a different world seed different.

## Packages

- **N — glowcap**, Opus high, worktree `voxel-glowcap` from M's last commit;
  files `crates/cubarium-voxel-flora/**`, `crates/cubarium/src/voxel/stand.rs`,
  backlog, experiment note section "Round 5b".
- **O — frondgrazer**, Opus high, worktree `voxel-fauna` from the same commit;
  files `crates/cubarium-voxel-fauna/**` (new, in the workspace), the new
  `crates/cubarium/src/voxel/animal.rs`, the minimal wiring in
  `crates/cubarium/src/voxel/mod.rs` and `crates/cubarium-gpu/src/voxel/**`
  (a part id range only), backlog, experiment note section "Round 5c".
- **Art** — Wrysk's own art-direction thread (see the handoff above); when
  `design/voxel-art-direction.md` lands, a small follow-up package replaces
  the interim glyphs with what it specifies.
- Astra rounds on 5a+L, then on 5b and 5c together.

## Rules

Fast iteration; explicit-path commits; no cargo fmt; placeholders named in
commits and the backlog, no tuning; short function tests only, no pinned
hashes; always fresh; do not touch `design/handoffs/README.md`, the cube, the
toy on port 7402, or the other package's crate; never HashMap iteration.
