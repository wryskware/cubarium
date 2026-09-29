---
status: decisions made by Wrysk 2026-09-28; awaiting his go to dispatch (after P1 merges)
date: 2026-09-28
owner: Fable (organism line)
design: design/predation-and-flight-2026-09-28.md
---

# Flight: F1 substrate with the bellwing, F2 its brain, F3 the seedporter

The design note is the spec (§3). F1 starts after P1 merges: both change the animal
record, the step and the snapshot, and one worker per crate at a time.

---

## Brief F1: free movement in y, perching, nectar, the bellwing

**Objective.** Bellwings that fly within their band, hover at flowers and sip, perch
on crowns, vine faces and undersides, sleep at night and shelter from downpours, in a
live world, driven by a heuristic.

**Worktree** `.claude/worktrees/flight`, branch `flight` off main.
**Crates:** `cubarium-voxel-fauna` (pose `y_m`, `Mobility` modes, air motion, perches,
flyer fan), `cubarium-voxel-flora` (bloomcrown nectar; latticevine `take_nectar`
unchanged), `cubarium-voxel` (air queries if the world lacks one), `cubarium-voxel-sim`
(founding and heuristics), `cubarium` (census, interim poses).

### What to build

1. **Modes and position** (§3.1–3.2): `y_m` on the pose, `Mobility` modes `Fly`,
   `Glide`, `Climb`, `Perched` beside today's. Build `Fly` and `Perched` fully;
   `Glide` and `Climb` as types with `todo!()` bodies and a note for F3.
2. **Air motion** (§3.2–3.3): sphere collision against terrain and water; the
   ceiling over the support face below; half speed inside foliage layers (crown cache);
   `lift` action; costs airborne, hover, lift work, takeoff.
3. **Perches** (§3.4): support-face tops, crown tops, latticevine faces, crown and
   overhang undersides. An anchor on the animal; losing it means takeoff or a drop.
4. **Flyer fan and Self inputs** (§3.6): the downward fan, `altitude`, `ceiling`, and
   the `bloom` class. The bellwing's manifest is new, so no existing digest moves.
5. **Nectar** (§3.7): bloomcrown `nectar` stock on adult stands, paid from reserve in
   daylight up to a cap; latticevine nectar through `take_nectar`. A nectar food class
   with `nectar_mineral_fraction`. Sip = `feed` with a bloom inside the mouth reach while
   hovering or perched on it. Seam: the body records the last bloom visited.
6. **Weather** (§3.8): shower cost factor; downpour forces landing, preferring a perch
   with cover overhead; no takeoff in a downpour. Bellwing `Diurnal`, sleeping perched.
7. **Lineage.** `Founder::Bellwing`, physiology from §3.9. Founding: seeded at world
   start only where the startup acceptance finds nectar sources in reach, plus
   `--introduce bellwing:N`.
8. **Heuristic ("hover-sip teacher"):** fly to the nearest visible bloom in range;
   hover and sip until it is empty or the reserve is full; then perch (undersides and
   vine faces first) and rest; with nothing in sight, a correlated wander within the
   band; land sheltered in a downpour; sleep perched at night.
9. **Census:** bellwings alive, births, deaths by cause, share of time airborne /
   perched / asleep, sips per hour by source (latticevine, bloomcrown), perch kinds used,
   sheltered share during downpours.
10. **Presenter, interim:** no bellwing model is baked yet. Bake the BW-A blockout
    (`runs/animal-bodies-2026-09-23/`, final body, original colours) through the
    package V pipeline (`scripts/blender/bake_voxel_models.py`, `source.json`); if that
    needs more than adding a source entry, draw an interim glyph and say so. Draw it at
    `y_m`, a two-frame vane beat when flying, folded when perched, hanging under an
    underside. Every move on the every-frame clock (nothing pops).

### Rules that must hold

- Conservation through flight costs, sips and nectar production.
- A flyer never occupies a solid or water cell and never exceeds its ceiling.
- Walkers are unchanged: their pose `y_m` is their support face, and their manifests
  and digests do not move.
- Paid reproduction; no spontaneous appearance.

### Tests first (separate worker, ≤ 200 ticks)

1. A bellwing with no support stays aloft while paying the airborne cost; a walker in
   the same spot is refused.
2. It cannot enter rock or water, and cannot climb past the ceiling.
3. Hover costs more per second than cruise, which costs more than perching; lift work
   is paid going up and not going down.
4. It lands on a ground face, a bloomcrown crown top, a latticevine face and under a
   frond's crown slab; each perch holds it at no airborne cost.
5. Its perch crown is eaten away: it takes off (or drops when it can't pay).
6. A sip takes latticevine nectar through `take_nectar` and bloomcrown nectar from the
   new stock; the books close; an empty flower gives nothing.
7. Bloomcrown nectar refills in daylight up to its cap and not at night.
8. In a downpour a flyer aloft descends and lands, and a perched one can't take off.
9. Inside foliage layers its speed cap halves.
10. The flyer fan sees a flower 3 m below-ahead; `altitude` and `ceiling` read true on a
    hand-built column.
11. Snapshot round trip with a flying and a perched bellwing.

### Measure

Census, heuristic controllers, presets small, default and the desktop terrarium,
seeds 1–8, 4 h. Arms: no bellwings, and bellwings seeded at founding.

Read off: bellwings alive at 2 h and 4 h, births, intake against flight cost per
body-hour, the census shares above, bloomcrown seed output against the control (does
nectar production cost the plant visibly?), and the step's cost. Plus one **MP4**:
bellwings working a flowering vine and sheltering in a downpour.

Don't tune; report the mechanism and the lever if bellwings starve everywhere or boom.

### Return (≤ 40 lines)

Commits, chosen numbers and why, the table as medians with ranges, the MP4's path,
anything that needs Wrysk.

**Worker:** tests `opus-high`; implementation `opus-high`. Same constraints as P1
(CCD1, `nice -n 10`, explicit-path commits, no merge, no windows, protected files).

---

## Brief F2 (outline): the bellwing's brain

Clone the hover-sip teacher onto the bellwing's manifest (4 actions: `forward`, `turn`,
`feed`, `lift`), then join the next live-train round as a third or fourth lineage.
Gate: bellwings alive and moving share against the heuristic, seeds 1–8 × three presets.
Written in full when F1 lands.

## Brief F3 (outline): the seedporter

- **Climb** on plant stems (stands ≥ 1 m tall) and latticevine faces, on the
  shredder's wall machinery with a `ClimbSurface` kind (§3.5).
- **Glide** from ≥ 1 m at ratio 2.5, speed 3 BL/s, steered; lands on the first support,
  crown or covered face.
- **Fruit:** latticevine fruit through `take_fruit`; a lanternberry fruit product (paid
  from reserve, hanging stock, replacing "fruit = parcel").
- **Seams:** seeds ride in the gut and drop with the dung at the next grove.
- **Predation:** a grounded seedporter joins the lanternjaw's prey list.
- Body SP-AB2 (final). Written in full when F1 lands.
