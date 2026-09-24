# Brief: latticevine face-cover simulation (flora crate) (2026-09-23)

Approved by Wrysk 2026-09-23 ("yeah go ahead and dispatch it"). The spec is
`design/art-direction/species-dossier-D15-latticevine-2026-09-23.md`, section
**Revision 2** (everything under it, drought tolerance included). Read it in full.
Where this brief and the dossier disagree, ask; don't pick.

Two workers in one worktree, one after the other:

1. **Test author**: writes the behaviour tests below against a minimal public API
   sketch. The tests compile against stubs (`todo!()` bodies are fine) and fail. No
   implementation.
2. **Implementer**: makes them pass without weakening them. If a test is wrong, the
   implementer says so in its report and changes that test only with a stated reason.

## Scope

In: a new module in `crates/cubarium-voxel-flora` (e.g. `src/cover.rs`) holding the
**vines** (plants) and their **covered faces**, stepped inside `Flora::step`, with its
own config section in `FloraConfig`, snapshot and ledger lines, founders at world
creation, and read access through `FloraView` for a later presenter package.

Out:
- The presenter tile layer: it comes after the voxel texture package. Don't touch
  `crates/cubarium/src/voxel/` beyond whatever is strictly needed to compile.
- Fauna: climbing, roosting, feeding and gut dispersal. Provide the flora-side API only
  (`take_nectar`, `take_fruit`, `crop`), with no fauna callers.
- The CPU renderer. Tuning passes; any defaults you choose, state them.
- Seedling establishment from fallen fruit: fallen beads become litter for now.
  Establishment arrives with gut dispersal.

## Model (from the dossier, made concrete)

- **Face address:** a solid terrain voxel plus an outward direction. Allowed directions
  are the four horizontal sides and down (underside); up is the ordinary stand world.
  A face is **eligible** when its voxel is solid terrain and the voxel beyond the face is
  air (not water, not solid). Any solid terrain material is eligible; ivy climbs soil
  walls too. Say in the report if a material should be excluded.
- **Vine (plant):** root site (a soil support face, i.e. a stand-style `Site`), water
  and reserve stocks, a dormancy state with its dry and wet spell timers, age, and a
  lineage id (sisters share it). **Covered face:** owner vine, leafiness 0–1, rooted
  flag, spur phase and phase timer, nectar and fruit stocks.
- **Water:** the root drinks through the **existing drink path** (root box, available
  water, split between competing drinkers), so a vine and a stand at the same site
  compete for the same soil water and the world's water stays conserved. Transpiration
  scales with total leafiness.
- **Light per face:** sky visibility of the air voxel in front of the face (reuse the
  sky cache) times an orientation factor: sides 1, underside small (configurable, e.g.
  0.15). Shading by stand crowns may be skipped in v1; say if you skip it.
- **Growth and spread:** each face earns from light × leafiness, limited by the vine's
  water, and pays upkeep (leaf plus woody). Reserve buys: regrowth of leafiness, spread
  onto an adjacent **eligible unowned** face (up, down, sideways, or around an edge onto
  the next face), and sister rooting. **No size cap.** The only limits are water, light,
  upkeep and cropping.
- **Hanging:** a vine rooted on a ledge or overhang top starts on the face below the
  lip and spreads down and onto the underside.
- **Sister rooting:** a covered face adjacent to a soil support face with no vine root
  can pay to root a sister there. The sister is a new vine with the same lineage id.
  It takes ownership of the faces nearer its root than the parent's (path distance
  along owned cover). It has its own stocks and shares no resources after rooting.
- **Lateral competition (the dossier's rule):** never spread onto an owned face.
  A face whose leafiness is below the contest threshold is contestable. Among its owner
  and the owners of adjacent faces, the one with the most reserve per covered face takes
  it; the owner keeps it on a tie or when ahead. Evaluate only when a face crosses below
  the threshold, or on a slow check wheel (like the seed bank's). Never scan the whole
  cover every tick.
- **Drought:** a sustained dry spell at the root (moisture below the dry threshold for
  a configured duration) means dormancy. Leafiness falls to 0 over a short fall, and the
  lost leaf mass is deposited as **litter** on the ground below. Runners stay, spurs
  stop cycling, and the woody upkeep continues. A sustained wet spell (above the wet
  threshold, which is higher than the dry one) means regreening from the root outward,
  paid face by face. When a dormant vine can't pay the woody upkeep, it loses its
  **outermost** faces first (greatest path distance from the root). A dropped face
  becomes unowned bare rock.
- **Spurs:** one per covered face. A cycle starts (bare → bud → flower → fruit → spent →
  bare) when the vine has reserve above a threshold and rain stopped recently (reuse
  `was_raining` / the shower-end signal), with a per-face jittered delay. Nothing
  cycles while dormant. Flower holds **nectar** and fruit holds **fruit** (the bead
  mass), both paid from reserve and both **depletable**. Unconsumed fruit at spent
  falls to litter. Beads already in fruit when dormancy begins stay until eaten or spent.
- **Terrain change:** a face whose voxel stops being solid, or whose front voxel stops
  being air, is removed (like `prune_unsupported`). A vine whose root site becomes
  unsupported dies, and its faces become unowned.
- **Founders:** config-driven founder vines at world creation, on eligible foot-of-wall
  and ledge-top sites. The desktop terrarium and the panel config should get some. Use
  `config/desktop/terrarium.toml` and `config/tachyon/voxel.toml` as the smoke worlds.
- **Accounting:** every flow in the ledger. Mass conservation holds: carbon into
  reserve, leaves and wood, out as litter, nectar and fruit taken, upkeep. Water is
  conserved through the drink path.
- **Schema:** bump the flora snapshot `SCHEMA`, plus any composite world schema that
  embeds it. Old worlds are refused, never migrated (standing rule).
- **Performance:** dense arrays or FxHash for anything keyed by face (standing rule:
  `rustc-hash` on the sim path). Report the flora step time on the desktop terrarium
  before and after, with a normal founder count.

## Behaviour tests (the test author writes these; each short and focused)

1. A founder at a wall foot covers its first face and, with water and light, spreads
   up. On a tall wall with ample water it keeps extending with no plateau from any cap,
   stopping only at the top of the wall.
2. A vine rooted on a ledge top covers the face below the lip and spreads downward, and
   onto an overhang's underside. Underside faces earn less than side faces with the
   same sky.
3. Spread never takes a healthy face owned by another vine, sister or stranger.
4. Contest: a face below the threshold goes to the adjacent owner with more reserve per
   face. The owner keeps it when it's ahead or tied.
5. Sister rooting: cover reaching a free soil support face roots a sister with the same
   lineage. Faces nearer the sister switch owner, and afterwards the two vines' stocks
   evolve independently.
6. Drought: a sustained dry spell brings dormancy, and leaf mass moves to litter
   (conserved). Runners remain, and faces keep their owner.
7. Hysteresis: one short wet pulse during dormancy doesn't regreen. A sustained wet
   spell regreens, root-near faces first.
8. A starving dormant vine loses its outermost faces first, and those faces become
   unowned.
9. Spurs cycle only with reserve and after rain, and not all on the same tick (phases
   scatter). None cycle while dormant.
10. Nectar exists only on faces in flower and fruit only in fruit. `take_nectar` and
    `take_fruit` deplete them and are counted in the ledger. Uneaten fruit at spent
    becomes litter.
11. A vine and a stand drinking from the same site both get water, and the total water
    is conserved.
12. `crop(face, want)` thins leafiness and returns the mass taken (for grazers later).
13. Terrain edits: removing a face's voxel, or filling its front with solid or water,
    removes the face. Removing the root's support kills the vine.
14. Snapshot round trip preserves vines and faces. An old-schema snapshot is refused.
15. `FloraView` exposes each covered face's owner, leafiness, rooted flag, dormant flag
    and spur phase: what the presenter will draw from.

Tests are short function tests on hand-built small worlds (see the existing tests in
`lib.rs`/`step.rs` for fixtures). No long simulations, no pinned hashes, no byte-identical
checks (standing rules).

## Standing rules

Work only in the worktree. Commit with explicit paths. Don't merge, deploy or touch the
panel. At the end, run `cargo test -p cubarium-voxel-flora`, plus `-p cubarium` if the
schema bump touches it. Keep cargo at a sensible `-j` (the desktop is capped at about 50 %
for agents).

## Returns

- **Test author (≤ 20 lines):** worktree path and branch, the API sketch (signatures),
  and the test list with file:line, all compiling and failing.
- **Implementer (≤ 30 lines):** commits; each test's status; any test changed and why;
  the defaults chosen (thresholds, costs, orientation factor); flora step time before and
  after; a 10-minute smoke run on the terrarium config (vines alive, faces covered,
  dormancy seen or not); anything the dossier left open that you had to decide.
