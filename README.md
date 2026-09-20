# Cubarium

A persistent artificial ecosystem for a 64×64 LED cube and, first, a local
desktop display. One shared 3D voxel world with terrain, free water and a small
plant and animal community whose changes have physical causes.

## Current direction (voxel, 2026-09-20)

The project is being rebuilt around a shallow **voxel ringworld**, developed and
tested locally first and adapted to the cube and the Tachyon panel later. The
[terrain and ecosystem proposal](design/terrain-and-ecosystem-proposal-2026-09-16.md)
is the lead planning document. The
[ecological-niches reconsideration](design/ecological-niches-reconsideration-2026-09-15.md),
the [theoretical biosphere](design/theoretical-biosphere-2026-09-16.md) and the
[voxel ecological sketch](design/voxel-ecology-sketch-2026-09-16.md) sketch the
community. The [game roadmap](design/game-roadmap-2026-09-16.md) is a
lower-priority product exploration.

Priority as of this date: the ambient display comes first, the
[senses design](design/handoffs/voxel-senses-handoff-2026-09-18.md) runs alongside
it, and game mode waits. `design_status` still marks all of this `leaning` or
`exploration`; nothing here promotes a proposal to a decision.

Voxel sensing phases one and two are complete on the current branch. The
bounded pipeline now has honest Stage-A starts, two-patch Stage-B arenas,
recurrent-policy ablations, deterministic baselines and a fed evolutionary
search. Stage A meets its target for both founders; Stage B does so for the
browser but not the blind founder. The first phase-three diagnostic, through
commit `ee607a2`, halves the initial Stage-B resource, balances the held-out
signed offsets and adds a fixed blind offset sweep. The smaller patch made the
browser deplete it in 7/8 held-out trials, but neither founder reacquired the
successor patch in any held-out trial. This closes the timing hypothesis: the
next bounded sensing package needs to train the post-depletion transition, not
merely shorten the first feeding leg. This remains a narrow sensing result; it
does not establish broader ecology or production readiness.

Voxel terrain, water and the first plants and animals live in the
`crates/cubarium-voxel*` crates. The round briefs under `design/handoffs/` and
the dated notes under `design/7_Research/` record what each round established.
Start with [design/README.md](design/README.md) for the current map,
[design/handoffs/README.md](design/handoffs/README.md) for open work, and
[design/backlog.md](design/backlog.md) for deferred features.

## Run it

Local GPU window (seeded example habitat by default; `--empty` for the bare
world):

```sh
./scripts/run-voxel.sh
```

Loopback viewer without a GPU window:

```sh
./scripts/run-voxel.sh --sink web --web-port 7393
```

The older cube display: `./scripts/run-cube.sh` builds this checkout and drives
the physical cube plus the shared viewer on port 7393. It is a development
launcher, not a frozen release. Old captures and duplicate build trees are
disposable; source history belongs in Git.

## The Tachyon panel

The world also runs as a shelf piece on the Tachyon's 1080×1920 panel.
[docs/tachyon.md](docs/tachyon.md) is the whole story: install, the privilege
split between `cube-screen-shim` and `cubarium`, resume rules, the
`CUBARIUM_EXTRA_ARGS` look knob, and the `ssh -L` tunnel for the loopback
viewer. The panel is a live rendering target; the scripts are
`scripts/tachyon-deploy.sh`, `scripts/tachyon-install.sh` and
`scripts/tachyon-status.sh`, with the unit and config under `config/tachyon/`.

## Design authority

[design/0_Canon/README.md](design/0_Canon/README.md) and the
[decision ledger](design/0_Canon/DECISIONS.md) bind; a detailed proposal is not
an accepted decision. Dated material under `design/7_Research/` is evidence of
its date and never a current task list. Working preferences are recorded in
[WORKING_POLICY.md](WORKING_POLICY.md), and agent instructions in
[AGENTS.md](AGENTS.md). The art direction is
[design/art-direction/Cubarium_Art_Direction_v0.1.md](design/art-direction/Cubarium_Art_Direction_v0.1.md).
