"""labd: one service that owns every sim, test and build job (stdlib only).

Design and the kind interface: design/lab-daemon.md. The CLI is scripts/lab/lab, the
daemon scripts/lab/labd. Kinds are plugins (labkit/kinds/*.py); the core knows nothing
sim-specific.
"""
