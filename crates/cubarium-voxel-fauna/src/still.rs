//! Why a body standing still is not failing: the legitimate reasons shared by the
//! observatory's stuck rule and the staged runner's liveness check.

use crate::{Animal, Fauna, HuntMode, MobilityMode, State, Steer, VigilMode};

/// Why an awake, free body standing still is not failing: the first legitimate reason
/// that holds now, or `None` when it is exposed to the stuck rule. A body is exposed only
/// while it **wants food** (eating or emergency latch on) and holds still for no chosen
/// reason. Asleep bodies (`rest`), riders, hunkered and perched bodies are resting by
/// choice or carried. A sated body (both latches off: digesting, fed rest, den rest, any
/// state) has nowhere it needs to go. A hungry body is excused while it waits in ambush
/// (the patrol's `Wait` with its lure lit), roosts (a glider's rest hour), is retracted
/// or freezes under a threat (its lineage's `Freeze` ground response), is a pack pup
/// held at the den and provisioned there, or is a pack adult its pack tactic holds (at
/// the den until the pack's hunt quorum; an emergency starts a hunt, so an emergency
/// member is not excused).
pub fn still_reason(fauna: &Fauna, a: &Animal) -> Option<&'static str> {
    let freezes = a
        .founder
        .is_some_and(|f| fauna.config().founder(f).vigilance.mode == VigilMode::Freeze);
    still_reason_of(a, fauna.steer_of(a.id), freezes)
}

/// [`still_reason`] given the body's steer source and whether its lineage freezes.
pub fn still_reason_of(a: &Animal, steer: Option<Steer>, freezes: bool) -> Option<&'static str> {
    if a.rest.is_some() || a.state == State::Asleep {
        Some("asleep")
    } else if a.attached.is_some() {
        Some("riding")
    } else if a.hunkered {
        Some("hunkered")
    } else if a.mobility.mode == MobilityMode::Perched {
        Some("perched")
    } else if !(a.eating || a.emergency) {
        Some("sated")
    } else if steer == Some(Steer::Patrol) && a.hunt.mode == HuntMode::Wait {
        Some("ambush")
    } else if steer == Some(Steer::Roost) {
        Some("roost")
    } else if a.mobility.retract_ticks > 0 || (freezes && steer == Some(Steer::Flee)) {
        Some("threat")
    } else if a.pack.is_some_and(|p| p.pup) {
        Some("pup")
    } else if a.pack.is_some() && steer == Some(Steer::Pack) && !a.emergency {
        Some("pack")
    } else {
        None
    }
}
