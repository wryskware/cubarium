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

**A centre is replaced, never edited.** These files are the trainer's own output,
copied byte for byte; they validate themselves against this build (schema token,
lineage, weight count, finite weights, and the founder manifest digest), and an
edited weight is a policy nobody trained. To change the default, copy a different
centre in under a new name that says where it came from, point the `include_str!`
at it, and record the provenance here — do not patch a file in place.

The disclosed control for the default is `--founder-heuristic <lineage|all>`,
which puts that lineage back on its observation-only heuristic;
`--founder-policy <lineage>=<file>` runs some other saved centre instead.
