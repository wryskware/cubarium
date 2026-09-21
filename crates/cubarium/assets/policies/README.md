# Founder policy centres shipped with the binary

The two trained centres the ambient run (`cubarium voxel`, no `--arena`) installs
on its founder lineages by default. They are embedded with `include_str!` by
`crates/cubarium/src/voxel/mod.rs`, so the binary carries them and a run needs no
files on disk.

| file | lineage | provenance |
| --- | --- | --- |
| `littershredder-p3c-wander-gen441.json` | `Founder::Blind` | P3-C blind wander-seed, generation 441 (`runs/voxel-es-p3c-blind-wander/centers/gen441-center.json`) |
| `frondgrazer-p3c-wander-gen322.json` | `Founder::Browser` | P3-C browser wander-seed, generation 322 (`runs/voxel-es-p3c-browser-wander/centers/gen322-center.json`) |

Both come from the P3-C wander-seed runs reported in
`design/handoffs/voxel-senses-phase2-briefs-2026-09-19.md`, integration note 6
(Fable, 2026-09-20, at 7f7111e): the browser centre closes Stage B landed at 8/8
acquired, depleted, bitten and reacquired; the blind centre is that run's
selected best (score .436/.471), whose Stage B target band is `near`, not
`landed`. Wrysk chose them as the live default on 2026-09-20 after founders
driven by these centres survived 26 simulated minutes on the generated closed
world (40 alive) where the observation-only heuristics died (0 alive).

**A centre is replaced, never edited.** These files are the trainer's own output,
copied byte for byte; they validate themselves against this build (schema token,
lineage, weight count, finite weights, and the founder manifest digest), and an
edited weight is a policy nobody trained. To change the default, copy a different
centre in under a new name that says where it came from, point the `include_str!`
at it, and record the provenance here — do not patch a file in place.

The disclosed control for the default is `--founder-heuristic <lineage|all>`,
which puts that lineage back on its observation-only heuristic;
`--founder-policy <lineage>=<file>` runs some other saved centre instead.
