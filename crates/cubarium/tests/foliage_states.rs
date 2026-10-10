//! **Foliage states** (`design/handoffs/foliage-states-implementation-2026-10-08.md`): a
//! species whose model carries states is drawn whole in the state its **total** foliage
//! fraction picks, against the species config's thresholds and hysteresis, and moves
//! between states by a cross-fade on the frame clock. Every model here is built by hand
//! and every threshold, hysteresis and fade length is read from the config or set by
//! the test: nothing pins a tuned number.

use std::collections::BTreeSet;
use std::time::{Duration, Instant};

use cubarium::voxel::colours;
use cubarium::voxel::model::{
    self, FoliageTrack, ModelCell, ModelLibrary, PlantModel, PlantStep, StateInfo, Tag,
    fade_cells, next_state, state_at,
};
use cubarium::voxel::stand::{Part, Stands};
use cubarium_voxel::{Command as VoxelCommand, Config, Material, World};
use cubarium_voxel_flora::{Command, Flora, FloraConfig, FloraView, Species, Stand};

const SP: Species = Species::Bloomcrown;
const W: u32 = 24;
const H: u32 = 16;
const D: u32 = 6;

/// An empty grid that draws models unturned (`Stands::set_plant_turn`), so the drawn cells
/// read back as the model's own offsets. The turn has its own tests (`voxel_models`).
fn unturned() -> Stands {
    let mut stands = Stands::empty(W, H, D);
    stands.set_plant_turn(false);
    stands
}

fn world() -> World {
    let mut world = World::empty(Config {
        width: W,
        height: H,
        depth: D,
        ..Config::default()
    });
    for z in 0..D {
        for x in 0..i64::from(W) {
            for y in 0..=2 {
                world.apply(VoxelCommand::SetMaterial {
                    x,
                    y,
                    z,
                    material: Material::Soil,
                });
            }
        }
    }
    world
}

fn mc(offset: [i32; 3], material: u8, tag: Tag) -> ModelCell {
    ModelCell {
        offset,
        material,
        tag,
    }
}

/// The trunk every state shares.
fn trunk() -> Vec<ModelCell> {
    (0..4).map(|j| mc([0, j, 0], 1, Tag::Trunk)).collect()
}

/// A model of `n` states in one step: the shared trunk, a 3 × 3 crown of `Foliage(1)` at
/// height 5 in the full state only, a rosette of `Foliage(0)` at height 1 that every state
/// but the last keeps, and one marker cell per state at `[2, 4 + k, 0]` that only state
/// `k` holds.
fn model(n: usize, height_m: f64) -> PlantModel {
    let state = |k: usize| {
        let mut cells = trunk();
        if k == 0 {
            for dx in -1..=1 {
                for dz in -1..=1 {
                    cells.push(mc([dx, 5, dz], 3, Tag::Foliage(1)));
                }
            }
        }
        if k + 1 < n {
            for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                cells.push(mc([dx, 1, dz], 2, Tag::Foliage(0)));
            }
        }
        cells.push(mc([2, 4 + k as i32, 0], 4, Tag::Foliage(1)));
        cells
    };
    PlantModel {
        steps: vec![PlantStep {
            height_m,
            radius_m: 0.25,
            variants: vec![state(0)],
            states: (1..n).map(|k| vec![state(k)]).collect(),
        }],
        states: (0..n)
            .map(|k| StateInfo {
                name: format!("s{k}"),
                start: 1.0,
            })
            .collect(),
    }
}

/// One full-grown stand of `SP` in `flora`, and a library holding `model(n)` for it.
struct Rig {
    world: World,
    flora: Flora,
    lib: ModelLibrary,
    n: usize,
}

impl Rig {
    fn new(config: FloraConfig) -> Rig {
        let world = world();
        let mut flora = Flora::new(config);
        let wood_max = flora.config().species(SP).wood_max;
        assert!(flora.apply(
            &world,
            Command::Seed {
                x: 10,
                z: 3,
                species: SP,
                wood: wood_max
            }
        ));
        let sc = flora.config().species(SP);
        let n = sc.foliage_state_below.len() + 1;
        assert!(n >= 2, "the species under test has foliage states in its config");
        let mut lib = ModelLibrary::default();
        lib.insert_plant(SP, model(n, sc.crown_height_m_at(wood_max)));
        Rig {
            world,
            flora,
            lib,
            n,
        }
    }

    fn below(&self) -> Vec<f64> {
        self.flora.config().species(SP).foliage_state_below.clone()
    }

    fn hysteresis(&self) -> f64 {
        self.flora.config().species(SP).foliage_state_hysteresis
    }

    /// The stand with its total foliage at `fraction` of capacity, spread over its
    /// layers in `shares` (summing to one).
    fn stand_at(&self, fraction: f64, shares: &[f64]) -> Stand {
        let mut s = self.flora.view().stands[0];
        let sc = self.flora.config().species(SP);
        s.foliage = fraction * sc.alpha * s.wood;
        for (i, share) in shares.iter().enumerate() {
            s.layer_stock[i] = s.foliage * share;
        }
        s
    }

    /// Rebuild `stands` with the stand at `fraction` (spread evenly over the stand's
    /// layers' own shares), on the frame clock `now`.
    fn draw(&self, stands: &mut Stands, fraction: f64, now: Instant) {
        let shares = self.even_shares();
        self.draw_stand(stands, self.stand_at(fraction, &shares), now);
    }

    fn draw_stand(&self, stands: &mut Stands, stand: Stand, now: Instant) {
        let list = [stand];
        let view = FloraView {
            stands: &list,
            crowns: cubarium_voxel_flora::CrownCache::none(),
            ..self.flora.view()
        };
        stands.set_film_now(Some(now));
        stands.rebuild_with(&self.world.view(), view, &self.lib);
    }

    /// The layers' own shares of the stand's foliage (what a full stand holds).
    fn even_shares(&self) -> Vec<f64> {
        let s = &self.flora.view().stands[0];
        let total: f64 = s.layer_stock.iter().sum();
        s.layer_stock.iter().map(|v| v / total).collect()
    }

    /// The model offsets drawn in the grid.
    fn drawn(&self, stands: &Stands) -> BTreeSet<[i32; 3]> {
        let site = self.flora.view().stands[0].site;
        let (x, y0, z0) = (i64::from(site.x), i64::from(site.y) + 1, i64::from(site.z));
        let mut out = BTreeSet::new();
        for dx in -3..=3 {
            for dy in 0..10 {
                for dz in -2..=2 {
                    let z = z0 + i64::from(dz);
                    if z < 0 || z >= i64::from(D) {
                        continue;
                    }
                    if stands.at(x + i64::from(dx), y0 + i64::from(dy), z as u32) != Part::None {
                        out.insert([dx, dy, dz]);
                    }
                }
            }
        }
        out
    }

    /// The offsets of state `k`'s cells.
    fn state_cells(&self, k: usize) -> BTreeSet<[i32; 3]> {
        self.lib.plant(SP).unwrap().steps[0]
            .cells(0, k)
            .unwrap()
            .iter()
            .map(|m| m.offset)
            .collect()
    }

    /// A fraction inside state `k`'s band (between its threshold and the next one).
    fn inside(&self, k: usize) -> f64 {
        let below = self.below();
        let hi = if k == 0 { 1.0 } else { below[k - 1] };
        let lo = if k == below.len() { 0.0 } else { below[k] };
        0.5 * (lo + hi)
    }
}

fn marker(k: usize) -> [i32; 3] {
    [2, 4 + k as i32, 0]
}

/// The state a fresh stand draws is the one its total foliage fraction picks against the
/// species' thresholds: exactly that state's cells, whole.
#[test]
fn the_state_follows_the_total_foliage_fraction() {
    let rig = Rig::new(FloraConfig::default());
    for k in 0..rig.n {
        let mut stands = unturned();
        rig.draw(&mut stands, rig.inside(k), Instant::now());
        assert_eq!(rig.drawn(&stands), rig.state_cells(k), "state {k} at {}", rig.inside(k));
        for j in (0..rig.n).filter(|&j| j != k) {
            assert!(!rig.drawn(&stands).contains(&marker(j)), "no other state's marker");
        }
    }
    // And the pure rule: the count of thresholds the fraction is under.
    let below = rig.below();
    for (k, &t) in below.iter().enumerate() {
        assert_eq!(state_at(t, &below), k, "at a threshold the fuller state holds");
        assert_eq!(state_at(t - 1e-9, &below), k + 1, "just under it, the barer one");
    }
}

/// A state is left for a barer one as soon as the fraction is below its threshold, and
/// regained only once the fraction is at least the hysteresis above it.
#[test]
fn hysteresis_holds_a_state_until_the_fraction_clears_its_threshold() {
    let rig = Rig::new(FloraConfig::default());
    let (below, h) = (rig.below(), rig.hysteresis());
    assert!(h > 0.0, "the config's hysteresis is a positive band");
    let t = below[0];
    let mut stands = unturned();
    stands.set_foliage_fade_s(0.0);
    let t0 = Instant::now();
    let mut frame = 0u64;
    let mut draw = |f: f64| {
        frame += 1;
        rig.draw(&mut stands, f, t0 + Duration::from_millis(16 * frame));
        rig.drawn(&stands)
    };
    assert!(draw((t + h + 0.01).min(1.0)).contains(&marker(0)));
    assert!(draw(t - 0.01).contains(&marker(1)), "below the threshold: the next state");
    assert!(draw(t + 0.5 * h).contains(&marker(1)), "inside the band: it stays");
    assert!(draw(t + h).contains(&marker(0)), "at the threshold plus the band: back");
    // The pure rule agrees, from either side.
    assert_eq!(next_state(0, t + 0.5 * h, &below, h), 0);
    assert_eq!(next_state(1, t + 0.5 * h, &below, h), 1);
    assert_eq!(next_state(1, t + h, &below, h), 0);
    assert_eq!(next_state(0, 0.0, &below, h), below.len(), "straight to the barest");
}

/// A change of state fades on the frame clock over the configured time: cells both states
/// hold never leave, the outgoing state's other cells only ever go and the incoming
/// state's only ever come, it starts as the old state and ends as the new one, and in
/// between it is a mix. From the full state to the barest it fades directly: no middle
/// state's cell is ever drawn.
#[test]
fn a_state_change_cross_fades_monotonically_on_the_frame_clock() {
    let rig = Rig::new(FloraConfig::default());
    let fade_s = 1.5;
    let mut stands = unturned();
    stands.set_foliage_fade_s(fade_s);
    let t0 = Instant::now();
    rig.draw(&mut stands, 1.0, t0);
    let (from, to) = (rig.state_cells(0), rig.state_cells(rig.n - 1));
    assert_eq!(rig.drawn(&stands), from);
    let shared: BTreeSet<_> = from.intersection(&to).copied().collect();
    let outgoing: BTreeSet<_> = from.difference(&to).copied().collect();
    let incoming: BTreeSet<_> = to.difference(&from).copied().collect();
    assert!(!outgoing.is_empty() && !incoming.is_empty() && !shared.is_empty());

    let steps = 30;
    let (mut last_out, mut last_in) = (outgoing.len(), 0);
    let mut mixed = false;
    for i in 0..=steps + 2 {
        let t = t0 + Duration::from_secs_f64(fade_s * f64::from(i) / f64::from(steps));
        rig.draw(&mut stands, 0.0, t);
        let drawn = rig.drawn(&stands);
        assert!(shared.is_subset(&drawn), "a cell both states hold never leaves");
        for k in 1..rig.n - 1 {
            assert!(!drawn.contains(&marker(k)), "no middle state on a direct fade");
        }
        assert!(drawn.is_subset(&from.union(&to).copied().collect()));
        let n_out = drawn.intersection(&outgoing).count();
        let n_in = drawn.intersection(&incoming).count();
        assert!(n_out <= last_out && n_in >= last_in, "monotonic at step {i}");
        mixed |= drawn != from && drawn != to;
        (last_out, last_in) = (n_out, n_in);
        if i == 0 {
            assert_eq!(drawn, from, "no time has passed: still the old state");
        }
        if i > steps {
            assert_eq!(drawn, to, "after the fade: the new state, whole");
        }
    }
    assert!(mixed, "the change passes through a mix: it does not pop");
}

/// Asked back mid-fade, the fade runs backward: the cells it swapped swap back, and it
/// settles where it started.
#[test]
fn a_fade_reversed_midway_runs_back_to_where_it_started() {
    let rig = Rig::new(FloraConfig::default());
    let fade_s = 1.0;
    let mut stands = unturned();
    stands.set_foliage_fade_s(fade_s);
    let t0 = Instant::now();
    let at = |s: f64| t0 + Duration::from_secs_f64(s);
    rig.draw(&mut stands, 1.0, at(0.0));
    let full = rig.drawn(&stands);
    rig.draw(&mut stands, 0.0, at(0.0));
    rig.draw(&mut stands, 0.0, at(0.5 * fade_s));
    let midway = rig.drawn(&stands);
    assert_ne!(midway, full, "half way the fade has moved");
    let mut last = midway.symmetric_difference(&full).count();
    for i in 1..=12 {
        rig.draw(&mut stands, 1.0, at(0.5 * fade_s + 0.05 * fade_s * f64::from(i)));
        let d = rig.drawn(&stands).symmetric_difference(&full).count();
        assert!(d <= last, "on its way back");
        last = d;
    }
    assert_eq!(rig.drawn(&stands), full, "back where it started");
}

/// The state model is drawn whole: a full stand whose foliage all sits in its upper
/// layer still draws the rosette of its lower layer. With the species' thresholds
/// emptied in the config, the same model and stand thin by layer as before.
#[test]
fn a_state_species_is_never_thinned_by_layer() {
    let rig = Rig::new(FloraConfig::default());
    // Every bit of foliage in the crown (layer 1), none in the rosette (layer 0).
    let mut lopsided = vec![0.0; rig.even_shares().len()];
    lopsided[1] = 1.0;
    let rosette: BTreeSet<[i32; 3]> = [[1, 1, 0], [-1, 1, 0], [0, 1, 1], [0, 1, -1]].into();
    let mut stands = unturned();
    rig.draw_stand(&mut stands, rig.stand_at(1.0, &lopsided), Instant::now());
    assert!(rosette.is_subset(&rig.drawn(&stands)), "every rosette cell, layer 0 empty or not");

    let mut config = FloraConfig::default();
    config.species_mut(SP).foliage_state_below.clear();
    let plain = Rig {
        world: world(),
        flora: {
            let mut f = Flora::new(config);
            let wood = f.config().species(SP).wood_max;
            assert!(f.apply(&rig.world, Command::Seed { x: 10, z: 3, species: SP, wood }));
            f
        },
        lib: rig.lib.clone(),
        n: rig.n,
    };
    let mut stands = unturned();
    plain.draw_stand(&mut stands, plain.stand_at(1.0, &lopsided), Instant::now());
    assert!(
        rosette.is_disjoint(&plain.drawn(&stands)),
        "without states, an empty layer 0 thins its rosette away"
    );
}

/// A fallen foliage cell is painted as its layer's foliage at the top of the ramp,
/// wherever it sits: the same style as a crown cell of that layer and material at the
/// model's top, and not the dark base a ground cell of that layer takes.
#[test]
fn fallen_foliage_paints_at_the_top_of_its_layers_ramp() {
    let p2 = model::named_index("p2");
    let top = colours::plant(SP, Tag::Foliage(1), p2, colours::BANDS - 1, false);
    assert_eq!(colours::plant(SP, Tag::Fallen(1), p2, 0, false), top);
    assert_ne!(colours::plant(SP, Tag::Foliage(1), p2, 0, false), top, "the ramp has a base");

    let mut rig = Rig::new(FloraConfig::default());
    let height_m = rig.lib.plant(SP).unwrap().steps[0].height_m;
    let mut cells = trunk();
    cells.push(mc([0, 6, 0], p2, Tag::Foliage(1)));
    cells.push(mc([1, 0, 0], p2, Tag::Fallen(1)));
    cells.push(mc([-1, 0, 0], p2, Tag::Foliage(1)));
    let mut lib = ModelLibrary::default();
    lib.insert_plant(
        SP,
        PlantModel {
            steps: vec![PlantStep {
                height_m,
                radius_m: 0.25,
                variants: vec![cells],
                ..Default::default()
            }],
            ..Default::default()
        },
    );
    rig.lib = lib;
    let mut stands = unturned();
    rig.draw(&mut stands, 1.0, Instant::now());
    let site = rig.flora.view().stands[0].site;
    let (x, y0, z) = (i64::from(site.x), i64::from(site.y) + 1, site.z);
    let style = |dx: i64, dy: i64| stands.style(stands.at(x + dx, y0 + dy, z)).expect("drawn");
    assert_eq!(style(1, 0), style(0, 6), "fallen = the layer's top");
    assert_ne!(style(-1, 0), style(0, 6), "a ground foliage cell is not");
}

/// The change-only rebuild draws exactly what the full rebuild draws, every frame of a
/// fade.
#[test]
fn the_change_only_rebuild_matches_the_full_one_through_a_fade() {
    let rig = Rig::new(FloraConfig::default());
    let mut full = unturned();
    let mut inc = unturned();
    inc.set_incremental(true);
    let t0 = Instant::now();
    let script = [1.0, 1.0, 0.0, 0.0, 0.0, 1.0, 1.0, rig.inside(1), rig.inside(1), 0.0];
    for (i, &f) in script.iter().chain(std::iter::repeat_n(&0.0, 40)).enumerate() {
        let t = t0 + Duration::from_millis(100 * i as u64);
        rig.draw(&mut full, f, t);
        rig.draw(&mut inc, f, t);
        assert_eq!(rig.drawn(&full), rig.drawn(&inc), "frame {i}");
    }
}

/// The track itself: settled tracks stay put, a fade takes `fade_s`, and a request for a
/// third state mid-fade first finishes the fade it is in.
#[test]
fn a_track_finishes_its_fade_before_starting_the_next() {
    let mut t = FoliageTrack::settled(0);
    t.advance(0, 10.0, 2.0);
    assert_eq!(t, FoliageTrack::settled(0));
    t.advance(1, 0.5, 2.0);
    assert!(t.is_fading() && (t.from, t.to) == (0, 1));
    let b = t.blend;
    t.advance(2, 0.5, 2.0);
    assert!((t.from, t.to) == (0, 1) && t.blend > b, "still fading 0 to 1");
    t.advance(2, 10.0, 2.0);
    assert_eq!(t, FoliageTrack::settled(1));
    t.advance(2, 0.0, 2.0);
    assert!((t.from, t.to) == (1, 2), "then on to 2");
    t.advance(2, 0.0, 0.0);
    assert_eq!(t, FoliageTrack::settled(2), "a zero fade switches at once");
}

/// The fade's cell rule on hand-built cells: at blend 0 it is the old cells, at 1 the new,
/// one cell per offset where both states hold a different cell there.
#[test]
fn fade_cells_swaps_each_offset_once() {
    let from: Vec<ModelCell> = (0..40).map(|i| mc([i, 0, 0], 1, Tag::Foliage(0))).collect();
    let to: Vec<ModelCell> = (0..40)
        .map(|i| mc([i, 0, 0], if i % 2 == 0 { 1 } else { 2 }, Tag::Foliage(0)))
        .collect();
    let sorted = |mut v: Vec<ModelCell>| {
        v.sort_by_key(|m| m.offset);
        v
    };
    assert_eq!(sorted(fade_cells(&from, &to, 9, 0.0)), from);
    assert_eq!(sorted(fade_cells(&from, &to, 9, 1.0)), to);
    for b in [0.1, 0.4, 0.7] {
        let mid = fade_cells(&from, &to, 9, b);
        let offsets: BTreeSet<_> = mid.iter().map(|m| m.offset).collect();
        assert_eq!(offsets.len(), 40, "every offset once at blend {b}");
        assert_eq!(mid.len(), 40);
    }
}
