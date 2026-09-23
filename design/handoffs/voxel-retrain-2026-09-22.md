---
status: open
date: 2026-09-22
owner: Fable (organism line), under Wrysk's delegation of the nine decisions
package: 5 of design/handoffs/voxel-organism-decisions-2026-09-21.md §9 / audit §10
---

# Package 5 — contract v2, training worlds, one policy refresh

## Why

Packages 0–4 are on main (40bdb4a). Across eight seed bases, placement is no
longer what kills the browser: the minute-3.4 death on `default` seed 1 was a
founder that started with food in reach, ate, walked 10.8 m to the strip's
front edge, held full forward against a drop for a minute, then walked into
water and drowned (census "Startup acceptance, 2026-09-22"). The two shipped
centres (`frondgrazer-p3d-reach-gen390`, `littershredder-p3c-wander-gen441`)
were trained on a flat 8 × 4 m arena with a static pond, one body, no terrain
and no drowning — nothing died in 1.278 G training ticks — and the live
world has since changed under them (step rule, bodies in metres, layers,
diets, startup acceptance).

The scoping pass behind this brief (2026-09-22, read-only, main at 40bdb4a)
found the gaps below; file:line references are its.

| # | Trained with → live now | Handled in |
| --- | --- | --- |
| a | eye 1.5 cells → 0.8·H in metres (senses.rs:739-760); not in the digest | A |
| b | pitches [−20, 0, 20] → decided −40..+40 (decisions §6), not implemented | A |
| c | any water cell / cell above a detritus pool is a wall (senses.rs:626-700) → decided "occlude only below physical height" | A |
| d | manifest 0.25/0.125 m (digest only) → physiology 0.375/0.19 m × (body/body_max)^(1/3); newborns (0.46 L) never trained | A (digest), B (sampled sizes) |
| e | mouth lift 1 voxel → band [0, 1.33·H]; arena Stage B patches at rise 1 are now **unreachable** (arena.rs:126-139) | B |
| f | flat arena; z-edges and drops read open to rays and contact (body.rs:817-820, senses.rs:690-694) | A (edges), B (terrain) |
| g | static pond, nothing drowns; live bodies drown although motion refuses water > 0.05 m (body.rs:615, step.rs:1435) — **unexplained** | A, first |
| h | two species, one slab each; litter only → every vascular species per layer; caps and carrion | B |
| i | `Chem(litter)` now carries litter + carrion | A (rename) |
| j | cruise 0.67 BL/s | A |
| k | one body per arena; six body channels always zero | B |
| l | `Self.birth_readiness` omits surplus-hold, refractory, egg rules (body.rs:1302-1306) | A |
| m | 0.25 m arena only; `small` runs at 0.125 m | B |

Input counts stay 23 (blind) / 37 (browser). The digest moves, the loader
refuses a warm start across it (trainer.rs:561-586), and worlds are always
fresh — so this is **from scratch: new heuristic teachers, a new imitation
clone, then ES**, the P3-C path.

## Decisions (made here, Fable)

- **D1 Contract v2.** Manifest `SCHEMA_VERSION` 2; its canonical text carries
  the physical anchors — adult length and height, eye at 0.8·H, mouth band
  [0, 1.33·H] and reach 0.25·L, contact geometry, `climb_m`, wade and drown
  depths, the five pitches, `Chem(detritus)`, cruise, the occlusion rule and
  the corrected `birth_readiness`. The body's meaning changed twice on
  2026-09-22 without the digest noticing; it must not again.
- **D2 Cruise 1 BL/s** (standing rule, pace in body lengths): browser 0.375 m/s,
  shredder 0.19 m/s; `forward_reference` follows; `motor_respiration_per_s`
  unchanged. Visible effect: animals walk 1.5× faster and a metre costs
  two-thirds of what it did.
- **D3 Occlusion.** Water occludes a ray only below its free surface (depth ×
  cell height, no new constant). A detritus pool occludes below its physical
  height = organic / (cell area × bulk density); the density is an authored
  placeholder in backlog §1.
- **D4 The edge of the world is a wall.** A ray or contact probe leaving the
  strip's z range reads solid, not open: a body cannot go there. A drop inside
  the world stays what it is; the −40° pitch is how a body sees one.
- **D5 Landscapes are whole frozen worlds, not crops.** A crop has to invent
  water, cue and walking boundaries (audit §8).
- **D6 Water in training is frozen**, two states per landscape seed captured
  during the pre-roll: drained, and mid-shower. Live water costs ~40 h per ES
  run. Rising-water survival is judged only in live evaluation.
- **D7 Who acts.** In a landscape episode every founder the seeder placed runs
  the candidate, births off; the score is their mean. Matches live, eight
  samples per episode, and the body channels finally see bodies.
- **D8 Horizon** 4,800 ticks (4 min) for landscape episodes.
- **D9 Score unchanged** — no edge-shaping term; blocked attempts already pay
  motor cost.
- **D10 Seeds.** Training landscapes 101–116, held-out 201–208, live gate on
  seed bases 1–8. Presets small, default, wide.
- **D11 Body size** sampled uniformly between birth size and adult at episode
  start; the arena gains a 0.125 m variant.
- **D12 One branch.** A and B run in parallel worktrees and are merged into
  `retrain` together with C; main never carries a centre the loader refuses.

## P5-A — contract v2 (fauna + manifest)

Worktree `retrain-a`, branch `retrain-a` off main.

1. **Row g first, as evidence.** Find how live bodies drown when motion
   refuses water deeper than the wade depth: water rising onto a standing body
   (showers), a destination read at a different face than the drowning check,
   or something else. If it is a bug in the step (the body moves where the
   rule says it cannot), fix it with a test; if it is the model working
   (rising water), say so and change nothing. Report before going on only if
   the answer needs a decision the brief does not give.
2. Pitches −40, −20, 0, 20, 40 (45 rays). Sector values stay fractions of the
   ray count, so the channel count does not move.
3. Occlusion per D3; world edge per D4, for rays and contact probes.
4. `Chem(litter)` → `Chem(detritus)` (name and canonical text only).
5. Cruise per D2.
6. `Self.birth_readiness` includes the surplus-hold, refractory and egg rules
   the birth step actually applies.
7. Cone occupancy: build it for a window around each observer (range + reach)
   or cache it per episode and invalidate on bites/deaths — your call — with a
   test that its readings equal the full rebuild's on a seeded world.
8. Manifest schema 2 per D1; new digests.
9. The heuristic teachers updated from observations only: back off / turn
   when `motor_delivery` is low or the downward rays read a drop or water.
10. The shipped centres will no longer validate. On this branch the host falls
    back to the heuristics with a loud line; do not touch the policy files.

Tests first, own commit: 45 rays at the five pitches; a ray over shallow
water passes above the surface and is stopped below it; a ray and a probe
past the z edge read solid; `birth_readiness` is 0 during the refractory and
while surplus is held; the windowed/cached occupancy equals the full build;
the manifest's canonical text changes when any anchor changes; row g's test
if it is a bug.

## P5-B — training worlds (voxel-sim + search)

Worktree `retrain-b`, branch `retrain-b` off main.

1. Move the founding loop (`ambient_habitat`, `found_a_habitat`, the
   pre-roll) and `habitat.rs` out of the `cubarium` binary crate into
   `cubarium-voxel-sim` (or a small new crate) so `cubarium-search` can call
   them; the host calls the moved code unchanged in behaviour.
2. `Prepared::{Arena, Landscape}` in search.
3. **Arena rebuilt**: crowns inside the mouth band (basal rosettes, seedlings
   ≤ 0.125 m, a stripped upper crown above the band), caps and carrion for the
   shredder, a 0.125 m variant, bystander bodies (D7 means other bodies are
   normal).
4. **`LandscapeSet{preset, seeds, water_state}`**: worlds founded by the moved
   loop at seeds per D10, frozen water per D6, all placed founders act per D7,
   bodies sampled per D11, horizon per D8; protocol `p5-landscape-1`.
5. Per-episode diagnostics: intake by food class, metres walked against unique
   area covered, blocked share of motor cost, time within one body length of a
   drop or edge, death cause.
6. Measure ticks/s per worker for arena and landscape episodes and the time to
   prepare the 16 × 3 × 2 training worlds.

Tests first, own commit: arena Stage B patches are reachable by the band from
the start face on every layout; a landscape episode is deterministic given its
seed; frozen water does not move during an episode; every founder in a
landscape episode runs the candidate; the moved founding loop gives the host
the same world it did (founded counts and acceptance verdict equal on
`default` seed 1 — counts, not hashes).

P5-B must not change sensing or the manifest (that is P5-A); if it needs to,
stop and report.

## P5-C — train and judge (after A and B merge into `retrain`)

Briefed separately when A and B land. Outline: record teacher streams on the
mixed arena + landscape set, fit the clone, ES (32 pairs, 512 updates, 23
workers); `policy=<lineage>=<file>` on `voxel_founder_autopsy` and
`voxel_census`; the gate below; copy the chosen centres in.

**The gate.** `voxel_founder_autopsy 60` over seed bases 1–8 × small / default
/ wide, main vs candidate: browser median alive at 60 above 0 on default and
wide and not lower on any preset; shredder median not lower; drownings not
up. Plus `voxel_census 6` on three bases. Held-out landscapes 201–208 against
the new heuristic, the clone and stationary. One-seed rows are anecdotes.

## Constraints (both packages)

Own worktree; `CARGO_TARGET_DIR` inside it; up to 23 cores. Explicit-path
commits only; end each with the `Co-Authored-By` line you were given. Fresh
worlds only; no pinned hashes; tests ≤ 200 ticks. No tuning: new numbers are
backlog §1 placeholders. Do not edit `design/handoffs/README.md`,
`design/README.md`, `config/tachyon/*`, `scripts/tachyon-*`, `docs/tachyon.md`,
`art/gen/*`, or the untracked `design/organism-anatomy-2026-09-21.md`. Do not
train (that is C). Full workspace suite (release) before reporting.

## Return (≤ 40 lines each)

Commits; choices made under your authority and why; row g's answer (A);
measured costs (B); anything contradicting this brief, with evidence.
