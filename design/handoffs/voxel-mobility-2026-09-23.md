# Mobility: shredders climb walls, browsers take real ledges, wading and drowning by body size

Wrysk (2026-09-23) approved this. Shredders "are basically centipedes, so they should
probably be able to climb vertical surfaces (terrain, not stands)". Browsers should "go
up or down reasonable gaps" and "wade in a reasonable amount of water". Branch `mobility`
is `retrain` 169ae89 with main 30d72e9 merged in (713558a); it lands on main with
`retrain`.

## Today

| | Size | Climb | Wade | Drown |
|---|---|---|---|---|
| Shredder | 0.375 × 0.125 × 0.125 m | 0.125 m up or down | 0.05 m | instant at 0.2 m |
| Browser | 0.75 × 0.375 × 0.375 m | 0.25 m up or down | 0.05 m | instant at 0.2 m |

- The shredder's climb is `FounderPhysiology::climb_m` in `body.rs`. The water numbers
  are `wade_depth_m` / `drown_depth_m` in `SpeciesConfig`, and death is in `deaths()` in
  `step.rs`.
- The water numbers predate package L's body sizes. A shower that lifts a puddle past
  0.2 m kills a 0.375 m browser on that tick, which likely explains P5-C's drownings.

## Decisions

1. **Shredders climb vertical terrain.**
   - A shredder whose advance meets a solid terrain riser taller than its step attaches
     to the face. It moves along the face vertically at half its walking pace (body
     lengths per second), with the motor cost per second × a climbing factor that you
     choose and justify (lifting its weight).
   - At the top edge it steps onto the top face when there is headroom.
   - Walking off an edge deeper than its step down, it climbs down the face instead of
     refusing.
   - An overhang stops the climb and the body turns back. Ceilings are a later package.
   - Stands are not climbable.
   - While climbing it does not feed. Its senses read from where it is, with its heading
     into the face.
   - A climbing state on the body, with fauna schema bumped.
   - Walkable components and route maps join terrain the shredder can climb, for
     habitat acceptance and arena placement.
2. **Browsers step up 0.375 m (own height) and down 0.75 m (body length).**
   - Split the climb into up and down limits.
   - Where code needs undirected connectivity (components, acceptance), use mutual
     reachability and say so.
3. **Wading and drowning scale to the body.** Thresholds are fractions of the
   **current** body height, so juveniles scale:
   - Wade: browser 0.5 × height (0.19 m adult); shredder 0.25 × height (~3 cm).
   - Drown: water deeper than the body's own height, continuously for ≥ 60 s (a
     per-animal counter that resets when not over).
   - Escape: a body standing deeper than it can wade may step to any face shallower than
     its own.
   - Clutches keep their rule.
4. **Keep the contract v2 observation and action layout.** The next training round trains
   on these rules. Keep the arena on the same rules; the arena and trainer tests must pass.
5. **Presenter:** a climbing shredder's model is drawn pitched flat against the face,
   head up when climbing and down when descending. This is an **interim** look (art
   direction: stage-1 voxel models; no taste calls beyond that).

## Tests first (separate commit, tiny)

1. A shredder against a 4-voxel wall reaches the top face in about `height / (0.5 · pace)`
   and pays the climbing factor.
2. It climbs down a cliff instead of refusing.
3. An overhang stops it.
4. A stand is never climbed.
5. A browser steps up 0.375 m but not 0.5 m, and steps down 0.75 m but not 1.0 m.
6. Wade depths scale with the current body height (juvenile vs adult).
7. A body submerged past its height survives 59 s, drowns at ≥ 60 s, and the counter
   resets on leaving.
8. A body stuck in water deeper than its wade depth can step to a shallower face.
9. The shredder's walkable components join a wall's foot and top; the browser's use
   mutual reachability.

## Measure

- Coupled census, default seeds 1–4 and the terrarium seed 1, 2 h each, before and after.
- Read off: deaths by cause (drownings especially), climbs and descents per animal (a
  counter), the share of shredder time on walls, and browser ledge use.
- Use `voxel_founder_autopsy` drowning lines where useful.

## Return (≤ 40 lines)

Commits, the numbers chosen (climbing factor), before/after drownings and movement use,
anything that needs a decision.
