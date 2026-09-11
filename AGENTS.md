# Cubarium working rules

Cubarium is a persistent ambient ecosystem for five connected 64×64 cube faces.
Current repository status and next work are in README.md and
design/implementation-plan.md. Plans do not imply an implementation exists.

Before using or editing design material, read design/0_Canon/README.md and the
relevant entries in design/0_Canon/DECISIONS.md.

- Written, polished, detailed, or implemented does not mean canonical. Only
  active accepted ledger entries and their cited sources bind within their scope.
- Treat documents without design_status as unclassified exploration.
- Preserve modality: do not turn a leaning, proposal, example, or current
  implementation into a requirement.
- Do not append accepted decisions, mark documents decided, or supersede canon
  without Wrysk's explicit authorization for that specific choice. Continue
  routine authorized design and implementation work without approval rituals.
- Material under design/7_Research/ is evidence, not decisions.
- Use Lore retrieval first for codebase and design-history questions when
  available; verify the referenced source and fall back to file tools when needed.
- The display shim at ~/vuzic/led-cube-shim owns output transport and hardware
  mapping. Consult its current contract and reuse its geometry helpers.
- Keep the normal display free of analytical UI. Development diagnostics belong
  in explicit tools, captures, or logs.
