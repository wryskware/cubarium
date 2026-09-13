---
design_status: exploration
last_reviewed: 2026-09-12
decision_refs: []
---

# Short matched optional-care audit

Root independently exercised the committed core (`a909aa0`) from the genuine
schema-7 live-world handover checkpoint `world-67984.cubw`, seed 1, 94 organisms.
Two copies advanced 12,000 ticks to 79,984: ten simulated minutes, no live state
writes, HTTP commands, or shim output. The host package was still in progress.

Reproduce with the root-owned numerical scenario:

```sh
cargo run --release -p cubarium --example care_compare --offline -- \
  captures/checkpoints/pre-care/world-67984.cubw --ticks 12000
```

The exact checkpoint is a local recovery artifact, not checked-in test data.
The example also accepts a different exact snapshot path, but the results below
only describe this input. Its SHA256 is
`278aa850c9df61328eb29e395b40cb4bbb910bcb610dc6f5f7dcc9af71924cf9`.

The care copy received one feed, one shower three seconds later, and one cleanup
15 seconds after each two-minute cycle began. Targets rotate between an ordinary
front-face point, a side seam, and the open lower rim. This is five of each action,
spaced beyond the proposed host cooldowns. Cleanup is intentionally partial.

| Measurement | No additional care | Care schedule |
| --- | ---: | ---: |
| Final population | 94 | 95 |
| Population min–max | 92–98 | 94–103 |
| Births / starvation deaths | 27 / 27 | 30 / 29 |
| Maximum material drift | 1.46e-11 | 1.63e-11 |
| Maximum energy drift | 4.73e-8 | 4.90e-8 |
| Maximum water drift | 7.51e-12 | 5.55e-12 |

Care receipts: five feeds applied, five showers applied and completed, five
cleanups partial. Actual cumulative imports: 15 material, 30 chemical energy,
20.00000000000004 summed water depth. Exports: 8.511576735980825 material and
14.770204125078632 chemical energy. Net food allowance used:
6.4884232640191755. No capacity rejection or surface travel fallback occurred.

The numerical audit compares against fixed opening inventories and the explicit
source/export differences on every tick; it does not treat `from_state`'s
re-derived baseline as proof that historical accounting was conserved. Care
progress, field and organism invariants are checked every tick. Event lists are
drained to keep the observer bounded. Hashes in output are decimal strings.

Final ecological hashes: baseline `3385187011931866109`, cared
`4054821303290430911`. A second identical run reproduced both and all 15
receipts. The CLI also rejects a duration outside its bounded 420–72,000 tick
range (419 tested, exit 2).

This passes a short numerical integration check, not a long-run ecological
acceptance gate. One checkpoint and ten minutes cannot establish predator
viability, unattended longevity, improved carrying capacity, or a pleasing
visible feeding response. No claim about journal recovery or browser controls
follows from this direct-core scenario; those require host tests separately.
