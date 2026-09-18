---
status: resolved
date: 2026-09-17
owner: Fable
---
> **RESOLVED 2026-09-17.** Landed on main; see git log for the commits.

# Voxel replacement-control study: the harness, not the run

Astra designed this study in R5.4 and corrected it in R7.2
(`design/7_Research/astra-voxel-first-wave-review-2026-09-16.md`). It is the
first thing that could count as coexistence evidence for two producers, and it
is explicitly **not** a preset gate or a clearance condition. This brief builds
the **study harness** and runs one short smoke of it; the long controls and the
probe are a separate, explicitly authorised run that Wrysk starts.

## What the harness must do (R5.4 + R7.2, condensed)

- **Conditioned state.** Condition hydrology and the resident population first
  (interval storage, habitat and head all reported, not only head). Provision a
  **fixed per-site mineral inventory at world creation** for the whole study
  (`FloraConfig::initial_mineral` applied eagerly to every support site, booked
  as `seeded_mineral_in` once), so fertility is matched across arms; lazy
  colonisation imports are not matched fertility.
- **Predeclared sites.** Three suitable, unoccupied introduction sites across
  the eligible band, chosen once from the conditioned state by the model's own
  `Gates` and the placement helpers package L repaired (`canopy_over`,
  `OpenSoil`), each with its eligible dispersal recipients counted and printed.
  The site list is declared once and reused by every arm; it is never
  re-derived per arm (R7.2, package L's note).
- **Arms per site and direction.** From the one conditioned state: resident
  only; resident plus newcomer; newcomer with the resident **excluded**
  (resident stands and seed banks removed with `Clear`-style bookings, water,
  litter and soil mineral retained). One founder of the tested species at wood
  0.3, ordinary `Seed` foliage and reserve, zero parcel, no bank. Geometry,
  forcing, phase, species values and draw keys fixed within each pair.
- **Refusals, not silent arms.** An arm whose resident was never planted or has
  disappeared is printed as a refusal and never labelled an invasion (R7.2). A
  velvetpad resident is placed on declared damp terrain shade or under a
  separately declared background canopy held constant across arms; it must
  not secretly require the competitor being excluded. Add the empty-layer
  velvetpad-resident selection check.
- **Observation.** Per tick, newly seen newcomer IDs (births), losses and
  survivors separately; donor funding and delivery events by identity so a
  founder's repeated donations cannot pass as descendant reproduction; first
  birth separately from descendants reaching donor size and funding packages
  themselves.
- **Timing.** A declared observation cap per species pair, predeclared from the
  presets' own arithmetic and printed before the run: the time from newborn to
  donor at the capped wood rate plus one fully funded package. For
  bloomcrown/umbrellafrond that is ≈ 2,708 s + 300 s; for stonecushion ≈ 8,047 s
  + 600 s, so a 6,000 s cap cannot resolve it and the harness must say
  "unresolved at cap" rather than infer exclusion. Once controls establish
  the replacement time `G` in both directions, the probe window is at least
  3 × the larger `G`.
- **Verdict wording.** Positive increase while rare through replacement, beyond
  the founder reserve subsidy, in both directions while the resident persists,
  is coexistence evidence for these conditions. One birth, founder survival, or
  a declining resident is not. One successful site is a possible refuge.

## Deliverables

- `replacement <resident> <newcomer> <seconds> [seed] [noise]` in
  `crates/cubarium-voxel-flora/examples/two_producers.rs` (or a new example
  `replacement.rs` sharing the placement helpers), printing the predeclared
  cap, the three sites with recipient counts, each arm's per-100 s table and
  the refusal/unresolved wording above. `--cap` may shorten it for smoke runs.
- Eager per-site mineral provisioning as a `FloraConfig` option
  (`provision: Lazy | AtCreation`, default lazy so nothing else changes), with
  a test that `expected_mineral` is constant across arms under `AtCreation`.
- Tests (short): the exclusion arm books the resident's stands and banks out
  and keeps water/litter/mineral; a resident that dies before the newcomer's
  introduction yields a printed refusal, not an arm; the cap arithmetic
  function reproduces 2,708.15 s and 8,047 s from the presets; the site list is
  identical across arms for one conditioned state.
- One smoke: bloomcrown resident / umbrellafrond newcomer, `--cap 300`, in
  `--release`; report the tables and wall time in the experiment note under
  "Replacement harness smoke — 2026-09-17". **No 6,000 s run.** The controls
  and probe are a run Wrysk authorises separately.

## Package

**P — replacement harness**, Opus high, worktree from the commit after
packages M and N land (it touches `two_producers.rs` and `lib.rs`), files
`crates/cubarium-voxel-flora/**`, backlog, the experiment note. Rules as the
other voxel briefs: explicit-path commits, no fmt, no tuning, short tests, no
long runs, never HashMap iteration.
