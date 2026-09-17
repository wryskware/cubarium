---
status: leaning
date: 2026-09-17
owner: Fable
---

# Voxel round 4: three producer presets, niche contracts first

Round 3 (main at 6cbe4af, cleared by Astra round 6 at e32f276) left a substrate
with an organic-plus-mineral ledger, paid seed banks, root aeration and a bounded
water budget. It cleared the substrate for more presets; it did **not** certify
habitat, self-replacement or coexistence, and this round does not try to. It adds
**springturf, stonecushion and velvetpad** as `Species` with stated roles, each
with one paid birth, a short survival check and a failing neighbour, and reruns
the short experiment with five species. **No animals, no new hydrology, no long
study.** The replacement-control study Astra designed in round 5 (R5.4) stays a
separate later package.

Direction (Wrysk, 2026-09-17): the simulation and ecology get solid for an
**ambient piece** first; the game layer waits. So nothing here adds hooks,
currencies or gates; the deliverable is a producer community that looks alive.

## The contracts (Astra R6.1, R6.2)

State the role first; the numbers are placeholders that encode it and are named
as such in the backlog. Roles that already exist:

- **Bloomcrown** — the light-demanding producer of sunny, aerated soil. Baseline,
  no occupancy target, no threshold change to recover the old 96-column count.
- **Umbrellafrond** — the **wetland** producer: soil roots with sustained wetness,
  saturation tolerance 1, the existing free-water depth limit. Kept as is
  (`establish_pore_min` 0.45 stays; do not lower it as a repair).

New roles, each one sentence of ecology and the traits that encode it:

- **Springturf** — the **pioneer turf** of open, moist soil: shallow roots
  (depth 1, radius 1), sun-demanding (`light_half` high, `establish_light_min`
  high), fast (high `propagule_rate`, small `alive_min` and `wood_max`, high
  `maintenance` so it is short-lived), wide hop (3), low crown (height
  `[0.5, 1.0]`, radius `[0.5, 1.0]`). Loses under canopy and on dry ground; wins
  the first years on bare soil.
- **Stonecushion** — the **cushion of bare rock**: a stand whose support face is
  rock or bedrock and whose roots reach a **soil pocket** within radius 1,
  drought-tolerant (`wilt_pore` very low, `establish_pore_min` very low), slow
  (low rates, low maintenance, tiny crown `[0.5, 0.5]` / `[0.5, 1.0]`), hop 1,
  little light need beyond open sky. Today the gates and the root box are
  soil-only (`step.rs` around the support/soil checks Astra cites at 396, 1188,
  1305). If the current gates refuse a rock support, add **one explicit rule**:
  a species flag (say `rock_support: bool`, placeholder) under which a rock or
  bedrock support face is accepted **provided the root box holds at least one
  soil voxel**; water is still drawn only from soil voxels. Say in the commit
  message that this is the rule Astra R6.2 named as an explicit addition.
- **Velvetpad** — the **moist, aerated understory** pad (the niche Astra R4.6
  separated from umbrellafrond's wetland): shade-tolerant (`light_half` small,
  `establish_light_min` low), damp but not waterlogged soil (`establish_pore_min`
  around 0.3, `wilt_pore` 0.2, `sat_pore` 0.6, `establish_saturated_max` around
  0.6 so a wholly saturated box does stress it), roots depth 2 radius 1, hop 1,
  low broad crown (height `[0.5, 1.0]`, radius `[1.0, 2.0]`).

Choose the three thresholds of a role **together** (pore floor, wilt, sat) so a
role is consistent, and write the reason in the preset's doc comment as the
existing two presets do.

## Shared boundary

- `crates/cubarium-voxel-flora/**`: `Species` grows to five (`ALL`, `index`,
  `name`, `parse`); `FloraConfig` gains three fields and presets; every
  `[f64; 2]` indexed by species in `FloraLedger` becomes `[f64; Species::COUNT]`;
  add `SpeciesConfig::validate()` (finite nonnegative split summing to one,
  positive wood fraction, `alive_min <= wood_max`, rates finite) and call it from
  `Flora::new` / config load with a clear error.
- `crates/cubarium/src/voxel/stand.rs`: palettes and `seed_style` for the three
  new species inside the Outrun family (`design/appearance.md`); crown geometry
  must give a turf, a cushion and a pad at least one cell each. `mod.rs` is owned
  by the render package this week: touch it only if `f X Z <species>` does not
  already go through `Species::parse`, and then only that line.
- `crates/cubarium-voxel-flora/examples/two_producers.rs`: species by name
  everywhere it is hard-coded; `compare` and `chesson` accept any pair; add
  `community <s> [seed] [noise]`: all five founders on their contract habitat
  (bloomcrown open ridge, umbrellafrond wet hollow, springturf bare moist soil,
  stonecushion rock with a soil pocket, velvetpad under a founder's crown), the
  same per-species report (births, deaths, descendants by identity, gate
  diagnosis at introduction and observation).
- Backlog rows in `design/backlog.md` §1 for every new placeholder, including
  `rock_support` if added.
- The core (`cubarium-voxel`) does not change. If a query is missing, add it with
  a test and say so.

## Tests each preset must leave behind (short function tests only)

Per new species, in a new `tests/round4.rs`:

1. **Paid birth**: a bank holding one whole package on a site passing that
   species' gates germinates into a stand at exactly `alive_min` wood; the
   donor-side ledger (`propagule_requested/funded/landed`) and both residuals
   hold.
2. **Survival and income**: the newborn, on its contract habitat with ample
   light and water for that role, has positive net income over 200 ticks and does
   not die.
3. **Failing neighbour**: the same package on the neighbouring condition the role
   excludes never germinates and falls to litter with its mineral (springturf
   under a dense crown; stonecushion on rock with **no** soil pocket in reach;
   velvetpad on a wholly saturated box, where germination is refused, plus an
   adult velvetpad placed there reaching stress above zero).
4. **Validity**: `validate()` passes all five presets and rejects one deliberately
   broken clone (split summing to 1.1; `alive_min > wood_max`).

Plus one presenter test in `stand.rs`: each new species stamps at least one cell
and its palette is distinct from the other four.

## Reporting corrections carried (Astra R6.3)

In `design/7_Research/voxel-round3-experiment-2026-09-16.md`: the diagnostic
runs 400 **coupled** seconds after a 50 s warm-up, so the final age is 450 s (not
"350 s later"); limit the gate claims at :630 and :640 to their measured states;
scope "pre-K7" at :687 to the three-arm comparison; 0.1 % at :689 becomes
0.005 %. In `two_producers.rs`: drop "settled water" from the observation label
(:973) and call the size difference at :982 a net count change. In
`lib.rs` (:342) write construction as `gross − gross / (1 + build)`.

## Then

- Run `community 400` in `--release` on the default generated world and one
  noise reseed; append "Round 4 — 2026-09-17" to the experiment note with the
  per-species table and the gate diagnosis. Say how long the run took. This is a
  smoke run, not a study; do not add arms.
- Fable reads the diffs, runs the crate tests, and launches the Astra round.

## Packages

**R4 — presets**, Opus high, main checkout, files as listed above. Commits per
step: species enum + validate; each preset with its tests (three commits);
harness `community`; presenter palettes; docs corrections. Explicit paths only.

## Rules

Fast iteration; explicit-path commits; no cargo fmt; no knob tuning beyond
placeholders named in the commit and the backlog; no long simulation tests, no
pinned hashes; always fresh; do not touch `design/handoffs/README.md`, the cube,
`crates/cubarium-gpu/**` or `crates/cubarium/src/sink/**`; never HashMap
iteration.
