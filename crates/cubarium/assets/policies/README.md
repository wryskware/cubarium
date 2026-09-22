# Founder policy centres shipped with the binary

The two trained centres the ambient run (`cubarium voxel`, no `--arena`) installs
on its founder lineages by default. They are embedded with `include_str!` by
`crates/cubarium/src/voxel/mod.rs`, so the binary carries them and a run needs no
files on disk.

| file | lineage | provenance |
| --- | --- | --- |
| `littershredder-p3c-wander-gen441.json` | `Founder::Blind` | P3-C blind wander-seed, generation 441 (`runs/voxel-es-p3c-blind-wander/centers/gen441-center.json`) |
| `frondgrazer-p3d-reach-gen390.json` | `Founder::Browser` | browser vertical-reach retrain, Stage B landed, generation 390 (`runs/voxel-es-p3d-browser-b/centers/gen390-center.json`) |

The **blind** centre comes from the P3-C wander-seed run reported in
`design/handoffs/voxel-senses-phase2-briefs-2026-09-19.md`, integration note 6
(Fable, 2026-09-20, at 7f7111e): it is that run's selected best (score
.436/.471), whose Stage B target band is `near`, not `landed`. Nothing about the
blind lineage moved when the browser gained a vertical mouth reach — its
manifest digest and its litter arenas are unchanged — so this centre is still a
policy for this build. Wrysk chose it as the live default on 2026-09-20 after
founders driven by these centres survived 26 simulated minutes on the generated
closed world (40 alive) where the observation-only heuristics died (0 alive).

The **browser** centre is the retrain `design/handoffs/voxel-browser-reach-
2026-09-21.md` required. `mouth_reach_up_voxels = 1` moved the browser's
manifest digest and its stands now stand at three crown heights, so its task is
a different one (`p3a-half-initial-patch-1+p3d-crown-heights-1`) and every
browser centre trained before it is refused — the always-fresh rule, not a
regression. Seeded from a fresh imitation clone of the browser heuristic on that
arena and searched for 512 updates at 32 antithetic pairs over the 16 training
layouts, it closes the landed held-out eight at 8/8 acquired, 8/8 depleted, 8/8
successor bitten and 8/8 reacquired, score .840 mean / .840 median against
P3-C's .808/.839, and takes 724 of its bites off crowns one voxel above its
head, on all eight layouts.

## What these centres were trained with, and what the model now runs

**2026-09-22, package 1b** (`design/handoffs/voxel-body-anchors-2026-09-22.md`):
both centres still load — their manifests, and so their digests, are untouched,
because every number that moved lives on `FounderPhysiology` and not on the
`Manifest`. What moved under them is the **body**:

| | trained with | the model now runs |
| --- | --- | --- |
| eye | `standing_y + 1.5` cells — 0.0625 m over the surface on `small`, 0.125 m on `default`/`wide` | `0.8 x body height` in metres: 0.15 m for an adult browser, on every grid |
| mouth | the crown cell at the head layer and, for the browser, one whole voxel above it | the physical band `[0, 1.33 x height]` over the standing surface: 0.249375 m for an adult browser, which on a 0.25 m grid is the head layer alone |
| body | the manifest's 0.25 x 0.125 m browser, constant at every age | 0.375 x 0.1875 x 0.1875 m adult, scaled by `(body / body_max)^(1/3)` |
| clearance | `1 + mouth_reach_up_voxels` voxels | `ceil(height / voxel)` voxels |

The observation vector is the same 37 (browser) and 23 (shredder) inputs in the
same order with the same meanings, so the weights are applicable in the sense
the digest checks. They were not *selected* under these anchors, and the
behaviour they produce is expected to be visibly different — a browser trained
to lift its head into a crown one voxel up now stands under a crown it cannot
reach. **The retrain is package 5**, and until it lands these centres are the
disclosed default rather than a tuned one. The same caveat applies to the
contact receptor's change of meaning in package 1a
(`design/handoffs/voxel-founder-step-2026-09-22.md`).

**2026-09-22, package 3** (`design/handoffs/voxel-diets-2026-09-22.md`): both centres
still load. Nothing about the observation *shape* moved — same 23 and 37 inputs, same
module ids, slots, widths, channel order and taste-resistance mapping, so both digests
are the ones these files carry and a test pins them
(`crates/cubarium-voxel-fauna/tests/diets.rs`). What moved is what two of the
shredder's channels **mean**, and what both animals' mouths accept:

| channel | trained with | the model now runs |
| --- | --- | --- |
| `Chem(litter)` (shredder) | a diffused field whose source at a face is that face's **litter** stock | the same field, same emission curve, transport, decay and threshold, whose source is **litter + carrion + glowcap cap tissue** — the shredder's three foods (decisions §3) |
| `Taste(1)` (shredder) | the litter under the mouth, or bare ground | the richest of the three foods under the mouth, or bare ground; all three read the one soft resistance class the schema calls `litter` |
| `Taste(1)` (browser) | the foliage of any stand in the band, glowcap included | the foliage of any **vascular** stand in the band; a glowcap cap is fungal tissue and is no longer offered to a browser's mouth at all |

On a world holding only litter the shredder's cue is value-for-value the field it was,
so the change is invisible until there is a corpse or a cap. Where there is one, a
policy selected against a litter-only cue is being asked to read a cue that means more
than litter: **the retrain is package 5**, and until it lands these centres remain the
disclosed default rather than a tuned one, exactly as for the body anchors above.

**A centre is replaced, never edited.** These files are the trainer's own output,
copied byte for byte; they validate themselves against this build (schema token,
lineage, weight count, finite weights, and the founder manifest digest), and an
edited weight is a policy nobody trained. To change the default, copy a different
centre in under a new name that says where it came from, point the `include_str!`
at it, and record the provenance here — do not patch a file in place.

The disclosed control for the default is `--founder-heuristic <lineage|all>`,
which puts that lineage back on its observation-only heuristic;
`--founder-policy <lineage>=<file>` runs some other saved centre instead.
