---
date: 2026-10-03
author: Fable
status: proposal (for Astra's review, then a study package)
---

# Plant succession: velvetpad first, then springturf, then the woody plants

Wrysk, 2026-10-03, after the C1 fauna check: "i think the replacement should be springturf
in the short term. like mosses colonize first, then grasses take over, then other pants."

This replaces the C1 direction of `design/velvetpad-niche-r2-2026-10-03.md` as the
preferred one. Astra's r2 review (`design/7_Research/astra-velvetpad-niche-r2-review-2026-10-03.md`)
recommended C1 plus competitor changes, with velvetpad-targeted overtopping (its (b)) only if
replacement needed to be faster. This proposal makes (b), aimed at springturf, the main
mechanism, and drops C1.

## What the evidence says

- **C1 starves the grazers and litter feeders.** Paired fauna measure (main 8746fabe,
  founder_tally, seeds 1/3/5/6/8/9/12/14 × 2 h, today's plants → C1, medians):
  - litter shredder B: 0.94 → 0.48;
  - frondgrazer B: 1.74 → 0.70;
  - loftstrider B: 5.5 → 1.5;
  - snail, bellwing, seedporter and capgnawer: unchanged;
  - foliage bites 935k → 475k and litter bites 280k → 136k (sums over the 8 worlds);
  - no starvation within 2 h.

  The cause is that drained ground stays bare: competitors take freed ground over more than
  6 h (r2 section 6).
- **Established mats hold today.** Only woody seeds (bloomcrown, lanternberry, vaulttree at
  0.1 a check) come up through a ground cover. Springturf, stonecushion and umbrellafrond
  have overtop 0. Runners need vacant sites. So 2–5 % of drained velvetpad sites change
  hands in 2 h (r2 section 4).
- **Springturf is the right successor on lit ground.** It is a grazeable mat (crown
  0.125–0.19 m, graze refuge 0.3, foliage rate 0.02 against velvetpad's preset .0061), and
  sheds more (senescence .00156 against .000581). Its establishment gate is sky ≥ 0.75,
  saturation ≤ 0.3, water ≥ 0.3. Its adult needs L ≥ 0.5 (r2 viability grid).

## Proposal

**Stage 1, velvetpad (the pioneer moss):** unchanged. It colonises bare ground fast, lit or
shaded, drained or wet.

**Stage 2, springturf (the grass):** a springturf seedling can come up through **velvetpad
only**, on its own per-check draw. The existing overtop path does this, restricted to the
target:
- a new knob, `overtop_covers`: a species list per species, the ground covers its seedlings
  can come up through;
  - empty means any ground cover, which is today's behaviour, so the woody species are
    unchanged;
- springturf: `overtop_per_check` > 0, `overtop_covers = ["velvetpad"]`;
- its establishment gates apply as if the site were free, as they do for woody seeds today.
  So the takeover is confined to open (sky ≥ 0.75), not soggy (saturation ≤ 0.3) ground.

**Stage 3, the woody plants:** unchanged. Bloomcrown, lanternberry and vaulttree already
come up through any ground cover at 0.1 a check, springturf included.

**Velvetpad's resulting niche:** shaded or soggy ground. Springturf can't start there, and
no other plant breaks even in drained shade except stonecushion, slowly (r2 item 2). In
deep shade it shares with glowcap. Velvetpad's water knobs stay as today (no C1), so it
keeps its pioneer role on drained ground until the grass arrives.

## Open questions for Astra

1. **Rate.** What `overtop_per_check` for springturf? Woody plants use 0.1. The
   measurement target: the share of lit, drained velvetpad sites that become springturf
   within 1, 2 and 3 world days (2, 4 and 6 h). Is about half within 2 world days a
   reasonable target for "grasses take over"?
2. **Runners.** Should springturf's runners (clonal share 0.3) also take velvetpad sites,
   the way grass rhizomes invade moss, or only its seeds? Lean: seeds only first, one
   change at a time, then runners if it's too slow.
3. **Does velvetpad come back?** Can it retake springturf ground? Today it can't (overtop 0,
   runners need vacancies), so only springturf deaths reopen the ground. Is a one-way
   replacement right, with velvetpad persisting in shade and wet and recolonising
   disturbance (deaths, grazing kills, drowning)?
4. **Facilitation.** Real succession often needs the pioneer to build soil first. Should
   springturf's draw depend on the velvetpad mat's age or litter, or is the plain draw
   enough for now? Lean: plain draw.
5. **Fauna.** Springturf is grazeable and sheds more litter, so foliage and litter should
   hold or rise. The measure is the same paired fauna check (8 worlds × 2 h, B, R̂,
   access, bites by food) on a world that has had time to succeed: a 6 h plants-only
   world, then fauna on top. Is that the right shape?
6. **Pass condition**, fixed before running:
   - **S1:** at 6 h, springturf holds at least 40 % of lit, drained, non-soggy soil sites
     that velvetpad held at 2 h.
   - **S2:** velvetpad still holds at least 70 % of shaded or soggy sites and recruits there
     (births in hours 4–6 > 0 on every seed).
   - **S3:** occupied drained ground at 6 h is at least 90 % of today's (no bare ground
     left behind).
   - **S4:** the paired fauna B for litter shredder and frondgrazer is not below today's by
     more than 10 %.
