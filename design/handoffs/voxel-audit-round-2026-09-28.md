---
status: open
date: 2026-09-28
owner: Fable (organism line); Wrysk said go 2026-09-28
follows: design/7_Research/ecology-parameter-audit-2026-09-28.md (§8); voxel-browser-movement-2026-09-28.md
---

# Audit round: the world we were tuning is mis-scaled

Wrysk's parameter audit shows that parking is the right strategy in this world. One plant
regrows enough to feed 5–8 parked browsers forever, because plants and animals are counted
in org-units roughly 1,000× apart. The es4 search is stopped and its held-out check
skipped. Commit each step when it is tested, and report its SHA in one line; merges land
as they come.

## Steps

1. **Motor cost tied to upkeep (§8.1).**
   - Walking cost = `motor_factor` × `maintenance_per_s`, per lineage, default 1 (the
     shipped ratio), so an upkeep multiplier no longer silently changes the price of
     walking.
   - `motor_factor` is a search parameter, log range [0.5, 4].
   - Retire `config/training/ecology-tuned-2026-09-28.toml`: mark it superseded in its
     header.
2. **Mass-scale test (§1.4, §8.2).**
   - A per-lineage `mass_scale` multiplies every absolute animal quantity the audit lists:
     `body_max`, `body_min`, `birth_body`, `birth_cost`, `surplus_floor`, `egg_organic`,
     `bite_per_s`, `bite_half_stock` and `growth_max_per_s`. Per-unit rates and metres are
     left alone.
   - Heuristic walker vs round 2's parker at ×1, ×30, ×100 and ×300, for browsers; the same
     for shredders against litter.
   - New-defaults world, small, still water, seeds 1–8 × 60 min, plus one 6 h census at the
     winner.
   - Report walked m/h, moving share, intake per body-hour, births, starved, and the
     population at 6 h.
   - Compute R, S and F at each scale, to confirm the audit's arithmetic.
   - Say how the brains' senses (taste saturation and so on) read at the new mass.
   - `mass_scale` becomes a search parameter, log range [1, 1000].
3. **Breeding eligibility (§8.5; Wrysk decided).** An animal may breed after being in
   surplus for at least 80 % of the last 900 s, instead of all of it unbroken, so a
   foraging trip is not a reset. This lands before the mass test's final census.
4. **Defence readable, on the biological clock (§8.4).**
   - The mouth's stand choice reads (1 − defence) × stock.
   - Switch-on within a few minutes of grazing; relaxation over tens of minutes.
   - Taste: change it only if the contract allows it without a new input. Otherwise say so
     and leave it.
5. **Training worlds are the live world (§8.6).** The frozen arenas and landscapes get the
   graze refuge, the defence, the night and the ecology vector.

## Wrysk's decisions, recorded

- **The reward (§8.3):** keep the score as it is, with no new terms. The world, through
  mass scale, must make parking unable to sate.
- **One clock, K_bio (§8.7):** yes, but after the mass test, and agreed with the weather
  thread's night work. Not started here; it gets its own brief.
- **The registry (§8.8):** held until predation P1 is on main.

## Constraints

- CCD1 under `nice -n 10`.
- Tests first, each ≤ 200 ticks.
- Explicit-path commits with the two trailer lines. Merge main in often.
- Block in the foreground, and kill your own wait loops before reporting.

## Return (≤ 40 lines)

End with a recommendation for the next brain training round.
