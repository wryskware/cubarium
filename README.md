# Cubarium

A persistent artificial ecosystem living on the five visible surfaces of a
64×64 LED cube. Small organisms should acquire recognizable habits and shapes,
alter their surroundings, leave descendants, and sometimes disappear. The world
continues between glances.

**Current state:** repository and design vault initialized; no simulation or
renderer has been implemented. The current task is planning, not a runnable demo.

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
