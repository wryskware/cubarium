Verdict: **revise**.

# Astra review — ecology timescales, 2026-10-02

Independent review of `design/ecology-timescales-2026-10-02.md` at `7c7c991d`.
This is research evidence and advice for Wrysk, not acceptance of a design decision
(`design/0_Canon/README.md`, D-0001/D-0002). The direction is useful: retain the
compressed cycle, budget sleep explicitly, and avoid adding photosynthetic moonlight.
The capacity claims and full-roster search specification need correction before approval.

Evidence: the requested inventory, parameter audit §2, co-training result dated
2026-10-02, and implementation inspected against the review commit (no crate diff).
“Inventory” below means `timescales-inventory-2026-10-02.md`. Calculations are static
checks, not new simulations. Lore retrieval was unavailable under the tool approval
policy; Graft and the referenced source files supplied implementation evidence.

1. **Important — The clock mismatch is real; it is not a complete diagnosis of the failures.**

   **Evidence:** inventory §§1.1–1.3, 2.1, 5–6; parameter audit §2;
   `crates/cubarium-voxel-fauna/src/step.rs:447–545,632–638`.
   With reserve/body = 0.5 and maintenance = 0.001/s, full awake reserve lasts
   `0.5/0.001 = 500 s = 8.33 min`. At sleep multiplier 0.5 it lasts 1,000 s,
   but hunger reaches 0.5 after 500 s, so the stated maximum uninterrupted sleep
   from full is correct. These are reserve limits, not time to death: structure
   can subsequently be burned. `120 × (1−0.67) = 39.6 min` is sun-down duration;
   actual sleeping time follows daylight's 0.3 threshold, not the solar horizon.
   Nocturnal lineages must budget approximately 80 minutes of daylight sleep.

   The audit's 2,000–5,000× is a rough mammal-metabolism reference, not a common
   acceleration for all biology. Its explicit 20–30 days versus 1,000 seconds
   gives 1,728–2,592×; gestation, plants and lifespan have different factors.
   At the illustrative 3,000×, 40 minutes means 83 days, and 12 hours means
   1,500 days. “About 1,000” uses the low end of the stated range. More importantly,
   inventory §6 shows low daytime food acquisition: seven lineages fail before
   night, bellwing/seedporter intake is only 4%/1% of outflow, and many plants grow.
   Night endurance cannot by itself repair that.

   **Change:** qualify the headline as a metabolism comparison; replace “every
   symptom follows” with separate clock-budget and food-access/recruitment diagnoses.
   Preserve the distinction between defaults in inventory §6 and trained ecologies
   in the co-training gate. Full founders are a useful founding change, not evidence
   of sustainability.

2. **Important — Keep the endurance screen, but include usable reserve, wake margin and competing costs.**

   **Evidence:** `fauna/src/step.rs:447–545,547–630,632–638` (paths in this finding
   are under `crates/cubarium-voxel-fauna/`); inventory §2.1.
   For an adult with fixed structure, let `c` be reserve/body, `m` basal upkeep,
   `s` sleep multiplier, `f0` reserve fraction at sleep onset and `h` wake hunger.
   The relevant uninterrupted-sleep time is
   `max(0, f0−(1−h)) × c/(m×s)`, not simply `c/(m×s)`.
   The proposed full-reserve screen with 1.5× margin and h=0.9 does imply
   1.35 sleeping phases before waking **if it starts full**. At that boundary,
   it needs `f0 ≥ 0.1 + 1/1.5 = 0.767` merely to finish one phase asleep.
   Half-full entrants do not pass the behavioral interpretation.

   At current upkeep, h=0.9 gives 15 minutes of sleep at s=0.5, or 25 minutes at
   s=0.3: neither fixes a 40-minute night or an 80-minute day. At fixed reserve,
   the proposed gross screen needs roughly 3.6× lower upkeep for diurnal s=0.5,
   4.3× for nocturnal s=0.3, and 7.2× for nocturnal s=0.5. Juvenile growth,
   healing and gestation spend reserve even during sleep; alertness and den travel
   interrupt sleep. A crepuscular animal has two separate sleeping bouts, with
   only short feeding opportunities between them.

   **Change:** retain a cheap optimistic adult screen, label it as such, and add
   phase-specific onset-reserve and whole-cycle replenishment checks. Budget
   growth, reproduction, host drain (finding 8), and waking costs separately.
   Measure actual activity-window durations from the clock. Sleep factors 0.5/0.3
   are reasonable provisional game knobs; the inventory's BMR torpor anchors do
   not directly calibrate a sim maintenance rate compared with field metabolism.

3. **Important — Raising wake hunger changes predation and starvation behavior, not just sleep length.**

   **Evidence:** `crates/cubarium-voxel-fauna/src/predation.rs:143–157`;
   `src/step.rs:467–484,1722–1741`; inventory §1.3 and §2.3.
   Sleeping prey have zero dodge awareness. A hit wakes them and sets an alert
   timer, so it is wrong to infer that every subsequent hit is also guaranteed;
   nevertheless, longer sleep gives predators many more undefended first strikes.
   At h=0.9 an animal wakes with only 10% of reserve. With today's small-lineage
   upkeep that is 50 seconds of awake basal reserve, before movement or other costs.
   It can then catabolize structure; lowering upkeep stretches this runway but
   does not guarantee finding food. Hunter satiation also has its own wake_at=0.5:
   raising wake_hunger alone does not extend satiated torpor to 90% depletion.

   **Change:** treat h=0.9 as a candidate, not a universal fix. Check sleep entry,
   emergency waking, first-hit exposure and post-wake food access together. Keep
   alert wake behavior explicit and test nocturnal lineages across their longer
   daylight sleep. Do not claim a reserve screen proves safe sleep.

4. **Important — Night reflush can conserve material; “the day's total energy is unchanged” is too strong.**

   **Evidence:** `crates/cubarium-voxel-flora/src/step.rs:2577–2708`;
   inventory §3; `design/parameters.md` photosynthetic `reflush_below` rows.
   Existing reserve reflush pays construction through `budget.build_from`, and
   nectar/fruit transfers subtract precisely what they add. Extending those paths
   into darkness need not create energy. However, reserve spending changes foliage,
   respiration, tomorrow's capture capacity and available reserve room; photosynthesis
   itself is capped by spend/storage demand. Daily fixation is therefore not invariant.
   Initial planted stocks are also endowments, not previously measured day income.

   The proposed starch rule lacks allocation priorities. Spending Q/time-to-dawn
   on leaves *in addition to* maintenance, nectar and fruit can exhaust the store
   early. Current reflush only operates below 25% of foliage capacity, so merely
   removing its light gate will not repair the ordinary 10–13% night foliage loss
   measured in the inventory. That may be acceptable, but it is a different promise.

   **Change:** state conservation as “no new unbooked input.” Allocate one spendable
   reserve budget after maintenance and a dawn safety floor, share it among leaves
   and reproductive products, and retain construction respiration and material
   accounting. Specify the reflush threshold, rate cap, and bounded time-to-dawn
   behavior, including clock-off/local modes. Do not silently turn a starch analogy
   into full reserve depletion or replacement of every lost leaf.

5. **Blocking — The nectar/fruit screen mixes food and assimilated units and overstates daily supply.**

   **Evidence:** inventory §§2.1, 3, 4.1;
   `crates/cubarium-voxel-flora/src/step.rs:2683–2715`.
   A seedporter's `0.356 × 0.001 × 7200 ≈ 2.56 ou/day` is assimilated basal
   demand, not fruit eaten. At yield 0.5 it requires 5.13 ou of fruit if always
   awake. With ideal day activity/night sleep at 0.5 it still needs about
   `0.356×0.001×(4824+1188)/0.5 = 4.28 ou` of fruit per day.
   A stand's 0.72 ou/day assumes production for all 7,200 seconds. Using the
   proposal's approximate phase lengths gives only 0.482 ou by day, plus
   `0.0001×0.3×2376 = 0.0713 ou` at night: about 0.554 ou/day, or 0.277
   assimilated. Thus approximately **eight** continuously harvested, fully supplied
   adult stands support the ideal sleeping seedporter's basal budget, not four;
   the always-awake case needs about ten. Twilight modifies this estimate slightly.

   These are optimistic ceilings: 0.02 stock caps, fruit drops, stand maturity,
   accessibility, plant reserve competition, motion and reproduction all matter.
   Existing stocks remain food at night; stopping production is not an automatic
   40-minute famine. Both proposed beneficiaries are diurnal and would sleep more
   under Part 1a. At 0.3×, nectar/fruit reserve spending is conservative if debited,
   but can starve plants or reward concentrated farming without creating energy.

   **Change:** compute edible production and consumer demand in the same units,
   integrate actual day/night rates, and include accessible stock, yield and shared
   plant reserve limits. Apply it separately to nectar and fruit consumers. Treat
   0.3 as a tunable candidate. “No photosynthetic moonlight” is sound for this design;
   retaining picture-only moonlight is consistent with inventory §1.4.

6. **Blocking — The predator population claim is not supported by the balance arithmetic.**

   **Evidence:** inventory §§2.3, 4.2; co-training plan, 2026-10-02 result.
   The baseline arithmetic checks: lanternjaw basal cost is
   `0.00021×(2376+0.5×4824)=1.006 ou/day`, with full-night lure adding 0.499.
   A full adult grazer supplies `0.5×(1.5+0.75)=1.125`, giving 0.89–1.34
   kills/day. A chorister costs about 6.60, or 5.87 grazers/day; four cost 23.46.
   These omit satiated torpor savings and hunting/reproduction losses; actual
   realized carcass intake also depends on reserve room, decay and competition.

   The proposed screen's algebra gives 10–15 predators from 10–15 births at
   0.5 kills each and a 50% allocation. But most of those births are not grazers.
   C1 supplies 9.75 grazer births and 5.125 shredder births per world-day.
   Valuing them optimistically as eventual full adults gives
   `0.5×(9.75×1.125 + 5.125×0.28125)=6.205 ou/day`, enough for about **11.0**
   lanternjaws at 0.5625 ou/day each. X21 gives about **7.6**, defaults about
   **1.25**, on the same deliberately optimistic calculation. Newborns are smaller,
   non-predatory mortality consumes recruitment, and choristers compete for prey.
   A kill of a shredder is not a grazer meal. A world at 150 animals is also an
   extrapolation: the gate's trained populations are much smaller per world.

   **Change:** replace “sustains 10–14” and “this is why unreachable” with a
   conditional energy ceiling. Allocate demand across prey species and recruited
   biomass, subtract other deaths, and check competing predators together.
   Prey births at a proposed population band are not known before running it:
   use a generous necessary-condition pre-screen, then measure realized births,
   survival, kills and consumption. Do not reject genomes on unvalidated gate
   extrapolations. Define “meal” as realized intake; ≤0.5 grazer-equivalent kills/day
   corresponds to ≥2 days between such meals, not the whole stated 1–3-day range.

7. **Important — Packs already take large prey; the smaller-pack budget must include pups and replenishment.**

   **Evidence:** `crates/cubarium-voxel-fauna/src/pack.rs:79–97,101–231,361–381`;
   `src/predation.rs:86–108`; `src/body.rs:450–522`;
   `src/step.rs:2375–2416,2523–2558`.
   Prey eligibility already uses the **sum of nearby adult packmates' bodies**.
   Two full choristers allow `1.5×2×2.744=8.232 ou`, exceeding the adult
   loftstrider's 5.28. The inventory's 4.1-ou cap is the lone-hunter cap, not a
   pack cap. Adding √pack-size would double-count cooperation if layered onto it.
   A full loftstrider is indeed worth `0.5×(5.28+2.64)=3.96 ou` before losses.

   With upkeep ÷5 and the old diurnal rhythm, 2–3 adults need 2.64–3.96 ou/day:
   roughly 0.67–1 loftstrider/day before pups. Crepuscular sleep reduces that,
   but even continuous ordinary sleep at 0.5 costs 1.58–2.37 ou/day for 2–3 adults.
   Three therefore cannot meet the two-day-per-loftstrider end without additional
   satiated torpor or other savings, even before hunting and reproduction.

   Provisioning happens only at the den, above 60% giver reserve, at efficiency
   0.7; pups otherwise can use a nearby carcass. At current adult size, transfer
   rate is 0.02744 ou/s, so the reserve/supply/location constraint matters more
   than nominal transfer speed. Two to four pups cost 1.066–2.132 ou just to
   produce, plus growth and upkeep for 7,200 s before independence. The breeder
   must fund at least two packages above its floor. The 21,600-s interval is
   between litters; **it does not delay the first litter by six hours**. Pack size
   config controls founding adults, not a persistent adult population cap.

   **Change:** remove the proposed new pack prey-cap mechanic; tune the existing
   local summed-body rule only if measurements warrant it. Budget an entire
   litter cycle, including den delivery and juvenile losses. Measure usable dawn/
   dusk hunting time (`Activity::awake`, `src/lib.rs:567–577`; thresholds
   `src/knobs.rs:175–186`), rather than assuming two broad hunting sessions.

8. **Blocking — Lanternjaw host-dependent recruitment is absent from the proposal and inventory.**

   **Evidence:** `crates/cubarium-voxel-fauna/src/step.rs:320–373,2867–2988`;
   `design/parameters.md:2167–2170`.
   Passing footfall, adult exclusion and the 1/3 dusk draw is insufficient:
   hatching also requires a host within 0.75 m with a free rider slot (default
   one). A hatchling attaches, drains **5% of its host's awake basal upkeep**
   regardless of host activity, and normally detaches at 24% of lanternjaw adult
   body. This is an additional recruitment bottleneck and a non-kill predator
   demand, omitted by a kills-versus-births screen.

   For a sleeping host at s=0.3 with one rider, basal-plus-drain is 0.35 rather
   than 0.3: endurance falls about 14%. At s=0.05, that drain doubles the
   sleeping host's expenditure. Lowering prey upkeep also lowers juvenile
   lanternjaw income. Raising host sleep can reduce movement/footfall near eggs.

   **Change:** add host encounter, occupied slots, rider growth/dropoff, host loss
   and adult establishment readouts before searching predator balance. Include
   rider demand in sleep and prey budgets. Diagnose the gate's establishment
   failures against these gates before attributing them to adult kill cadence.

9. **Important — RT-A is a reasonable ambient-art option, but not a zero-cost existing mode.**

   **Evidence:** inventory §§1.1–1.4; proposal Part 2; D-0002.
   RT-A preserves a lively compressed ecology while allowing a local-time picture.
   Its visible mismatch is an aesthetic tradeoff for Wrysk: night-active creatures
   may hunt under a daytime picture. It needs explicit ecological and presentation
   clocks: photosynthesis, sleep, hatch gates and relevant climate/light inputs
   must continue to read the ecological clock. Simply switching today's `local`
   mode does not implement that split. Local nights are not universally 12 h:
   latitude/longitude uses seasonal sunrise/sunset, with clamped extremes.

   RT-B should divide **rates** by 12 and multiply **durations** by 12, including
   lifespan, gestation, egg viability, pack intervals and other coupled clocks.
   The stated 4–12 h lives become **2–6 real days**, not 1–2. Preserving real
   movement speed changes encounters, transport and motor accounting. It can be
   searched headlessly; cost follows required ticks and interactions, not elapsed
   wall-clock days. A transferred compressed candidate is a sensible starting
   point, but a single retune is not guaranteed sufficient.

   RT-C's 5–10% solar input is about **19,000–38,000×** the quoted natural lunar
   ratio, not 380,000×. Nor is 5–10% torpor alone sufficient: a small lineage's
   full reserve at current upkeep lasts only 2.78–1.39 h at those multipliers.
   Forcing all animals to sleep at night also removes the nocturnal niche.

   **Change:** keep cycle as default and offer RT-A as an explicitly separate
   presentation choice, subject to Wrysk's taste. Correct the table and describe
   the clock-split work. Add accelerated/shortened nights as an option with its
   limitation: accelerating *all* simulation and sky together shortens wall time
   but preserves metabolic expenditure per night; accelerating only the ecological
   sky reduces that expenditure but abandons local solar synchronization. Accelerating
   biology under a fixed real-time sky makes the starvation problem worse.

10. **Blocking — The two-hour targets confuse founding survival with replacement and can reward the wrong result.**

    **Evidence:** inventory §§1.3, 2.2, 6;
    `crates/cubarium-voxel-fauna/src/step.rs:2867–2949,2523–2558`;
    `crates/cubarium-search/src/es/voxel/live/search.rs:458–478,572–639`.
    A two-hour run includes one dusk, not repeated egg-bank recruitment. Nine
    founding eggs yield at most about three expected hatches at the 1/3 draw even
    before host/footfall constraints. Default lanternjaw first breeding needs
    roughly 176 minutes from birth, plus a later dusk for its eggs. It cannot
    demonstrate a replacement generation in that horizon. The scorer's late-birth
    window also depends on start phase and aggregation windows, and its readiness
    fallback can remove a breeding penalty without an actual birth.

    Conversely, full adult chorister founders can fund a first litter early.
    Requiring a second-half litter penalizes that successful early birth because
    the next is locked out for six hours. Requiring merely any litter can reward
    spending the founding endowment. No pup born after time zero reaches the
    two-hour independence age within a two-hour run. Increasing endurance and
    filling founders makes “survives two hours without acquiring food” easier.

    **Change:** keep 8×2 h as the economical initial gate, but use lineage-specific
    outcomes: establishment, positive replenishment, successful feeding/kill and
    viable offspring/escrow where an entire replacement cycle cannot fit. Separate
    hatching founding eggs from adult reproduction and riders from established
    hunters. Check pup survival and provisioning, not a rigid second-half litter.
    Validate finalists from representative saved worlds across the relevant next
    dusk, pup independence and litter interval; these are the explicit slow-clock
    reasons for longer checks allowed by WORKING_POLICY. Do not lengthen every
    candidate evaluation to six hours.

11. **Important — Make the search space and food-access checks explicit before an overnight run.**

    **Evidence:** `crates/cubarium-voxel-fauna/src/registry.rs:48–69,119–143,206–207`;
    `crates/cubarium-search/src/es/voxel/live/screen.rs:202–257`;
    inventory §6; co-training result dated 2026-10-02.
    Sleep depth and wake hunger already have per-founder registry rows. The
    missing work is selecting/wiring the intended genes, not inventing the rows.
    Part 1 allows reserve to move, but Part 3 does not explicitly include
    reserve_cap. Existing registry upkeep bounds are 2.5e-4–4e-3/s; the proposed
    lanternjaw 2–3× reduction from 2e-4 and chorister ÷5 from 4e-4 fall below that
    range if those absolute bounds are used. Confirm the actual candidate decoder
    and ranges instead of assuming “within its gene range.” The upkeep tie also
    lowers bite rate and motor cost: lower expenditure can mean slower carcass
    recovery unless intake-factor genes compensate.

    R/S/F currently reads the frondgrazer physiology and photosynthetic foliage
    band. It does not establish viability for nectar, fruit, fungal caps, film,
    litter, high-canopy browsing or carnivory. Food abundance is not food access;
    the inventory's abundant plants and starving founders demonstrate the gap.

    **Change:** list the actual per-lineage gene ranges and ties, including reserve,
    sleep/wake and intake, and exhibit a candidate satisfying all analytic screens
    before starting CMA. Check founding food accessibility and each lineage's
    replenishment early; inspect bells/fruit consumers especially. Retain multi-seed
    early stopping and throughput measurement. Holding C1's two brains fixed is a
    reasonable controlled experiment, but their movement/feeding performance must
    be remeasured after changed costs and rhythms, with a later lock step retained.

12. **Minor — Separate immediate proposals from deferred life-history intentions.**

    **Evidence:** proposal “The principle,” Part 1d and final decisions;
    inventory §2.2.
    First breeding “after at least one full day and night” conflicts with keeping
    roughly one-hour first breeding under a two-hour cycle. Two to six days of
    lifespan is also a different commitment from several days to adulthood.

    **Change:** label delayed maturity as a later option, or change the current
    principle to match the retained life histories. Present sleep, night plant
    spending, predator cadence and RT-A as separable choices; none becomes canon
    through this review.
