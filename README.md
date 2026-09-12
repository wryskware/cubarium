# Cubarium

A persistent artificial ecosystem living on the five visible surfaces of a
64×64 LED cube. Small organisms should acquire recognizable habits and shapes,
alter their surroundings, leave descendants, and sometimes disappear. The world
continues between glances.

**Current state (2026-09-12):** M1 is implemented under task authorization:
a Rust workspace with the continuous surface substrate (`cubarium-surface`, with
an independent 3D oracle and test suite), a linear-light renderer, and a host
that shows geometry fixtures in a desktop preview, sends them to the shim, or
captures PNGs (`cargo run -p cubarium -- demo --help`). The physical-cube
review (seam continuity, E8 reflection) is still pending Wrysk's observation.
M2, the first living world, is implemented in `cubarium-core` per
`design/m2-world-spec.md` with exact material and energy audits, checkpoint
resume, and telemetry (`cargo run --release -p cubarium -- run --help`); the E2
succession experiment (`scripts/e2-run.sh`, results in `design/7_Research/`)
found a regime with local depletion and recovery that survives a simulated
day, now the default tuning. What remains for M2 is viewing on the cube. Implementation does not make the architecture
canon; see the decision ledger.

A [Godot creature-art project](art/README.md) now provides editable sprite parts,
pivot rigs, rest/move/feed/bud animation timelines and habitat art. Its baked
sprites run on the shared cube geometry and output sinks in an explicit art
study: `./scripts/art-study.sh` serves the cube/net viewer on port 7394 at normal
speed. `./scripts/godot.sh --editor` opens the art project. `cubarium run --art
assets/atelier --sink web` draws the live world with the authored sprites, each
organism's clip driven by its real rest/seek/feed/gestation state and the habitat
motifs by producer biomass; without `--art` the image is unchanged. The rigs are
a cosmetic hue tercile, so nothing about the ecology or inheritance changed. See the [workflow and next slice](design/game-art-workflow.md).

Start with the [design overview](design/README.md), then the
[implementation plan](design/implementation-plan.md). The
[owner's brief](design/brief.md) records the requirements; architecture and
ecological mechanisms are preferred proposals, explicitly marked `leaning`.

The existing display shim at `~/vuzic/led-cube-shim` owns hardware output and
already supplies surface geometry helpers. Cubarium will build on that contract.
See the [local integration findings](design/7_Research/local-contracts.md).

Design authority follows [Lore's vault rules](design/0_Canon/README.md) and the
[decision ledger](design/0_Canon/DECISIONS.md). `.lore.toml` enables `lore-v1` in
`annotate` mode. A detailed proposal is not an accepted decision.
