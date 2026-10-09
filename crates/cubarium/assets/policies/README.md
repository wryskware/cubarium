# Explicit founder policies

Every lineage runs its observation-only heuristic by default. No trained policy is
embedded in the binary. The retired C1 centres are available in Git history.

Use `--founder-policy LINEAGE=FILE` for an ambient run or `policy=LINEAGE=FILE`
in the census and related tools to install a saved policy explicitly. Files must
match this build's schema, lineage and founder manifest. Lineages without an
explicit policy keep their heuristic.

The legacy `--founder-heuristic LINEAGE|all` and example `heuristic` arguments
remain accepted as no-ops. Explicit policies take precedence.

Policy-loading tests generate fixtures from `tensor::initial_center_shape`;
no trained centre is retained as a test fixture.
