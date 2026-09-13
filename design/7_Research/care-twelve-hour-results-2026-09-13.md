---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Twelve-hour care comparisons: accounting and ecological evidence

Fresh default seeds1,2 and3 each completed matched untreated/repeated-care
comparisons over864000 ticks (12h). These are not copies of the live world.
Care cycles every2400 ticks: feed, then rain at+60, clean at+300, rotating targets
between ordinary face, seam and rim. All rejected requests remain in the record;
the net material allowance is never bypassed. Opening population24 in every arm.

| Seed / arm | Final population | Maximum | Surviving founder lineages | Deepest generation | Final grazer/glider/burrower/skimmer forms |
| --- | --- | --- | --- | --- | --- |
| 1 untreated | 129 | 164 | 2 | 34 | 113/0/16/0 |
| 1 repeated care | 114 | 175 | 2 | 46 | 0/84/30/0 |
| 2 untreated | 120 | 171 | 3 | 49 | 6/93/21/0 |
| 2 repeated care | 129 | 166 | 4 | 39 | 22/83/24/0 |
| 3 untreated | 125 | 173 | 3 | 41 | 100/0/25/0 |
| 3 repeated care | 119 | 164 | 4 | 38 | 64/27/28/0 |
| 1 occasional care (12000-tick cycle) | 93 | 146 | 5 | 38 | 64/7/22/0 |

No arm reached whole-world extinction; all had a minimum population24 including
startup. This does not establish a useful mature-population lower bound. Forms
are counted separately from actual founder ancestry. All three seeds lose skimmers;
seed1 retains different dominant forms with care. Survival through12h therefore
does not establish preservation of the intended diversity, longer-term stability,
or sufficient prey recovery for predators. No reseeding was used.

Seed1 accepted143 feeds and rejected217; seed2 accepted135 and rejected225. Each
accepted360 showers and360 cleanups (357 partial). Seed1 imported429 material and
exported400.9585736587185; seed2 imported405 and exported378.9079098043322.
Final net allowance used was28.041426341281724 /26.09209019566761. Large gross
turnover is therefore possible within a bounded net allowance; do not confuse
the30m bound with a lifetime gross-input limit or unlimited feeding permission.

## Independent accounting diagnosis

The original strict opening-inventory gate is unchanged. Seed1 cared persisted
energy residual peaks at2.0506936266428966e-5, above its1.9563930834586353e-5
limit. Two repeated runs with observer resets every200 and20 ticks reproduce the
EXACT same full final state hashes: untreated `2601251707681483130`, cared
`5399110610278733365`. Their compensated short-window cared residual maxima are
6.281197784119286e-10 and2.8410340746631846e-10. The largest immediate care-boundary
energy residual is4.547473508864641e-13.

At the same closing tick, persisted heat exceeds the windowed sum by about
2.0344e-5; the signed light and care-counter differences explain the remaining
residual to sub-nanounit readout precision. This strongly identifies accumulated
rounding in the long-lived heat counter for this case, not a care transfer leak.
It does not retroactively pass the persisted-ledger gate: both diagnostic commands
still exit1 and preserve their full reports. Correcting persisted accounting
requires explicit snapshot-compatible compensation and independent review; no
simulation accounting/default change was made here.

Seed1 material drift stays below9.36e-10 and water below2.15e-9. Seed2's original
repeated comparison passed its gates (untreated energy1.802956103347242e-5;
cared9.666980304245953e-6), but it lacks the later independent-window diagnostic.
Seed1 occasional and seed3 repeated have now completed under the report-preserving
old-core diagnostic. Seed3's baseline AND cared raw energy gates fail (maxima
1.9809653e-5 /2.1090723e-5, limit1.9735379e-5); seed1 occasional cared fails
(2.2022608e-5, limit1.9563931e-5). Their independent windowed errors stay below
1.22e-9, and immediate care boundaries below4.55e-13. This extends the heat-counter
rounding evidence to untreated ecology, without passing any failed raw gate.
The seed1 occasional baseline exactly matches the earlier seed1 baseline hash.
See [Astra's four-arm audit and outcomes](astra-long-run-energy-diagnostic-2026-09-13.md).
Seed2 occasional still lacks a complete report. Three seeds and combined care
inputs do not establish general diversity protection or predator balance.

Core compensation is now implemented in `b47eacc` with explicit schema9 migration;
root's harness integration is `57408c4`. Corrected twelve-hour repeats have started
but are not results yet. Preserve the old diagnostic executables/reports above.

Artifacts: ignored `captures/care-longitudinal-2026-09-13/`, especially
`seed1-repeated-window200.json`, `seed1-repeated-window20.json`, and
`seed2-repeated.json`. Diagnostic executable SHA256
`1c13f02d8f355ede833ddedbe7e452dd7de54dea8df9a602ab338aafdffd891d`.
Reproduce with `care_compare-windowed --seed 1 --ticks 864000 --care-every 2400
--audit-window 200` (then20). The observer is checked in as
`crates/cubarium/examples/care_compare.rs`; four unit tests include cadence hash
equality, schedules, source/limit validation and generation-safe ancestry.
