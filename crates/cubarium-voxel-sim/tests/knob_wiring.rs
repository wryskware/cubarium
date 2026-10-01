//! **Knob wiring** (R2 integration): a non-default `fauna.cue` reaches the litter field a
//! static arena's founder actually smells, on both of the static schedule's field paths —
//! [`Arena::prepare_senses`] + `into_sim_prepared`, and `into_sim` (which settles its own
//! field in `Sim::new_static`). Before R2's integration both built their field on the
//! default `CueConfig` whatever the layer's config said.

use std::sync::{Arc, Mutex};

use cubarium_voxel_fauna::{Actions, Controller, FaunaConfig, Founder, Response};
use cubarium_voxel_sim::params::Params;
use cubarium_voxel_sim::{Arena, ArenaGrid, LayerConfig, SimConfig};

/// Records the chem response channel of every observation and stands still.
struct ChemRecorder {
    offset: usize,
    log: Arc<Mutex<Vec<f64>>>,
}

impl Controller for ChemRecorder {
    fn drive(&mut self, o: &[f64]) -> Response {
        self.log.lock().expect("the log").push(o[self.offset]);
        Response::Bounded(Actions::REST)
    }

    fn reset(&mut self) {}
}

fn chem_offset() -> usize {
    Founder::Blind
        .manifest()
        .modules
        .iter()
        .find(|m| m.name == "Chem(detritus)")
        .expect("the blind manifest has its chem cue")
        .offset
}

/// The founder's chem readings over a short static run on `fauna`, through the prepared
/// path (`prepared`) or the self-settling one.
fn chem_trace(fauna: FaunaConfig, prepared: bool) -> Vec<f64> {
    let grid = ArenaGrid::Standard;
    let mut layers = LayerConfig::new(&Params::defaults(), grid.voxel_m());
    layers.fauna = fauna;
    let arena = Arena::build(Founder::Blind, 1, grid, layers);
    let id = arena.animal_id.expect("the arena placed its founder");
    let mut sim = if prepared {
        let senses = arena.prepare_senses();
        arena.into_sim_prepared(SimConfig::with_threads(1), senses)
    } else {
        arena.into_sim(SimConfig::with_threads(1))
    };
    let log = Arc::new(Mutex::new(Vec::new()));
    let recorder = ChemRecorder {
        offset: chem_offset(),
        log: Arc::clone(&log),
    };
    assert!(sim.fauna_mut().set_controller(id, Box::new(recorder)));
    for _ in 0..40 {
        sim.step_static();
    }
    let out = log.lock().expect("the log").clone();
    assert!(!out.is_empty(), "the founder was sampled");
    out
}

#[test]
fn a_changed_cue_reaches_the_static_arenas_field_on_both_paths() {
    let default = FaunaConfig::default();
    let mut weak = default.clone();
    // A larger emission reference makes every source emit less: the same litter is a
    // smaller fraction of the reference (emission is `(stock / reference).min(1)`).
    weak.cue.emit_reference = default.cue.emit_reference * 20.0;
    for prepared in [true, false] {
        let base = chem_trace(default.clone(), prepared);
        let again = chem_trace(default.clone(), prepared);
        let moved = chem_trace(weak.clone(), prepared);
        assert_eq!(base, again, "the arena run repeats (prepared {prepared})");
        assert!(
            base.iter().any(|&c| c > 0.0),
            "the founder smells something on the default cue (prepared {prepared})"
        );
        assert_ne!(
            base, moved,
            "fauna.cue.emit_reference must change what the founder smells (prepared {prepared})"
        );
    }
}
