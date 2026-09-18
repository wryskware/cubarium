---
status: open
date: 2026-09-18
owner: Wrysk (design), in his own interactive thread
---

# What a creature can sense: handoff for Wrysk's senses thread

Wrysk (2026-09-18): "creatures should have specific sense appendages that
correspond and enable different senses, and these are likely tied to their
genome / lineage. what can actually be sensed is going to hugely shape how the
game plays as well as how the creatures behave, so it's one of the more
keystone decisions we'll be making." He reviews this plan himself and comes
back with a finalized spec. Nothing is built until then.

This file is the prompt for that thread. Paste the "Prompt" section into a
fresh Fable session in this repo, or point the session here.

## The problem

The voxel world's first animal, the frondgrazer (`crates/cubarium-voxel-fauna`,
round 5c), senses by reading the world directly: every stand with foliage
within `sense_radius` support faces, scored by the model's own reach query
from every candidate face. That is omniscient sensing and it is a stand-in.
The intended design is: an animal perceives through **sense appendages** its
lineage carries, the perception is a bounded egocentric **observation
vector**, a **controller** (a heuristic today, a recurrent policy later) maps
observations and internal state to **actions**, and the controller never sees
the world's ledger, the stand list or any global truth. Brains are
interchangeable behind that interface; senses are part of the body and
therefore of the genome.

The decision is what can be sensed, at what cost, through what appendages,
and how that is inherited. It shapes behaviour, training, the game's
science-tree branch "animal biology: sensing", and what the viewer can read
off a creature's silhouette.

## What exists (read in this order)

- `design/recurrent-interface-contract.md` — the flat world's v1 contract:
  a 70-scalar observation vector (layout §1.1, rationale §1.2), a spatial
  sampler over fields (§2), 7 action channels (§3), recurrence state and
  cadence (§5), inheritance (§8). This is the closest prior art and the
  shape a voxel contract should rhyme with, not copy.
- `crates/cubarium-core/src/neural/{obs.rs,action.rs,gru.rs,state.rs}` — the
  runtime that implements it (schema 15), and `crates/cubarium-search` (the
  ES trainer). Reusable once the voxel sampler exists.
- `design/evolution.md` and `design/fauna-v2.md` — the phenotype/genome model
  the flat world uses; body parts and inherited traits live here.
- `design/theoretical-biosphere-2026-09-16.md` §4 "Seven animal roles" and §6
  "Geometric feeding access and refuges" (standable supports, bounded travel,
  mouth reach, body-sized passages, line of sight) — the biosphere's own
  requests about what animals need to perceive.
- `crates/cubarium-voxel-fauna/src/{lib.rs,step.rs}` — the current animal:
  record, species config, the six-step tick (terrain, maintenance, sense, act,
  births, death), the heuristic's sensing at `sense` and its greedy walk.
  `Reach { horizontal, up }` and `reachable_foliage` are the current "mouth".
- `crates/cubarium-voxel/src/world.rs` and `lib.rs` — what the world can
  answer cheaply: material per voxel, support faces, water depth, free and
  pore water, sky visibility per site (17-ray fan, cached per terrain
  version), wrapped `x`. There is **no line of sight** query and no
  occlusion model yet; the biosphere asks for one.
- `crates/cubarium-voxel-flora/src/lib.rs` — what plants expose (foliage,
  wood, crown geometry via `crown_voxels`, species, seed banks, litter, dead
  wood, carrion).
- `design/art-direction/Cubarium_Art_Direction_v0.1.md` — pillars 2 and 3:
  alien but evolutionarily suggestive; "a strange appendage should invite
  'what does it use that for?'". Appendages are the visible half of this
  decision.
- Memory context: pace in body lengths (~1 BL/s cruise); motor cost as a
  physics heuristic (disc bodies, energy-equivalent rotation); the flat
  world's ecology v1 sensing was 70 scalars sampled from fields.
- `design/game-roadmap-2026-09-16.md` §4 "Animal biology: digestive
  capabilities, locomotion, sensing" — parked, but the sensing branch is the
  same decision seen from the game side.

## Questions the spec should settle

- **Catalogue.** Which senses exist at all: light/shape (foliage masses,
  terrain, other bodies), chemical (food scent, trails, carrion, wetness),
  mechanical (water depth, slope, contact, vibration from other animals),
  internal (hunger, reserve, body, age). Which are v1.
- **Appendages.** Which appendage enables which sense, with what geometry:
  field of view, range, resolution (how many cones or samples), and whether
  it senses through occluders. Whether an appendage can carry two senses.
  How an appendage reads in the silhouette.
- **Cost.** What a sense costs the body: build cost, maintenance, and whether
  sensing itself costs energy per tick (the paid-flows rule applies to
  perception too, or it explicitly does not, stated either way).
- **Genome and lineage.** How appendages are inherited and vary: a fixed set
  per species, or genes for presence/size/range that mutate; what the
  science tree's "sensing" unlock would change.
- **Observation vector.** The exact scalars, their frames (egocentric,
  heading-relative), normalisation, and how a missing appendage shows up
  (zeros, or a smaller vector per lineage). Recurrence cadence: sense every
  tick or on a slower cadence.
- **Actions.** Move (one face per step today; continuous heading later),
  crop, rest, and whatever a sense implies (turn to look). Whether actions
  are the same for every lineage.
- **World queries.** What new queries the core must offer: line of sight,
  cone sampling of foliage/material, scent fields (a diffusing field is a new
  simulated quantity with its own cost), sound. Each is a rule addition with a
  performance cost; the sim is being made fast now and sensing must stay
  proportional to animals × samples, never to world size.
- **Diagnostic controller.** The heuristic rewritten to act only on the
  observation vector, so a failed population can be split into "bad policy"
  and "impossible budget" (biosphere §5).
- **What the player sees.** Whether a sense is legible in the ambient view
  (an animal turning its stalks toward food) and what the field guide would
  say.

## Constraints

Local and egocentric; paid where the body pays for everything else; no
global truth; proportional cost; brains interchangeable behind one
`Observation → Action` interface; determinism is **not** required (Wrysk:
measurements are statistical, never bit-exact); always fresh (a new sensor
schema refuses old worlds). The voxel sim is mid-performance-pass (local
water model, bevy schedule); the sensor sampler will be one of its systems.

## Deliverable

`design/voxel-senses.md`, status decided when Wrysk says so: the sense
catalogue, the appendage model and its genome linkage, the observation vector
layout, the action set, the world queries required (each with its expected
cost), the diagnostic controller contract, and the tests that would show a
sense works (short, statistical). Plus a one-page "what changes in the game"
note if it differs from the ambient reading. Fable then briefs the build from
that document; workers do not interpret it.

## Prompt (paste into a fresh Fable session)

> This is a design session with Wrysk on what creatures in the cubarium voxel
> world can sense. Read `design/handoffs/voxel-senses-handoff-2026-09-18.md`
> first, then the files it lists in order. Do not decide for him: lay out the
> options per question with their consequences for behaviour, training cost,
> simulation cost and the game, propose defaults, and iterate with him to
> `design/voxel-senses.md`. Brains must be interchangeable behind one
> observation/action interface; senses belong to appendages that belong to
> the genome; nothing global leaks to a controller. Do not edit crates; do not
> touch `design/handoffs/README.md`; the performance worker is active on the
> `voxel-perf` worktree. Explicit-path commits of the design file only.
