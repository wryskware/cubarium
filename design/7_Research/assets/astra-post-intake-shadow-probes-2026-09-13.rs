// Independent public-API probes for the in-progress post-intake shadow.
// Loaded as a test library by captures/build-cache/astra-post-intake-review/probe/Cargo.toml.
#[cfg(test)]
mod tests {
    use cubarium_core::{World, WorldConfig, DT};
    use cubarium_core::ids::{OrganismId, Slots};
    use cubarium_core::organism::{Mode, Organism};
    use cubarium_core::post_intake::{quota, PostIntakeShadow, ShadowEvent};

    fn fixture() -> (WorldConfig, OrganismId, Organism) {
        let cfg = WorldConfig::default();
        let world = World::new(cfg.clone()).unwrap();
        let (id, o) = world.state.organisms.iter().next().unwrap();
        let mut o = o.clone();
        o.structure = o.phenotype.structure_adult;
        o.reserve = 0.5 * o.phenotype.reserve_max;
        o.energy = 0.9 * o.phenotype.energy_max;
        o.mode = Mode::Feeding;
        o.escrow = None;
        (cfg, id, o)
    }
    fn admit(s: &mut PostIntakeShadow, cfg: &WorldConfig, id: OrganismId, o: &Organism) {
        let q = quota(o, &cfg.organism, true, true).unwrap();
        s.observe_settlement(id, o, &cfg.organism, true, true, 100, q);
        s.observe_boundary(id, o, &cfg.organism, true, true, 100, DT, o.mode, false, false);
        assert!(matches!(s.drain_events().as_slice(),
            [ShadowEvent::Attempt { admitted: true, .. }]));
    }

    #[test]
    fn baseline_inactive_mode_ends_active_ordinary_qualification() {
        let (cfg, id, mut o) = fixture();
        let mut s = PostIntakeShadow::new(64);
        admit(&mut s, &cfg, id, &o);
        o.mode = Mode::Resting;
        s.observe_boundary(id, &o, &cfg.organism, true, true, 101, DT, o.mode, false, false);
        assert_eq!(s.open_windows(), 0, "an inactive baseline no longer qualifies");
        assert!(matches!(s.drain_events().as_slice(), [ShadowEvent::Abort { .. }]));
    }

    #[test]
    fn censored_decisions_and_forms_reconcile_with_the_censor_event() {
        let (cfg, id, o) = fixture();
        let mut s = PostIntakeShadow::new(64);
        admit(&mut s, &cfg, id, &o);
        for b in 101..=105 {
            s.observe_boundary(id, &o, &cfg.organism, true, true, b, DT, o.mode, false, false);
        }
        s.censor(105);
        assert!(matches!(s.drain_events().as_slice(),
            [ShadowEvent::Censor { completed_decisions: 5, .. }]));
        assert_eq!(s.counters().total.completed_window_decisions, 5);
        assert_eq!(s.counters().by_form[o.phenotype.form as usize].censored, 1);
        assert_eq!(s.counters().by_form[o.phenotype.form as usize].completed_window_decisions, 5);
    }

    #[test]
    fn completed_boundary_living_exposure_excludes_the_dying_member() {
        let (cfg, id, o) = fixture();
        let mut s = PostIntakeShadow::new(64);
        s.observe_boundary(id, &o, &cfg.organism, true, true, 100, DT, o.mode, true, false);
        assert_eq!(s.counters().total.organism_ticks, 0);
        assert_eq!(s.counters().total.eligible_ticks, 0);
    }

    #[test]
    fn expired_episodes_are_attributed_to_the_known_living_form() {
        let (cfg, _, o) = fixture();
        let mut organisms = Slots::with_capacity(64);
        let id = organisms.insert(o.clone());
        let mut s = PostIntakeShadow::new(64);
        let q = quota(&o, &cfg.organism, true, true).unwrap();
        s.observe_settlement(id, &o, &cfg.organism, true, true, 100, q * 0.5);
        s.prune_removed(&organisms, 120);
        assert_eq!(s.counters().total.episodes_expired, 1);
        assert_eq!(s.counters().by_form[o.phenotype.form as usize].episodes_expired, 1);
    }
}
