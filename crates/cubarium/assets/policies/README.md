# Founder policy centres shipped with the binary

The two trained centres the ambient run (`cubarium voxel`, no `--arena`) installs
on its founder lineages by default. They are embedded with `include_str!` by
`crates/cubarium/src/voxel/mod.rs`, so the binary carries them and a run needs no
files on disk.

| file | lineage | provenance |
| --- | --- | --- |
| `littershredder-curio-c1-2026-10-02.json` | `Founder::Blind` | curiosity lock step, desktop run C1, generation 14 (`config/training/curiosity-2026-10-02/C1-centre-littershredder.json`) |
| `frondgrazer-curio-c1-2026-10-02.json` | `Founder::Browser` | curiosity lock step, desktop run C1, generation 14 (`config/training/curiosity-2026-10-02/C1-centre-frondgrazer.json`) |

Both centres are the **co-training lock step with curiosity** (brief
`design/handoffs/brain-curiosity-2026-10-02.md`, reward `config/training/curiosity.toml`),
desktop run **C1**: calibrated curiosity weights, warm-started from the lock step's g15
mean and centres at σ0 0.10 with 16 brain pairs, on the build with the crag fix and wall
slide (6e69e7b2), stopped by its 10:00 wall after 14 generations. Each file's own
`generation` field reads 14; they are the trainer's centres, copied byte for byte.

They were **selected by the 2026-10-02 gate** (`design/cotraining-plan-2026-10-01.md`,
"Result, 2026-10-02"; seeds 9–16 × 2 h). The most balanced arm was X21: C2's ecology with
C1's brains, where every trained lineage moves above 0.38 of the time and has more than 50
alive (shredders 103 alive / 80 births, browsers 51 / 41, summed over the 8 worlds).
Wrysk, 2026-10-02: "yeah lets get the new defaults locked in now." So these are the live
brains, and C2's ecology (`config/training/curiosity-2026-10-02/C2-mean.toml`) is baked
into the live presets' `[flora]` and `[fauna]` tables as plain values. The code defaults,
which co-training worlds are built from, are unchanged.

The gate's open failures stay open under these defaults: springturf is lost in 1–2 of 8
worlds, and lanternjaws never establish.

The previous defaults, `littershredder-p3c-wander-gen441.json` (P3-C blind wander-seed)
and `frondgrazer-p3d-reach-gen390.json` (browser vertical-reach retrain), and the notes on
what they were trained with, are in git history before this change.

**A centre is replaced, never edited.** These files are the trainer's own output,
copied byte for byte; they validate themselves against this build (schema token,
lineage, weight count, finite weights, and the founder manifest digest), and an
edited weight is a policy nobody trained. To change the default, copy a different
centre in under a new name that says where it came from, point the `include_str!`
at it, and record the provenance here — do not patch a file in place.

The disclosed control for the default is `--founder-heuristic <lineage|all>`,
which puts that lineage back on its observation-only heuristic;
`--founder-policy <lineage>=<file>` runs some other saved centre instead.
