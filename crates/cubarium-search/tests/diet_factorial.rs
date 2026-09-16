//! Definition tests for the controlled form × diet factorial
//! (`design/handoffs/ecology-v1-diet-factorial-opus-2026-09-16.md`, deliverables 2–4).
//!
//! Workstream F found that skimmer-rigged bodies carrying a foliage-end diet survive at 84 %
//! where the founder's middling `diet = 0.60` survives at 20 %, and said plainly that this is
//! an **association**: every foliage-diet skimmer in those rows is a descendant, so the
//! comparison mixes later birth, right-censoring, selection into a mutant lineage and
//! mutation at other loci. The factorial exists to remove exactly those. Its whole value
//! rests on the design being what it claims, so the design is tested before it is run:
//!
//! 1. **The three arms stand in the same eight cells.** If the arms differ in where they
//!    start, arm-to-arm differences are habitat differences.
//! 2. **Arm A changes `diet` and nothing else.** Every other locus of the roster skimmer —
//!    `form`, `depth`, `speed`, `size`, `metabolism`, `swim`, `hue`, `sense`, every drive —
//!    is identical between the 0.60 and 0.85 halves. That is the entire point of the arm.
//! 3. **Arm B holds `diet` at 0.85 and varies the body**, and arm C is the roster pairing.
//!    Two cells of the factorial appear in more than one arm (the grazer is already at 0.85,
//!    and the arm-A high half is arm B's skimmer); those are free internal controls and are
//!    asserted to be identical rather than left to chance.
//! 4. **The placement is neutral**: the eight cells come from fixed anchors spread over all
//!    five faces, not from ranking the landscape by food or by water. Selecting on either
//!    would hand one diet the better ground.
//! 5. **Nothing reproduces and nothing mutates among the clones**, and **the ledger closes**
//!    for every one of them, or the yields reported from it mean nothing.
//! 6. **A row reproduces**: the same arm at the same seed, run twice, is the same row.

use std::collections::BTreeSet;

use cubarium_core::WorldConfig;
use cubarium_search::factorial::{
    self, Arm, CLONES, DIET_HIGH, DIET_LOW, Design, Landscape, Placement, Roster,
};
use cubarium_surface::{CUBE_CELL_COUNT, CellId, Face, Scale, Topology};

/// A design small enough to run inside a test: long enough that bodies feed, move, are billed
/// and — for the ordinary founders, whose `bud_min_age_seconds` is 120 — breed at least once,
/// far short of the campaign's horizon.
fn tiny() -> Design {
    Design {
        ticks: 4_000,
        warm_up_ticks: 600,
        probe_every: 20,
        drain_every: 200,
        wet_min: 0.05,
        pursuit_stop: cubarium_core::hunter::PursuitStop::default(),
    }
}

fn fast_leaf(seed: u64) -> WorldConfig {
    cubarium_search::calibrate::candidate("fast-leaf")
        .expect("fast-leaf is a declared candidate")
        .config(seed)
        .expect("and it builds a config")
}

/// A synthetic landscape: everything alive, a handful of cells holding water, and one face
/// where the cell under an anchor is dead so the outward search has to move. Pure, so the
/// placement rule is tested without running a world.
fn synthetic() -> Landscape {
    let mut land = Landscape {
        depth: vec![0.0; CUBE_CELL_COUNT],
        foliage: vec![0.2; CUBE_CELL_COUNT],
        litter: vec![0.1; CUBE_CELL_COUNT],
    };
    for cell in [
        CellId::new(Topology::Cube, Scale::ONE, Face::Front, 4, 4),
        CellId::new(Topology::Cube, Scale::ONE, Face::Front, 5, 4),
        CellId::new(Topology::Cube, Scale::ONE, Face::Front, 4, 5),
    ] {
        land.foliage[cell.index()] = 0.0; // the Front (4,4) anchor's own cell, and its ring 1
    }
    for cell in [CellId::new(Topology::Cube, Scale::ONE, Face::Right, 8, 5), CellId::new(Topology::Cube, Scale::ONE, Face::Back, 12, 11)] {
        land.depth[cell.index()] = 0.4;
    }
    land
}

fn placements() -> [Placement; CLONES] {
    factorial::choose_cells(&synthetic(), 0.05).expect("the synthetic landscape places")
}

fn roster(seed: u64) -> Roster {
    let world = cubarium_core::World::new(fast_leaf(seed)).expect("fast-leaf builds a world");
    Roster::of(&world).expect("the default roster carries all four kinds")
}

// --- 1. the same eight cells in every arm -----------------------------------------------

/// The counterbalanced arm is arm A's exact complement as *planned output*: slot by slot the
/// same cell and class, the same skimmer body and every other locus equal, and the diet in
/// each slot is the other of the two constants — so a future change to `plan` that broke the
/// pairing would fail here, not only in the slot arithmetic.
#[test]
fn the_swapped_arm_is_arm_a_with_only_each_slots_diet_exchanged() {
    let r = roster(1);
    let cells = placements();
    let a = factorial::plan(Arm::A, &r, &cells);
    let s = factorial::plan(Arm::ASwap, &r, &cells);
    assert_eq!(a.len(), CLONES);
    assert_eq!(s.len(), CLONES);
    let mut diets = std::collections::BTreeSet::new();
    for i in 0..CLONES {
        assert_eq!(a[i].slot, s[i].slot, "slot {i}");
        assert_eq!(a[i].cell, s[i].cell, "slot {i}: the same cell");
        assert_eq!(a[i].wet_start, s[i].wet_start, "slot {i}: the same class");
        assert_eq!(a[i].form, s[i].form, "slot {i}: the same body");
        assert_ne!(a[i].genome.diet, s[i].genome.diet, "slot {i}: the diet is exchanged");
        let mut a_rest = a[i].genome.clone();
        let mut s_rest = s[i].genome.clone();
        a_rest.diet = 0.0;
        s_rest.diet = 0.0;
        assert_eq!(a_rest, s_rest, "slot {i}: every other locus equal");
        diets.insert(a[i].genome.diet.to_bits());
        diets.insert(s[i].genome.diet.to_bits());
    }
    assert_eq!(diets.len(), 2, "exactly the two diet constants appear across both arms");
    // Across the two arms each diet stands in every one of the eight cells once.
    for i in 0..CLONES {
        let low_in_a = a[i].genome.diet < s[i].genome.diet;
        let low_in_s = s[i].genome.diet < a[i].genome.diet;
        assert!(low_in_a != low_in_s, "slot {i}: the low diet is in exactly one arm");
    }
}

#[test]
fn the_three_arms_stand_in_the_same_eight_cells_in_the_same_order() {
    let r = roster(1);
    let cells = placements();
    let a = factorial::plan(Arm::A, &r, &cells);
    let b = factorial::plan(Arm::B, &r, &cells);
    let c = factorial::plan(Arm::C, &r, &cells);
    for arm in [&a, &b, &c] {
        assert_eq!(arm.len(), CLONES, "every arm places eight clones");
    }
    for i in 0..CLONES {
        assert_eq!(a[i].cell, b[i].cell, "slot {i}: A and B stand in the same cell");
        assert_eq!(a[i].cell, c[i].cell, "slot {i}: A and C stand in the same cell");
        assert_eq!(a[i].wet_start, b[i].wet_start, "slot {i}: and the same class");
        assert_eq!(a[i].wet_start, c[i].wet_start, "slot {i}: and the same class");
        assert_eq!(a[i].slot, i, "slots are the factorial's own index");
    }
}

#[test]
fn the_eight_cells_are_distinct_separated_alive_and_spread_over_every_face() {
    let land = synthetic();
    let cells = placements();
    let distinct: BTreeSet<u16> = cells.iter().map(|p| p.cell).collect();
    assert_eq!(distinct.len(), CLONES, "eight distinct cells");
    let faces: BTreeSet<Face> = cells.iter().map(|p| CellId(p.cell).face(Topology::Cube, Scale::ONE)).collect();
    assert_eq!(faces.len(), 5, "all five faces carry a clone");
    for (i, p) in cells.iter().enumerate() {
        assert!(land.foliage[CellId(p.cell).index()] > 0.0, "slot {i}: placed in a living cell");
        assert_eq!(
            CellId(p.cell).face(Topology::Cube, Scale::ONE),
            CellId(p.anchor).face(Topology::Cube, Scale::ONE),
            "slot {i}: a cell is found on its anchor's own face"
        );
    }
    for i in 0..CLONES {
        for j in (i + 1)..CLONES {
            let (a, b) = (CellId(cells[i].cell), CellId(cells[j].cell));
            if a.face(Topology::Cube, Scale::ONE) == b.face(Topology::Cube, Scale::ONE) {
                let d = (i32::from(a.cx(Topology::Cube, Scale::ONE)) - i32::from(b.cx(Topology::Cube, Scale::ONE)))
                    .abs()
                    .max((i32::from(a.cy(Topology::Cube, Scale::ONE)) - i32::from(b.cy(Topology::Cube, Scale::ONE))).abs());
                assert!(d >= 3, "slots {i} and {j} are not on top of each other: {d} cells apart");
            }
        }
    }
}

/// Habitat is measured, not assigned: the two cells the synthetic landscape floods are the
/// two that come back `wet`, and nothing about the placement selected for them.
#[test]
fn the_habitat_class_is_read_off_the_cell_and_not_chosen() {
    let cells = placements();
    let wet: BTreeSet<u16> = cells.iter().filter(|p| p.wet).map(|p| p.cell).collect();
    let expect: BTreeSet<u16> =
        [CellId::new(Topology::Cube, Scale::ONE, Face::Right, 8, 5).0, CellId::new(Topology::Cube, Scale::ONE, Face::Back, 12, 11).0].into();
    assert_eq!(wet, expect, "exactly the flooded cells read wet");
    // And the threshold is the one it was given, not a constant hidden in the rule.
    let none = factorial::choose_cells(&synthetic(), 10.0).expect("places");
    assert!(none.iter().all(|p| !p.wet), "a higher floor leaves nothing wet");
}

/// An anchor whose own cell is dead steps outward to the nearest living one rather than
/// placing a clone where nothing grows.
#[test]
fn a_dead_anchor_cell_steps_outward_to_the_nearest_living_one() {
    let cells = placements();
    let front = CellId(cells[0].cell);
    assert_eq!(CellId(cells[0].anchor), CellId::new(Topology::Cube, Scale::ONE, Face::Front, 4, 4), "the anchor is fixed");
    assert_ne!(front, CellId::new(Topology::Cube, Scale::ONE, Face::Front, 4, 4), "but its own cell is dead");
    let d = (i32::from(front.cx(Topology::Cube, Scale::ONE)) - 4).abs().max((i32::from(front.cy(Topology::Cube, Scale::ONE)) - 4).abs());
    assert_eq!(d, 1, "and the nearest living cell is one ring out");
}

#[test]
fn a_face_with_nothing_growing_on_it_is_refused_rather_than_placed_anyway() {
    let mut land = synthetic();
    for cx in 0..16u16 {
        for cy in 0..16u16 {
            land.foliage[CellId::new(Topology::Cube, Scale::ONE, Face::Top, cx, cy).index()] = 0.0;
        }
    }
    let err = factorial::choose_cells(&land, 0.05)
        .expect_err("a face with no living cell cannot host its clone");
    assert!(err.contains("Top"), "the refusal names the face: {err}");
    assert!(err.contains("living"), "and what is missing: {err}");
}

// --- 2. arm A changes only diet ---------------------------------------------------------

#[test]
fn arm_a_is_the_roster_skimmer_with_only_diet_changed() {
    let r = roster(1);
    let cells = placements();
    let a = factorial::plan(Arm::A, &r, &cells);
    let skimmer = r.genome(factorial::SKIMMER).clone();
    for spec in &a {
        assert_eq!(spec.form, factorial::SKIMMER, "arm A is one body");
        let mut as_roster = spec.genome.clone();
        as_roster.diet = skimmer.diet;
        assert_eq!(
            as_roster, skimmer,
            "slot {}: put `diet` back and the genome is the roster skimmer, locus for locus",
            spec.slot
        );
    }
}

#[test]
fn arm_a_is_four_and_four_interleaved_over_the_eight_cells() {
    let r = roster(1);
    let cells = placements();
    let a = factorial::plan(Arm::A, &r, &cells);
    let low: Vec<_> = a.iter().filter(|s| s.genome.diet == DIET_LOW).collect();
    let high: Vec<_> = a.iter().filter(|s| s.genome.diet == DIET_HIGH).collect();
    assert_eq!(low.len(), 4, "four at the founder's 0.60");
    assert_eq!(high.len(), 4, "four at 0.85");
    // Interleaved by location in pairs, so the two halves are spread over the surface rather
    // than one half sitting on one side of the world.
    assert_eq!(
        low.iter().map(|s| s.slot).collect::<Vec<_>>(),
        vec![0, 1, 4, 5],
        "the 0.60 half's slots"
    );
    assert_eq!(
        high.iter().map(|s| s.slot).collect::<Vec<_>>(),
        vec![2, 3, 6, 7],
        "the 0.85 half's slots"
    );
    let faces = |half: &[&factorial::CloneSpec]| {
        half.iter().map(|s| s.cell.face(Topology::Cube, Scale::ONE)).collect::<BTreeSet<_>>().len()
    };
    assert!(faces(&low) >= 3 && faces(&high) >= 3, "each half stands on at least three faces");
}

#[test]
fn the_founder_skimmers_diet_is_the_low_arm_and_it_is_not_the_high_one() {
    let r = roster(1);
    assert_eq!(r.genome(factorial::SKIMMER).diet, DIET_LOW, "0.60 is the roster's own");
    assert_ne!(DIET_LOW, DIET_HIGH);
    assert_eq!(r.genome(factorial::GRAZER).diet, DIET_HIGH, "0.85 is the grazer's own");
}

// --- 3. arm B holds diet, arm C is the roster pairing ------------------------------------

#[test]
fn arm_b_holds_diet_at_the_foliage_end_and_varies_the_body() {
    let r = roster(1);
    let cells = placements();
    let b = factorial::plan(Arm::B, &r, &cells);
    for spec in &b {
        assert_eq!(spec.genome.diet, DIET_HIGH, "slot {}: diet is held", spec.slot);
    }
    for form in factorial::FORMS {
        let of_form: Vec<_> = b.iter().filter(|s| s.form == form).collect();
        assert_eq!(of_form.len(), 2, "form {form}: two of each body");
        assert_ne!(
            of_form[0].cell.face(Topology::Cube, Scale::ONE),
            of_form[1].cell.face(Topology::Cube, Scale::ONE),
            "form {form}: its two clones stand on different faces"
        );
    }
}

#[test]
fn arm_b_changes_only_diet_away_from_each_roster_body() {
    let r = roster(1);
    let cells = placements();
    for spec in factorial::plan(Arm::B, &r, &cells) {
        let mut as_roster = spec.genome.clone();
        as_roster.diet = r.genome(spec.form).diet;
        assert_eq!(&as_roster, r.genome(spec.form), "slot {}: only `diet` moved", spec.slot);
    }
}

#[test]
fn arm_c_is_the_roster_pairing_untouched() {
    let r = roster(1);
    let cells = placements();
    let c = factorial::plan(Arm::C, &r, &cells);
    for spec in &c {
        assert_eq!(&spec.genome, r.genome(spec.form), "slot {}: the roster genome", spec.slot);
    }
    for form in factorial::FORMS {
        let of_form: Vec<_> = c.iter().filter(|s| s.form == form).collect();
        assert_eq!(of_form.len(), 2, "form {form}: two of each");
        assert_ne!(
            of_form[0].cell.face(Topology::Cube, Scale::ONE),
            of_form[1].cell.face(Topology::Cube, Scale::ONE),
            "form {form}: its two clones stand on different faces"
        );
    }
}

/// Two free internal controls: arm B's grazer is arm C's grazer (0.85 is already the roster
/// grazer's diet), and arm A's high half is arm B's skimmer. If either pair diverged, the
/// arms would not be measuring what their names say.
#[test]
fn the_arms_overlap_where_the_design_says_they_must() {
    let r = roster(1);
    let cells = placements();
    let (a, b, c) = (
        factorial::plan(Arm::A, &r, &cells),
        factorial::plan(Arm::B, &r, &cells),
        factorial::plan(Arm::C, &r, &cells),
    );
    for slot in 0..CLONES {
        if b[slot].form == factorial::GRAZER {
            assert_eq!(b[slot].genome, c[slot].genome, "slot {slot}: the grazer is shared");
        }
        if b[slot].form == factorial::SKIMMER {
            assert_eq!(
                b[slot].genome.diet, DIET_HIGH,
                "slot {slot}: arm B's skimmer is arm A's high half"
            );
            assert!(
                a.iter().any(|s| s.genome == b[slot].genome),
                "slot {slot}: and that exact genome appears in arm A"
            );
        }
    }
}

/// The contract's caps, read off the planned genomes: at `diet = 0.85` the detrital gate
/// `θ = 0.2` shuts (`1 − 0.85 = 0.15 < 0.2`), so every arm-B body is a *pure* foliage feeder
/// and "form still matters" cannot be a diet difference in disguise.
#[test]
fn every_arm_b_body_is_a_pure_foliage_feeder() {
    let r = roster(1);
    let cells = placements();
    let cfg = fast_leaf(1);
    for spec in factorial::plan(Arm::B, &r, &cells) {
        let p = cubarium_core::genome::decode(&spec.genome, &cfg.organism);
        assert!(
            (p.cap_foliage - f64::from(DIET_HIGH)).abs() < 1e-12,
            "slot {}: cap_foliage",
            spec.slot
        );
        assert_eq!(p.cap_detrital, 0.0, "slot {}: the detrital gate is shut", spec.slot);
    }
}

// --- 5. nothing reproduces, nothing mutates, and the ledger closes -----------------------

/// Run the design for real, briefly. Every clone is alive-or-closed with a ledger that
/// balances, none of them has budded, and the ordinary 24 founders are still in the world
/// competing for the same food.
#[test]
fn a_short_run_keeps_the_clones_sterile_and_the_ledger_closed() {
    let run = factorial::run_one(Arm::A, 1, tiny()).expect("a short arm A runs");
    assert_eq!(run.clones.len(), CLONES);
    assert_eq!(run.legacy_founders, 24, "burrower 4 + grazer 10 + glider 5 + skimmer 5");
    for row in &run.clones {
        assert_eq!(row.births, 0, "slot {}: a clone never buds", row.slot);
        assert!(
            row.material_residual.abs() < 1e-9,
            "slot {}: the material ledger closes ({})",
            row.slot,
            row.material_residual
        );
        assert!(
            row.energy_residual.abs() < 1e-9,
            "slot {}: the energy ledger closes ({})",
            row.slot,
            row.energy_residual
        );
        assert!(row.billed_ticks > 0, "slot {}: it was billed", row.slot);
        assert!(row.probes > 0, "slot {}: it was seen", row.slot);
    }
    assert!(run.clone_births == 0, "no clone in the arm reproduced at all");
    assert!(run.world_births > 0, "the ordinary founders do still breed around them");
}

/// The measure the verdict rests on: every clone's intake is split by channel and the caps
/// are what the genome says. A `diet = 0.85` clone can be credited from foliage and fruit
/// and *never* from litter or carrion, whatever it served.
#[test]
fn a_shut_gate_credits_nothing_however_much_is_served() {
    let run = factorial::run_one(Arm::A, 1, tiny()).expect("a short arm A runs");
    for row in run.clones.iter().filter(|r| r.diet >= f64::from(DIET_HIGH)) {
        assert_eq!(row.cap_detrital, 0.0, "slot {}: the gate is shut", row.slot);
        for ch in [factorial::LITTER, factorial::CARRION] {
            assert_eq!(row.digestible[ch], 0.0, "slot {}: nothing digestible on {ch}", row.slot);
            assert_eq!(row.reserve_credit[ch], 0.0, "slot {}: and nothing credited", row.slot);
            assert_eq!(row.battery_credit[ch], 0.0, "slot {}: and no charge", row.slot);
        }
    }
}

// --- 6. a row reproduces ----------------------------------------------------------------

#[test]
fn the_same_arm_at_the_same_seed_runs_to_the_same_row() {
    let once = factorial::run_one(Arm::C, 2, tiny()).expect("runs");
    let twice = factorial::run_one(Arm::C, 2, tiny()).expect("runs again");
    assert_eq!(once.final_state_hash, twice.final_state_hash, "the world reproduces");
    assert_eq!(once.clones, twice.clones, "and so does every clone's ledger");
    assert_eq!(once.placements, twice.placements, "and the placement it was given");
}

/// The placement is a property of the seed's own landscape, not of the arm: all three arms
/// at one seed get the same eight cells, and a different seed gets its own.
#[test]
fn placement_follows_the_seed_and_not_the_arm() {
    let a = factorial::run_one(Arm::A, 3, tiny()).expect("runs");
    let b = factorial::run_one(Arm::B, 3, tiny()).expect("runs");
    let other = factorial::run_one(Arm::A, 4, tiny()).expect("runs");
    assert_eq!(a.placements, b.placements, "one seed, one landscape, one placement");
    assert_ne!(a.placements, other.placements, "another seed is another landscape");
}

/// Recording the ledger must not move the world. The same arm run with the ledger off ends
/// on the same state hash as with it on, or every number in the campaign is an artefact of
/// measuring.
#[test]
fn recording_the_ledger_moves_nothing() {
    let on = factorial::run_one(Arm::A, 5, tiny()).expect("runs");
    let off = factorial::run_one_without_ledger(Arm::A, 5, tiny()).expect("runs");
    assert_eq!(on.final_state_hash, off.final_state_hash, "the ledger is inert");
}

// --- 7. the depth x diet factorial (workstream O) ----------------------------------------
//
// Four treatments, `depth` in {0.10, 0.55} crossed with `diet` in {0.60, 0.85}, on the one
// body arm A already showed can be moved a locus at a time. Eight slots cannot hold four
// treatments and still give every treatment every cell, so the assignment is a **4 x 8 Latin
// square over four rows**: row `r` gives slot `s` the treatment `(s mod 4) XOR r`, where bit 0
// of a treatment index is the foliage-end diet and bit 1 is the mid-height depth. The tests
// below pin every property the reading of the result depends on:
//
// - every one of the eight cells holds every one of the four treatments exactly once across
//   the four rows, so neither factor is confounded with founding cell (Astra's P1 on J);
// - every row holds every treatment exactly twice, on two different faces, so no treatment is
//   confined to one side of the world within a run and every run is a balanced mix;
// - rows are slot-by-slot complements one factor at a time: D1/D2 exchange `diet` with `depth`
//   held, D1/D3 exchange `depth` with `diet` held, D1/D4 exchange both;
// - a clone is the roster skimmer with `depth` and `diet` moved and nothing else;
// - `depth` moves the preferred height and no capacity, so a depth effect is a steering
//   effect and cannot be a yield difference in disguise.

use cubarium_search::factorial::{DEPTH_HIGH, DEPTH_LOW, TREATMENTS, treatment_of};

/// The whole point of four rows: the 4 x 8 assignment is a Latin square, so treatment is
/// orthogonal to cell. If this fails, a treatment difference is a habitat difference.
#[test]
fn every_cell_holds_every_treatment_exactly_once_across_the_four_rows() {
    let r = roster(1);
    let cells = placements();
    let plans: Vec<Vec<factorial::CloneSpec>> =
        Arm::DEPTH_ROWS.iter().map(|&a| factorial::plan(a, &r, &cells)).collect();
    for slot in 0..CLONES {
        let mut seen: Vec<(u32, u32)> = plans
            .iter()
            .map(|p| (p[slot].genome.depth.to_bits(), p[slot].genome.diet.to_bits()))
            .collect();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), 4, "slot {slot}: all four treatments stand in this one cell");
        // And they are the four the design declares, not four arbitrary pairs.
        let mut want: Vec<(u32, u32)> =
            TREATMENTS.iter().map(|(dp, dt)| (dp.to_bits(), dt.to_bits())).collect();
        want.sort_unstable();
        assert_eq!(seen, want, "slot {slot}: exactly the declared treatments");
        // The same cell, the same class, the same body in every row.
        for p in &plans {
            assert_eq!(p[slot].cell, plans[0][slot].cell, "slot {slot}: one cell in every row");
            assert_eq!(p[slot].wet_start, plans[0][slot].wet_start, "slot {slot}: one class");
            assert_eq!(p[slot].form, factorial::SKIMMER, "slot {slot}: one body");
        }
    }
}

/// Within a run every treatment stands twice, and on two different faces, so a run is a
/// balanced mix of the four rather than a treatment competing only against itself.
#[test]
fn every_row_holds_every_treatment_twice_on_two_faces() {
    let r = roster(1);
    let cells = placements();
    for arm in Arm::DEPTH_ROWS {
        let plan = factorial::plan(arm, &r, &cells);
        assert_eq!(plan.len(), CLONES);
        for (t, (depth, diet)) in TREATMENTS.iter().enumerate() {
            let of_t: Vec<_> = plan
                .iter()
                .filter(|s| s.genome.depth == *depth && s.genome.diet == *diet)
                .collect();
            assert_eq!(of_t.len(), 2, "{} treatment {t}: twice", arm.label());
            assert_ne!(
                of_t[0].cell.face(Topology::Cube, Scale::ONE),
                of_t[1].cell.face(Topology::Cube, Scale::ONE),
                "{} treatment {t}: on two faces",
                arm.label()
            );
        }
    }
}

/// The counterbalance, stated as a property of `plan`'s output rather than of the slot
/// arithmetic: each pair of rows exchanges exactly one factor (or both) in **every** slot.
#[test]
fn the_rows_are_slot_by_slot_complements_one_factor_at_a_time() {
    let r = roster(1);
    let cells = placements();
    let p = |arm| factorial::plan(arm, &r, &cells);
    let (d1, d2, d3, d4) = (p(Arm::D1), p(Arm::D2), p(Arm::D3), p(Arm::D4));
    for slot in 0..CLONES {
        // D1 <-> D2: diet exchanged, depth held.
        assert_eq!(d1[slot].genome.depth, d2[slot].genome.depth, "slot {slot}: D1/D2 depth held");
        assert_ne!(d1[slot].genome.diet, d2[slot].genome.diet, "slot {slot}: D1/D2 diet swapped");
        // D1 <-> D3: depth exchanged, diet held.
        assert_ne!(d1[slot].genome.depth, d3[slot].genome.depth, "slot {slot}: D1/D3 depth swap");
        assert_eq!(d1[slot].genome.diet, d3[slot].genome.diet, "slot {slot}: D1/D3 diet held");
        // D1 <-> D4: both exchanged.
        assert_ne!(d1[slot].genome.depth, d4[slot].genome.depth, "slot {slot}: D1/D4 depth");
        assert_ne!(d1[slot].genome.diet, d4[slot].genome.diet, "slot {slot}: D1/D4 diet");
        // And D2 <-> D4 is the pure depth swap at the other diet, D3 <-> D4 the pure diet
        // swap at the other depth: both within-cell contrasts exist at both levels.
        assert_ne!(d2[slot].genome.depth, d4[slot].genome.depth, "slot {slot}: D2/D4 depth");
        assert_eq!(d2[slot].genome.diet, d4[slot].genome.diet, "slot {slot}: D2/D4 diet held");
        assert_eq!(d3[slot].genome.depth, d4[slot].genome.depth, "slot {slot}: D3/D4 depth held");
        assert_ne!(d3[slot].genome.diet, d4[slot].genome.diet, "slot {slot}: D3/D4 diet");
    }
}

/// The slot arithmetic the analysis uses to label a row must be the one `plan` used to build
/// it, or every table in the note is mislabelled.
#[test]
fn the_analysis_labels_a_clone_with_the_treatment_plan_gave_it() {
    let r = roster(1);
    let cells = placements();
    for arm in Arm::DEPTH_ROWS {
        let row = arm.depth_row().expect("a depth arm has a row");
        for spec in factorial::plan(arm, &r, &cells) {
            let (depth, diet) = TREATMENTS[treatment_of(row, spec.slot)];
            assert_eq!(spec.genome.depth, depth, "{} slot {}", arm.label(), spec.slot);
            assert_eq!(spec.genome.diet, diet, "{} slot {}", arm.label(), spec.slot);
        }
    }
}

/// Only two loci move. Put `depth` and `diet` back and the genome is the roster skimmer,
/// locus for locus — including `swim`, `speed`, `size`, `metabolism` and every drive, so
/// `w_depth` (the *gain* on the depth term) is held while `h_pref` (its *target*) moves.
#[test]
fn a_depth_arm_is_the_roster_skimmer_with_only_depth_and_diet_changed() {
    let r = roster(1);
    let cells = placements();
    let skimmer = r.genome(factorial::SKIMMER).clone();
    for arm in Arm::DEPTH_ROWS {
        for spec in factorial::plan(arm, &r, &cells) {
            assert_eq!(spec.form, factorial::SKIMMER);
            let mut as_roster = spec.genome.clone();
            as_roster.depth = skimmer.depth;
            as_roster.diet = skimmer.diet;
            assert_eq!(
                as_roster,
                skimmer,
                "{} slot {}: only `depth` and `diet` moved",
                arm.label(),
                spec.slot
            );
            assert_eq!(spec.genome.drives.w_depth, skimmer.drives.w_depth, "the gain is held");
            assert_eq!(spec.genome.swim, skimmer.swim, "swim is held: both levels can wade");
        }
    }
}

/// Both values are roster values, in the genome's declared bounds, and they mean what the
/// note says: 0.10 is `h_pref = -0.8`, the low rim this world's water runs to, and 0.55 is
/// `h_pref = +0.1`, a body with no pull toward the rim.
#[test]
fn the_two_depth_values_are_roster_values_that_straddle_the_equator() {
    let r = roster(1);
    let cfg = fast_leaf(1);
    assert_eq!(r.genome(factorial::SKIMMER).depth, DEPTH_LOW, "0.10 is the skimmer's own");
    assert_eq!(r.genome(factorial::GRAZER).depth, DEPTH_HIGH, "0.55 is the grazer's own");
    assert!((0.0..=1.0).contains(&DEPTH_LOW) && (0.0..=1.0).contains(&DEPTH_HIGH), "in bounds");
    let mut low = r.genome(factorial::SKIMMER).clone();
    let mut high = low.clone();
    low.depth = DEPTH_LOW;
    high.depth = DEPTH_HIGH;
    let (pl, ph) = (
        cubarium_core::genome::decode(&low, &cfg.organism),
        cubarium_core::genome::decode(&high, &cfg.organism),
    );
    // `depth` is an `f32`, so the widened target is exact only to f32 precision.
    assert!((pl.h_pref + 0.8).abs() < 1e-6, "0.10 prefers the rim: {}", pl.h_pref);
    assert!((ph.h_pref - 0.1).abs() < 1e-6, "0.55 prefers just above the equator: {}", ph.h_pref);
    assert!(pl.h_pref < 0.0 && ph.h_pref > 0.0, "the two straddle the equator");
    // And the world really is wetter low down, or "the wet floor" is a name and not a place.
    assert!(cfg.habitat.moisture_height_gain < 0.0, "moisture falls with height");
}

/// `depth` moves the steering target and nothing else a body is paid or billed by: the same
/// caps, the same maximum speed, the same mouth, the same maintenance. A depth effect is
/// therefore where the body went, not what it could digest.
#[test]
fn depth_changes_only_the_preferred_height_and_no_capacity() {
    let r = roster(1);
    let cfg = fast_leaf(1);
    let mut low = r.genome(factorial::SKIMMER).clone();
    low.depth = DEPTH_LOW;
    let mut high = low.clone();
    high.depth = DEPTH_HIGH;
    let (a, b) = (
        cubarium_core::genome::decode(&low, &cfg.organism),
        cubarium_core::genome::decode(&high, &cfg.organism),
    );
    assert_ne!(a.h_pref, b.h_pref, "the preferred height moves");
    let strip = |p: cubarium_core::genome::Phenotype| {
        let mut p = p;
        p.h_pref = 0.0;
        p
    };
    assert_eq!(strip(a), strip(b), "and nothing else in the phenotype does");
}

/// The rows run, the clones stay sterile, the ledgers close, and a row reproduces.
#[test]
fn a_short_depth_run_is_sterile_closed_and_reproducible() {
    let run = factorial::run_one(Arm::D3, 1, tiny()).expect("a short depth row runs");
    assert_eq!(run.clones.len(), CLONES);
    assert_eq!(run.clone_births, 0, "no clone in the row reproduced");
    assert!(run.world_births > 0, "the ordinary founders still breed around them");
    let depths: BTreeSet<u64> = run.clones.iter().map(|c| c.depth.to_bits()).collect();
    assert_eq!(depths.len(), 2, "both depth levels are present in one world");
    for c in &run.clones {
        assert!(c.material_residual.abs() < 1e-9, "slot {}: material closes", c.slot);
        assert!(c.energy_residual.abs() < 1e-9, "slot {}: energy closes", c.slot);
        assert!(c.billed_ticks > 0 && c.probes > 0, "slot {}: billed and seen", c.slot);
    }
    let again = factorial::run_one(Arm::D3, 1, tiny()).expect("runs again");
    assert_eq!(run.final_state_hash, again.final_state_hash, "the world reproduces");
    assert_eq!(run.clones, again.clones, "and so does every clone's ledger");
}

/// The depth rows stand in the same eight cells arm A stood in, so this factorial and J's are
/// the same experiment with one locus added rather than two different placements.
#[test]
fn the_depth_rows_stand_where_arm_a_stood() {
    let a = factorial::run_one(Arm::A, 3, tiny()).expect("runs");
    let d = factorial::run_one(Arm::D2, 3, tiny()).expect("runs");
    assert_eq!(a.placements, d.placements, "one seed, one landscape, one placement");
}

/// A depth arm parses from its label and is not in the default three.
#[test]
fn the_depth_rows_parse_and_are_opt_in() {
    for arm in Arm::DEPTH_ROWS {
        assert_eq!(Arm::parse(arm.label()).expect("round trips"), arm);
        assert!(!Arm::ALL.contains(&arm), "the three-arm default is unchanged");
        assert!(arm.depth_row().is_some());
    }
    for arm in Arm::ALL {
        assert!(arm.depth_row().is_none(), "{} is not a depth row", arm.label());
    }
    assert!(Arm::parse("D5").is_err());
}
