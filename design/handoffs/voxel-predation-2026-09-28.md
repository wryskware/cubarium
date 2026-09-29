---
status: decisions made by Wrysk 2026-09-28; awaiting his go to dispatch (P1 waits for WX4 on main)
date: 2026-09-28
owner: Fable (organism line)
design: design/predation-and-flight-2026-09-28.md
---

# Predation: P1 substrate with the lanternjaw, P2 contract v3 and training

The design note is the spec. Section numbers below (§2.3 …) are the note's. Where
Wrysk overturns a line of §0, the brief follows him.

## Dependencies

- **P1 starts on main after WX4 merges** (WX4 adds `State::Asleep`; P1 extends it).
  If the lifespan package (`live-train`, `Departure::Aged`) is on main by then, take
  it too; if it lands later, merge main in before reporting. Schema numbers are
  whatever main's next free one is.
- **P2 needs live-world training on main** (branch `live-train`), which is Wrysk's call.

---

## Brief P1: predation substrate, lanternjaw body and heuristic

**Objective.** Lanternjaws that hatch from a buried egg bank, ambush, strike, wound,
kill, leave a carcass and eat it, rest, and lay the next bank, in a live world, driven
by heuristics. Prey heuristics flee. Everything visible
with interim looks.

**Worktree** `.claude/worktrees/predation`, branch `predation` off main.
**Crates:** `cubarium-voxel-fauna` (lib, body, step, senses, encounter, manifest's
`Founder`, controller, snapshot), `cubarium-voxel-flora` (carrion decay, carcass
record), `cubarium-voxel-sim` (introduction, habitat heuristics), `cubarium` (census
columns, the introduce control, interim presenter poses).

### What to build

1. **Lineage.** `Founder::Lanternjaw` with `FounderPhysiology` from §2.12, prey list
   and size window (§2.1), carnivore diet (carrion only at the mouth), `Nocturnal`.
   The v2 manifest machinery needs a lanternjaw manifest to exist; give it the
   Browser's layout plus a `lure` action so a policy file could load later. P2 replaces
   it.
2. **Bodies with volume** in the cone occupancy (§2.2), and the per-observer kind
   lookup `kind_of(observer, other) -> Prey | Threat | Other` with the still-threat rule,
   and the tremor field. P1 exposes these as functions the heuristics call; the v2
   manifests keep reporting `body` as today.
3. **Strike** (§2.3): the carnivore's `feed` → strike capsule test, windup, lunge under
   the step rule, recovery, cost at the lunge, contact then dodge. Strike outcomes are
   events applied serially after the act phase. The lure: lit while the heuristic waits,
   costing `lure_power`.
4. **Wounds** (§2.4): `Animal::wound`, damage, armour, speed cap, paid healing,
   `Counter` seam (a `None` for every lineage).
5. **Death** (§2.5): `Departure::Killed { by }`, whole body to carrion on the prey's face;
   carrion decay at the new rate; the carcass record on the ground pool.
6. **Carcass feeding** (§2.6): the lanternjaw's mouth eats carrion; reserve cap 1.0.
7. **Rest** (§2.8): satiated rest on WX4's rest state (a reason and an upkeep factor).
8. **Egg bank and introduction** (§2.9, §2.13): buried clutches, dormancy with paid
   upkeep and viability loss, the hatching trigger (footfall count, per-egg threshold,
   dusk, adult suppression; Wrysk's choice, behind one function), founding clutches in prey components at world
   creation. Development controls: `--introduce lanternjaw:N` (adults) and a viewer key
   that buries a clutch. Refuse and say why when no prey component exists.
9. **Heuristics.**
   - Lanternjaw ("ambush teacher"): when hungry, walk to a waiting spot beside prey food
     (a crown the grazers feed on, a litter patch at night) preferring cover; stop; lure
     lit; `feed` when a prey is inside the strike capsule; after a kill, eat; rest when
     full; eat carrion it finds.
   - Frondgrazer and littershredder: flee a seen threat within 3 m (grazer) or tremor
     above a threshold (shredder), then resume.
10. **Census and ledger:** columns per lineage for strikes, hits, kills by prey lineage,
    wounds healed, deaths `killed`, `resting`, carrion stock and carrion eaten by eater.
    The ledger stays closed.
11. **Presenter, interim** (art direction: stage-1 looks, no taste calls): the lanternjaw
    as a box model of its ladder size (its body plan is open), windup/lunge as a pose
    offset, a lit lure as one emissive voxel at the tail tip, the carcass as the dead
    animal's model lying flat and darkened, remains below 30 % as a small pale glyph,
    wounded as the starving tint at half strength.

### Rules that must hold

- Conservation: every strike, wound, heal, kill and bite closes the organic, mineral
  and energy books.
- Paid reproduction; no spontaneous appearance (introduction is a user or config act).
- No change to the frondgrazer's or littershredder's v2 manifests, so trained centres
  still load; their behaviour changes only through the world (predators, carcasses).
- FxHash and dense arrays on the sim path; no cross-animal writes in the parallel act.
- Fresh worlds only (schema bump; refuse old snapshots).

### Tests first (a separate commit by a separate worker, ≤ 200 ticks, no pinned hashes)

1. A sleeping adult grazer in the capsule is hit; wound = 0.37; the third hit kills.
2. A juvenile grazer and an adult shredder die in one hit.
3. A prey outside the capsule is never struck; a lanternjaw is never struck by a
   lanternjaw (not in the prey list); a prey over the size window is never struck.
4. The strike cost is paid on a miss; recovery blocks a second strike for 3 s.
5. A prey that walks out of the capsule during the windup is missed by geometry.
6. Dodge: awake walking prey facing the striker dodges at the table's rate over many
   seeded draws; asleep prey never dodges.
7. A kill departs `Killed`, deposits the whole body on the prey's face, and the books
   close. Two attackers' wounds add and the prey dies once.
8. A wound heals at the set rate, paying reserve; a starving animal doesn't heal; the
   speed cap follows the wound.
9. Carrion decays at the new rate; mineral goes to the soil as before.
10. A full lanternjaw rests at the torpor factor and wakes at half its cap.
11. `kind_of`: prey, threat, other across the three lineages; a still threat beyond
    0.5 m reads as other; tremor rises for a walking body and not for a still one.
12. Bodies with volume: a ray at the height of a 0.45 m lanternjaw's back hits it.
13. The introduction places N lanternjaws only in a prey component, and nothing but
    a hatching or the controls ever adds one.
15. A laid egg is invisible to the cone and inedible; it pays its upkeep and loses
    viability; a dead egg goes to litter; the books close.
16. An egg hatches at dusk once its footfall count reaches its threshold, not before,
    not by day, and not with an adult lanternjaw within 4 m.
14. Snapshot round trip with wounds, rest and the carcass record.

### Measure

Census, heuristic controllers (`--founder-heuristic`), presets small, default and the
desktop terrarium, seeds 1–8, 8 h (four days on the clock, so a bank can turn over). Arms: control (no
lanternjaws), and a founding egg bank (the default).

Read off:
- when the first lanternjaws hatch, how many eggs hatch or die, lanternjaws alive at
  4 h, eggs laid, the share of their time resting;
- kills per hour by prey lineage, strikes per kill, dodges;
- prey alive at 2, 4 and 8 h against the control;
- carcasses: lifetime, and who ate them (lanternjaw, shredder, decay);
- the step's cost at the largest population.

Plus one **MP4** (x264 yuv444p crf 12, real time, headless capture): a lanternjaw
waiting, a strike, and the carcass being eaten.

Don't tune. If lanternjaws starve in every world or wipe out the grazers in every
world, report the mechanism and the lever.

### Return (≤ 40 lines)

Commits; numbers you had to choose and why; the table above as medians with ranges;
the MP4's path; anything that needs Wrysk.

**Worker:** tests `opus-high`; implementation `opus-high` (a model-rule change with a
concurrent event phase); the census arms and the MP4 `sonnet-medium`, from the
implementer's finished branch. CCD1 (`taskset -c 8-15,24-31`, cargo `-j 16`), `nice -n 10`.
Explicit-path commits with the trailer lines you are given. No merge to main. No
windows: headless capture only. Don't edit `design/handoffs/README.md`,
`design/README.md`, `config/tachyon/*`, `art/gen/*`. Run the crates you touched; the
integrator runs the full suite at merge.

---

## Brief P2: sensing contract v3 and the trained lanternjaw

**Objective.** One manifest change for every lineage (so centres are refused once),
then a trained lanternjaw and prey that have learned about it.

**After** P1 on main and live-train on main.

### Contract v3 (§2.11)

- All lineages: body kinds `prey / threat / other` per sector (fraction, proximity) in
  place of `body`; a glow channel (fraction, proximity) if Wrysk says yes to §0.3;
  `wound` and `daylight` in Self.
- Blind: the tremor module (left, right).
- Lanternjaw: the Browser's layout with the v3 cone, a carrion chemical field, and the
  `lure` action (4 actions).
- Manifest schema 3; digests move; the host falls back to heuristics until centres
  exist, as today.

### Training

1. Clone each lineage's teacher (P1's heuristics) onto v3; transfer the frondgrazer's
   and shredder's round-2 weights where the layout allows.
2. **Round A:** the lanternjaw alone trains in live worlds (small still-water training
   worlds, lanternjaws introduced at minute 60), against heuristic prey.
3. **Round B:** all three lineages train together.
4. Gate as round 2 did: heuristic vs candidate, seeds 1–8 × small/default/terrarium,
   60 min, plus two 6 h censuses; add kills per hour, prey alive, and predator
   persistence to the table.

### Tests first

- Each v3 channel against a hand-built scene (a prey at 1 m ahead reads prey in the
  centre sector; a still threat at 2 m reads other; tremor left/right).
- Every lineage's manifest digest differs from v2, and a v2 centre is refused.

**Worker:** tests `opus-high`; contract `opus-high`; training runs `opus-medium` once
the contract is in. Return ≤ 40 lines: commits, the gate table, the 6 h readout,
what ships (Wrysk's call).
