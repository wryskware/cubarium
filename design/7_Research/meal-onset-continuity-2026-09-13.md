---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Actual-intake meal continuity

Fable's presentation-only candidate groups actual intake ticks into one continuous
authored feeding loop. It addresses brief move/feed alternation during nibbling
and joins a new meal at its own clip origin, instead of at an arbitrary shared
phase. Natural meals and manually encouraged meals use the same local signal.
There is no global wake-up, new ecological state, resource input or artificial
satiation. This is not implementation of ordinary quiet physiology.

## Contract and boundary review

`meal_present.rs` tracks one small record per living full organism ID. A0.25s
intake window bridges short nibble gaps; a0.3s cross-fade introduces/releases the
bout. Returning intake during release retains the origin instead of snapping.
Path, heading, turn, size and actual physiology remain untouched. `fed` means
any actual field intake, not necessarily a manual crumb or a measured meal size.
First-seen animals get no invented onset; repeated observations/draws do not
advance memory. Rewind starts fresh. Dead/reused IDs lose their history.

Root completed Fable's interrupted prototype after native Claude returned a
confirmed429 session limit. The review corrected an established-meal→gestation
phase cut: gestation now immediately releases the outgoing meal without a settle
hold, blending into the real bud. It also removes ordinary meal memory immediately
when the hunter adapter claims an ID, including membership supplied after the
ordinary observation at the same boundary. Hunter biology/art are unchanged.

Thirteen targeted real-stamp tests pass, including same-instant30/60/120fps,
no-intake behavior, continuous intake/gap/return, generation reuse, bounded memory,
repeated observations, rewind, transported seams and exact continuity at the
meal→gestation boundary. This proves specified rendering mechanics, not subjective
legibility or ecological improvement. Broad host verification is tracked separately.

## Capture protocol and current status

`meal_capture` steps one copied world and draws both presenters on precisely that
trajectory. Prescribed cases remain seed1/target0/Feed, seed8/target0/Feed (the
retained weaker response), and seed1/no input. Native256×128 nets contain five
64px faces; three frames per20Hz tick permit1×60fps playback. Frame ranges, input
schema/build, care receipt, opening/closing state hashes and a closing snapshot
are recorded. Existing output directories are refused. Captures are observational,
not an independent ecological comparison between two simulated arms.

Fable's earlier captures at `captures/meal-onset-2026-09-13` are retained as
preliminary evidence only: they precede its final nibble-hold revision and root's
boundary fixes. Do not cite their images as the final candidate. Root inspected
the seed1 native-enlarged sheet; final matched captures and continuous playback
must be reviewed before deployment. No cube update is claimed by this commit.

Remaining: complete broad verification, freeze/rebuild candidate, recapture all
three prescribed cases, compare continuous motion at1× and retain weak/ambiguous
results. Ship only if that review supports a readable improvement. Rain/cleanup
biological responses, genuine quiet habits and unattended diversity remain
separate open work; see the [care results](astra-care-response-results-2026-09-13.md)
and [developmental follow-up](fauna-development-followup-2026-09-13.md).
