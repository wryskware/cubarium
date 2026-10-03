Verdict: **revise**.

# Astra — metabolism and meals review, 2026-10-02

Reviewed `design/metabolism-meals-2026-10-02.md` at main `71c26d59`, including
the landed night package and roster groundwork. This is advice for Wrysk, not
an accepted design decision. The inventory and both earlier reviews provide
context; current code and the live preset take precedence over their historical
numbers. Calculations below are static checks, not new simulations.

Lower upkeep and reserve hysteresis are a plausible way to obtain feeding bouts
without implementing a stomach. The proposed tables do not yet predict the live
behavior: they mix food with reserve units, omit activity and family costs from
cadence, and leave a basal-linked juvenile food source effectively unfunded.

1. **Important — The basal diagnosis is right, but the baseline and natural comparison need qualification.**

   **Evidence:** proposal:23–28,36–57; inventory §§2.1–2.2; parameter audit §2;
   `crates/cubarium-voxel-fauna/src/lib.rs:1599–1624`;
   `config/desktop/terrarium.toml:56–61,135–140,298–305`.
   `0.001×7200=7.2` bodies/day and `0.5/0.001=500 s` of reserve are correct
   **code-default** reference calculations. Seven-lineage cuts from that reference
   are 48–360×; the old predator baselines instead give 36× for both chorister
   and lanternjaw. The live terrarium overrides these rates: frondgrazer upkeep
   is 0.001671/s, so reaching f=0.06 is a **200.5×** cut, not 120×. Existing
   hunters already have satiated rest, and the night package is now present;
   “no rests … no night to sleep through” is no longer an accurate blanket diagnosis.

   The audit's 20–30-day body-turnover analogy implies roughly 3.3–5%/day for
   that reference mammal; it does not establish a universal 2–10% range or a
   conversion between simulated organic body units and real wet body mass.
   Field metabolism also includes activity, whereas f here is awake resting
   upkeep. Treat the table as plausible art/game defaults, not calibrated biology.

   **Change:** distinguish code defaults, current live overrides and proposed
   values. Keep low f as a candidate, but call the result a deliberately mixed
   timescale model: slow maintenance and fast tissue production can coexist only
   if meals fund the latter separately. Do not infer a 50–360× reduction in
   **total** animal demand from the basal cut.

2. **Blocking — Define C and b in consistent units; a reserve gap is not food eaten.**

   **Evidence:** proposal:61–89; `crates/cubarium-voxel-fauna/src/step.rs:632–671,2089–2104,2226–2276`.
   Reserve is organic units, while C is declared a fraction of adult body. The
   trigger must therefore be `Q ≤ c×M − C×M_adult`, with an explicit choice of
   body reference. Since feeding is multiplied by yield and may be mineral-limited,
   recovering a stored deficit `C×M_adult` requires approximately
   `C×M_adult/y_eff` **raw food**, not C body of food. Similarly, if b is raw food
   per adult body per second, the ideal duration is `C/(y_eff×b)`, not `C/b`.
   Effective yield may be below configured yield. Existing stock saturation and
   controller feeding effort reduce realized intake further.

   **Change:** preferably define C as a **reserve-deficit trigger**, in adult-body
   equivalents, and distinguish raw bite rate from net assimilation rate. Retain
   the stock, mineral and reserve-room caps. Define a persistent eating/not-eating
   latch and settle the full event immediately when filling succeeds; do not
   rely on exact equality sampled after growth or reproduction has already
   spent some reserve. Give the latch explicit founding/newborn/save-state behavior.

3. **Blocking — f/C predicts only an awake, resting, non-growing, non-breeding adult; the proposed acceptance bands include animals for which it is wrong.**

   **Evidence:** proposal:69–85,91–92,133–140;
   `crates/cubarium-voxel-fauna/src/step.rs:447–545`;
   `crates/cubarium-voxel-fauna/src/body.rs:723–762`; `crates/cubarium-voxel-fauna/src/knobs.rs:143–186`.
   For a fixed adult and small, fast meals, the useful approximation is
   `meals/day ≈ total reserve expenditure per day / (C×M_adult)`.
   Total expenditure includes sleep/torpor-adjusted upkeep, motor, lure, strikes,
   healing, reproduction and transfers. At roughly 68% day/32% night, a diurnal
   adult sleeping at 0.5 burns `0.84 f`, giving a non-breeding chorister about
   **1.79 days** between threshold crossings. A nocturnal ectotherm sleeping at
   0.3 burns `0.524 f`: a shredder gets **1.05**, not two meals/day, and an
   unluring resting lanternjaw gets **4.77 days**, not 2.5. Those differences
   exceed ±30%. Luring and travel change the numbers again.

   If satiated torpor at 0.25 is retained between meals, resting gaps approach
   **6 days for choristers and 10 for lanternjaws**, before other spending.
   Replacing rest_at/wake_at does not say whether torpor itself is removed.
   A ten-day gap even exceeds the default lanternjaw's six-world-day median life.

   **Change:** explicitly choose post-meal activity and metabolic multipliers.
   Derive C from the resulting adult expenditure if 1–2-day pack meals and
   approximately 2.5-day ambusher meals are the intended resting benchmarks.
   Specify cadence exceptions for breeders, juveniles, active flight, luring
   and food shortage. Alternatively keep f/C only as an isolated all-awake basal
   test, with separate rhythm-aware behavioral expectations.

4. **Important — The frondgrazer bite-rate constraint only fits the short end of the meal-duration range, and “45 min to adult” is the wrong baseline.**

   **Evidence:** proposal:94–106,141–142;
   `crates/cubarium-voxel-fauna/src/step.rs:606–628`;
   `crates/cubarium-voxel-fauna/src/lib.rs:1599–1624`; `design/parameters.md:1287–1301`;
   `config/desktop/terrarium.toml:298–313`.
   At mass scale 30, code-default frondgrazer adult mass is 1.5 ou, newborn
   structure 0.15, breeding structure 0.9 and full-reserve growth G=0.000285 ou/s.
   Thus **43.86 min reaches breeding size**, while **78.95 min reaches adult
   size**, even with fullness held at one. Live-preset G=0.000308044 gives
   approximately 73.04 minutes to adult. Use the selected baseline in acceptance.

   A worked throughput budget: C=0.03 implies a 0.045-ou adult reserve gap.
   Growing at G while keeping reserve near cap c=0.5 requires approximately
   `(1+c)G = 0.0004275 ou/s` of assimilation, before maintenance: growth both
   spends reserve and increases its capacity. With 4,896 waking seconds/day and
   at most 60% of them feeding, sustained full-speed growth requires eating-time
   assimilation of at least `0.0004275/(0.6×4896/7200) = 0.001048 ou/s`.
   At effective yield 0.4 this means raw bites of at least **0.00262 ou/s**.
   The same net rate refills a non-growing adult's 0.045-ou gap in about
   **43 seconds**, shorter than the proposed 1–5 minutes.

   Allowing growth at two-thirds speed (the 1.5× time allowance) lowers that
   throughput requirement enough for roughly **64 seconds** per adult meal,
   before basal costs: about one minute is plausible, five minutes is not under
   these assumptions. A five-minute adult refill supplies only 0.00015 ou/s
   while eating, about seven times below the full-speed requirement. Actual finite
   juvenile trials depend on birth phase and initial endowment, so the day-averaged
   calculation is a sizing check, not a predicted individual trajectory.
   Raw growth is **1.368 adult bodies/day** here; growth plus reserve-cap expansion
   is **2.052** (about 2.218 on the live preset). “2.25 body/day” needs this distinction.

   **Change:** publish a per-lineage net-intake budget and body normalization.
   Choose either short adult bouts, stage-dependent bite rates, larger adult meal
   deficits, or slower juvenile growth; one fixed rate cannot independently set
   all three. Test young bodies as well as adults, especially if b scales with
   current rather than adult body mass.

5. **Important — Fast breeding will dominate many adults' meals and cannot be treated as a small cadence correction.**

   **Evidence:** `crates/cubarium-voxel-fauna/src/step.rs:2318–2446,2540–2575`;
   `crates/cubarium-voxel-fauna/src/lib.rs:1405–1448`; inventory §2.2.
   A frondgrazer's 0.3-ou offspring package is **6.67** proposed adult reserve
   gaps (0.045 ou), versus only 0.09 ou/day of proposed awake basal upkeep.
   Full reserves put an adult above the package-plus-floor eligibility line
   (0.45 ou at code defaults). The meal trigger, near 0.705 ou, also keeps it
   above that line much of the time, so funded adults will breed repeatedly.
   Default refractory 1,800 s is not the entire interbirth interval: add the
   surplus window and gestation. At continuously adequate reserves, the 80% of
   900-s hold and 180-s gestation make roughly **45 minutes**, rather than 30.
   The live preset's corresponding ideal interval is about **27 minutes**.

   A gestation can consume several C-gaps during one bout. The actual bout may
   be long rather than several separate meals, depending on whether intake can
   refill while escrow is being funded. High reserve/cap changes also alter
   breeding eligibility; “unchanged life-history fields” does not mean unchanged
   realized fecundity.

   **Change:** retain fast life histories if that is the choice, but make breeder
   demand and bout size explicit. Measure births, escrow spending and feeding
   duty together. Use non-breeding adults for the basal cadence test and a
   separate conservation/feeding test for breeders; do not promise all adults
   only two meals/day under ample food.

6. **Blocking — Pack provisioning can trigger another meal in seconds; the pack cadence is not an individual basal ratio.**

   **Evidence:** proposal:84–97,139–140;
   `crates/cubarium-voxel-fauna/src/pack.rs:162–231`;
   `crates/cubarium-voxel-fauna/src/body.rs:450–522`; `crates/cubarium-voxel-fauna/src/step.rs:2540–2575`.
   One adult chorister's C-gap is `0.12×2.744=0.32928 ou`. Provisioning at the
   den can transfer `0.01×2.744=0.02744 ou/s`, so an adequately needy pup can
   make the adult cross that meal trigger in **12 seconds**. The 60% reserve
   floor allows up to 1.0976 ou to leave a full adult, of which 0.7683 reaches
   pups at efficiency 0.7. This is over three C-gaps. A litter itself costs
   1.066–2.132 ou, before juvenile growth and upkeep. These costs do not shrink
   with the basal rate. Individual fullness and asynchronous pup demands also
   do not guarantee a synchronized three-adult feast or a shared hunt interval.

   The pack arithmetic `3×0.12×2.744=0.98784` is correct **as stored intake**.
   At yield 0.5 it withdraws about **1.976 ou raw carrion** from a fully stocked
   loftstrider's 7.92 ou, whose potential assimilated yield is 3.96. The proposal
   must not compare raw consumption with assimilated carcass yield. The one-hour
   decay is an e-fold time, not disappearance at one hour; the live preset's
   0.000192808/s gives about **1.44 h**. Residual carrion may provide the next
   meal, so needing a meal does not imply needing a new kill.

   **Change:** specify whether Wrysk's 1–2-day target applies to a non-breeding
   pack or also to a pack raising pups. For the latter, budget the whole family
   over a litter cycle and choose meal/provisioning parameters accordingly.
   Gate pack target acquisition as well as eating; log pack meals, individual
   intake and kills separately. Keep conservation and decay, and do not impose
   “every adult consumes C after every kill” on unequal or famine-depleted reserves.

7. **Blocking — A bite gate alone neither seeks food nor prevents satiated hunting, and sleep needs a separate emergency rule.**

   **Evidence:** proposal:63–65,91–114;
   `crates/cubarium-voxel-fauna/src/step.rs:447–518,1206–1237,1460–1489,2089–2104`;
   `crates/cubarium-voxel-fauna/src/pack.rs:235–245,369–382,452–463,563–578`.
   Feeding still requires controller effort and food contact. A frondgrazer's
   new meal trigger occurs at hunger `C/c=0.06`; untouched brains observing
   that small hunger need not seek food or request full effort. Removing the
   hunger factor from bite volume does not remove the effort factor. Hunters
   start strikes before the ordinary feeding function, and pack target/feast
   logic independently reads rest_at. Gating only `founder_feed` leaves those
   kill attempts possible. Flyer sipping has its own hunger-scaled path.

   Meal hunger and emergency wake hunger are very different thresholds:
   0.06 for the default grazer meal versus 0.9 for waking from sleep. If any
   meal request wakes a sleeper, the promised uninterrupted sleep may disappear.
   If only emergency hunger wakes it, meal starts can be delayed until dawn,
   accumulating more than C. Growth and breeding continue during sleep. Also,
   `resleep_hunger` is not in main at this commit; the branch reference is not
   an implemented main rule.

   **Change:** define precedence for circadian sleep, meal demand, emergency
   waking, alerts and satiated rest. Preserve an emergency-to-refill latch so
   a hungry sleeper does not alternate sleeping and taking one bite. Wire the
   meal state into hunting, pack steering, flyer feeding and all relevant intake
   paths. Keep the numeric sense contract if desired, but distinguish that from
   claiming unchanged controller behavior will seek/refill successfully. Validate
   the existing brains and known feeder-access failures on this new hunger range.

8. **Blocking — The basal cut also cuts lanternjaw rider food, while its growth target remains fast.**

   **Evidence:** `crates/cubarium-voxel-fauna/src/step.rs:320–373`;
   `design/parameters.md:1391–1399,2205–2208`; inventory §2.2;
   earlier roster review, findings 2 and 8.
   A rider gets 5% of its host's **awake basal upkeep**, independent of the host's
   activity. Under proposed frondgrazer f=0.06 and body 1.5, this is only
   `0.05×0.06×1.5 = 0.0045 ou/world-day`. Default lanternjaw dropoff size is
   `0.24×1.05=0.252 ou`. Even generously crediting the **entire 0.15-ou egg**
   toward structure, it needs another 0.102 ou: **22.7 world-days** of this
   income with zero rider maintenance and no retained reserve. That already
   exceeds the default six-day lanternjaw lifespan and the host's lifespan.
   Host death may release the rider sooner, but that is not successful growth.

   **Change:** decouple parasite feeding from the newly tiny basal rate, or
   explicitly reparameterize it against host reserves/body and the juvenile
   growth budget. Keep host harm accounted for. A meal model that only changes
   mouth bites cannot repair a rider's intake path; this dependency must be
   resolved before claiming predator behavior emerges naturally.

9. **Important — A stomach is unnecessary for hysteresis, but C cannot be advertised as a maximum meal size.**

   **Evidence:** proposal:61–73,136–140;
   `crates/cubarium-voxel-fauna/src/step.rs:606–671,2226–2276`.
   After famine, a grazer can have a deficit close to 0.75 ou rather than 0.045:
   filling it takes up to **16.7 C-gaps**, plus ongoing spending. During juvenile
   growth, both structure and reserve capacity rise while the animal eats. A
   breeder or provisioning adult may not reach full at all until that demand
   stops. Thus an episode can exceed C or remain continuously active without
   violating the proposed rule. The fixed adult-sized gap can also be larger
   than a small juvenile's entire capacity: a roster reserve multiplier of 0.5
   gives a newborn grazer cap 0.0375 ou, below its 0.045-ou C-gap, so the literal
   trigger would require negative reserve.

   **Change:** name C as a trigger and explicitly allow refill bouts above C,
   or add a separate per-bout quantity/time limit if a hard meal size matters.
   Neither requires a gut model. Scale or clamp the juvenile trigger and ensure
   every viable body can request food before exhausting its reserve. Record
   normal, emergency and growth/refill bouts separately so famine recovery does
   not fail a normal-meal test or masquerade as successful adult cadence.

10. **Important — Rebase the search explicitly; the existing roster file does not implement the new model or preserve its intended constraints.**

    **Evidence:** proposal:118–129; `config/training/search/roster.toml:29–33,41–74,103–148`;
    `crates/cubarium-voxel-fauna/src/registry.rs:56–69`;
    `crates/cubarium-search/src/es/voxel/live/screen.rs:280–355,421–425,616`
    (`rsf`, `night_endurance_s`, basal food demand and `roster`).
    The live preset overrides code upkeep; changing code defaults alone leaves
    `[base] file="config/desktop/terrarium.toml"` on the old rates. The active
    intake-follow-upkeep tie would slash bites along with basal. ×10 intake
    compensation cannot undo a 50–360× cut. R and F rise inversely with upkeep
    when other terms and the motor ratio stay fixed; **S does not**—its
    denominator is `need×turnover`, so upkeep cancels. Main's roster still makes
    only R readout-only; S and F retain their constraints.

    Basal-only food ratios and adult night endurance will become easy to pass,
    but tell little about growth, reproduction, provisioning, riders or food
    access. The meal gate is not an independent population limit: it responds
    to demand and does not cap funded reproduction. Lower grazing can leave
    vegetation richer and alter litter/nutrient recycling, but whole-community
    demand does not fall by the basal factor. Animals unable to reach food
    still starve eventually. With full founders and reserve endurance exceeding
    many lifespans, a two-hour survival result becomes particularly weak evidence.

    **Change:** in roster.toml/base resolution, explicitly set each daily f and
    its conversion on the **ecological** day (define off/local behavior; RT-A's
    picture clock must not change physiology). Free `intake_follows_upkeep`
    while preserving the intended motor tie; give b independent units/ranges
    and add named C genes or fixed values with juvenile-cap constraints. Widen
    registry/search ranges to reach roughly **2.78e-6–2.08e-5/s** at 7,200 s/day.
    Make the intended S/F policy real, recompute food budgets with production/
    family costs, and preserve actual feeding/replenishment targets. Freeze
    life histories explicitly and inspect resolved values before another search.
    Do not ask plant genes to compensate for an unreachable mouth or an unfunded
    rider. Earlier review concerns about misleading stage-only passes still
    require care; survival on initial stores is not sufficient acceptance.

11. **Important — Replace the universal ±30% cadence test with measurable, attainable cases.**

    **Evidence:** proposal:131–148; the spending/state interactions above;
    `crates/cubarium-voxel-fauna/src/night_survival_tests.rs:63–116` already
    disables breeding to isolate overnight adult upkeep.
    Counts in a single day are quantized and depend on starting reserve, clock
    phase and whether a bout spans the endpoint. A two-hour terrarium run is
    shorter than either proposed predator intermeal gap, and ample-food adults
    will reproduce unless the fixture explicitly disables it. No-nibbling means
    **no intake outside an active bout**, not no intake at every below-full state.

    **Change:** use small state/accounting tests for threshold crossing, latch
    retention, capped final bite, hunting inhibition and wake precedence. Use
    bounded studies (not long ordinary unit tests) for intermeal intervals and
    feeding duty. For the isolated cadence check fix body size, reproduction,
    activity, yield and food access, and compare against integrated expenditure;
    apply ±30% to intervals or rates over enough completed bouts. Separately test:

    - a famine/deep deficit and successful recovery without a C-size assertion;
    - a hungry predator with no prey: no meal counted, no reserve creation, and
      no assumption that hunts succeed on schedule;
    - a newborn at minimum body/cap, growth across a sleep phase, and reserve
      capacity increasing during a meal;
    - breeders and a pack with pups, where extra food is actually acquired rather
      than paid entirely from founding stores;
    - uneven pack reserves, interrupted/failed hunts, shared dwindling carrion,
      and resumed feeding without requiring a new kill;
    - a rider receiving enough food to grow under the new host metabolism.

    Keep the proposed eight-seed two-hour observation as an early access/behavior
    check, adding raw/assimilated meal size, duration, per-body duty, life stage,
    reproduction/provisioning expenditure and reserve change. Use targeted later
    observations for the explicitly multi-day predator intervals. Those additions
    test whether the behavior actually emerges, rather than whether the basal
    table can be reproduced in a simplified fixture.
