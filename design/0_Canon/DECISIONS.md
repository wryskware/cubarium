---
design_status: decided
last_reviewed: 2026-09-11
decision_refs:
  - D-0001
  - D-0002
  - D-0003
---

# Cubarium decision ledger

Append-only. Newest entries at the bottom. Schema per [README](README.md).
Only decisions already authorized in the owner's request are accepted below.

## D-0001 — Adopt the lore-v1 design vault

- **Date:** 2026-09-11
- **Status:** Accepted
- **Scope:** Design documentation and planning in this repository
- **Decided by:** Wrysk, explicitly requesting repository initialization and plans in Lore's design vault format on 2026-09-11
- **Decision:** Adopt `lore-v1`, an append-only decision ledger, and tentative-by-default design documents. Start authority behavior at `annotate` as the setup default, with `rank` available later.
- **Rationale:** Separate the owner's requirements and accepted choices from the agent's detailed, revisable proposals.
- **Consequences:** Consult this ledger before treating plans as binding. Promotion of additional choices requires specific authorization. Repository-wide agent instructions are proposed separately for review.
- **Supersedes:** None
- **Canonical sources:** [Canon rules](README.md)

## D-0002 — Establish the persistent cube ecosystem brief and current planning scope

- **Date:** 2026-09-11
- **Status:** Accepted
- **Scope:** Product intent, physical display constraints, engineering priorities, and the deliverable requested now
- **Decided by:** Wrysk, the ecosystem brief and final scope instruction supplied on 2026-09-11
- **Decision:** Plan a persistent ambient ecosystem on five connected 64×64 cube faces; prioritize continuous topology, ecological and evolutionary variety, expressive tiny organisms, long-running stability, and environmental input hooks. Reuse the existing display shim. For now, initialize the repository and produce plans; implementation follows later.
- **Rationale:** The installation should remain a living ambient world over long periods, with meaningful changes between glances, without depending on user attention or a scalar fitness objective.
- **Consequences:** Default presentation is the world without analytical overlays. The briefs may select and explain preferred mechanisms, but specific algorithms, bottom-edge behavior, runtime choices, numerical budgets, and milestone gates remain proposals.
- **Supersedes:** None
- **Canonical sources:** [Owner's brief](../brief.md)

## D-0003 — Install the approved repository agent instructions

- **Date:** 2026-09-11
- **Status:** Accepted
- **Scope:** Repository agent working rules and their installation
- **Decided by:** Wrysk, replying "go ahead" to the explicit request to install the reviewed rules as `AGENTS.md`
- **Decision:** Install the exact reviewed working rules in root `AGENTS.md`, making the vault's authority conventions and the existing display-shim boundary available to agents working in this repository.
- **Rationale:** Keep design provenance and project constraints available during ordinary work without requiring the vault-setup skill to be invoked again.
- **Consequences:** The approved rules are installed. This approval does not promote the proposed simulation architecture or authorize starting implementation.
- **Supersedes:** None
- **Canonical sources:** [Repository working rules](../../AGENTS.md)
