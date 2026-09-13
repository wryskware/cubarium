//! Independent observer boundary fixtures. No production writes or cohort experiment.
#[path = "quiet_compare/bouts.rs"]
mod bouts;

fn main() {}

#[cfg(test)]
mod independent {
    use super::bouts::{BoutEnd, Observer, Pre, RestClass};
    use cubarium_core::organism::Mode;
    use cubarium_core::quiet::{QuietEvent, QuietPolicy, QuietReason};
    use cubarium_core::{LifeEvent, OrganismId, World, WorldConfig};

    struct Pending {
        world: World,
        observer: Observer,
        pre: Pre,
        life: Vec<LifeEvent>,
        quiet: Vec<QuietEvent>,
        parent: OrganismId,
    }

    fn pending_birth() -> Pending {
        let mut cfg = WorldConfig::default();
        cfg.founders.kinds.clear();
        cfg.founders.count = 1;
        cfg.founders.initial_reserve_fraction = 1.0;
        cfg.founders.initial_energy_fraction = 1.0;
        cfg.drives.bud_min_age_seconds = 0.0;
        cfg.organism.gestation_seconds = 0.1;
        let mut world = World::new(cfg).unwrap();
        world.state.quiet.policy = QuietPolicy::PostBirthPauseV1;
        for (_, o) in world.state.organisms.iter_mut() {
            o.mode = Mode::Seeking;
            o.hunger_memory = 0.5;
        }
        let mut observer = Observer::new(&world.state);
        for _ in 0..100 {
            let pre = observer.before(&world);
            world.step();
            let life = world.drain_events();
            let quiet = world.drain_quiet_events();
            if let Some(parent) = quiet.iter().find_map(|e| match e {
                QuietEvent::Begin { parent, .. } => Some(*parent),
                _ => None,
            }) {
                return Pending {
                    world,
                    observer,
                    pre,
                    life,
                    quiet,
                    parent,
                };
            }
            observer.after(&world, pre, &life, &quiet).unwrap();
            assert!(observer.reconciled());
            observer.drain_bouts();
        }
        panic!("no real paid birth in the bounded fixture");
    }

    fn started() -> (World, Observer, OrganismId) {
        let p = pending_birth();
        let mut observer = p.observer;
        observer.after(&p.world, p.pre, &p.life, &p.quiet).unwrap();
        assert!(observer.reconciled());
        observer.drain_bouts();
        (p.world, observer, p.parent)
    }

    fn step(world: &mut World, observer: &mut Observer) {
        let pre = observer.before(world);
        world.step();
        let life = world.drain_events();
        let quiet = world.drain_quiet_events();
        observer.after(world, pre, &life, &quiet).unwrap();
    }

    #[test]
    fn astra_release_into_natural_rest_keeps_the_actual_release_reason() {
        let (mut world, mut observer, parent) = started();
        // A truthful satiated ordinary decision is possible on release. Synthetic resource
        // preparation isolates classification; it is not the proposed ecology recipe.
        let o = world.state.organisms.get_mut(parent).unwrap();
        o.reserve = 0.99 * o.phenotype.reserve_max;
        o.hunger_memory = 0.0;
        for _ in 0..40 {
            step(&mut world, &mut observer);
        }
        step(&mut world, &mut observer);
        assert_eq!(
            world.state.organisms.get(parent).unwrap().mode,
            Mode::Resting
        );
        assert_eq!(observer.releases, 1);
        let done = observer.drain_bouts();
        let recovery = done
            .iter()
            .find(|b| b.id == parent && b.class == RestClass::PostBirthRecovery)
            .unwrap();
        assert_eq!(recovery.ticks, 40);
        assert_eq!(
            recovery.end,
            BoutEnd::Released,
            "ordinary continued rest must not erase the successful recovery release"
        );
    }

    #[test]
    fn astra_death_on_last_held_interval_is_valid_and_retains_forty_completed_ticks() {
        let (world, mut observer, parent) = started();
        let birth = world.tick();
        let mut state = world.state;
        state.config.organism.max_age_seconds = (birth + 39) as f64 * cubarium_core::DT;
        let mut world = World::from_state(state).unwrap();
        for _ in 0..40 {
            step(&mut world, &mut observer);
        }
        assert!(world.state.organisms.get(parent).is_none());
        assert_eq!(observer.aborts.get("parent_gone"), Some(&1));
        assert!(
            observer.reconciled(),
            "a real core death on B+40 is not a malformed abort: {}",
            observer.summary()
        );
        let done = observer.drain_bouts();
        let recovery = done
            .iter()
            .find(|b| b.id == parent && b.class == RestClass::PostBirthRecovery)
            .unwrap();
        assert_eq!(
            recovery.ticks, 40,
            "the final held decision executed before age death"
        );
    }

    #[test]
    fn astra_duplicate_begin_record_is_not_a_second_paid_birth_opportunity() {
        let mut p = pending_birth();
        p.quiet.push(p.quiet[0]);
        let result = p.observer.after(&p.world, p.pre, &p.life, &p.quiet);
        assert!(
            result.is_err() || !p.observer.reconciled(),
            "duplicate Begin silently inflated admissions"
        );
    }

    #[test]
    fn astra_missing_begin_record_is_detected_from_the_real_new_pause() {
        let mut p = pending_birth();
        let result = p.observer.after(&p.world, p.pre, &p.life, &[]);
        assert!(
            result.is_err() || !p.observer.reconciled(),
            "new actual pause without Begin was certified"
        );
    }

    #[test]
    fn astra_unknown_refusal_does_not_count_as_a_real_birth_opportunity() {
        let (mut world, mut observer, _) = started();
        let pre = observer.before(&world);
        world.step();
        let life = world.drain_events();
        let mut quiet = world.drain_quiet_events();
        quiet.push(QuietEvent::Refuse {
            tick: world.tick(),
            parent: OrganismId {
                slot: 999,
                generation: 7,
            },
            child: OrganismId {
                slot: 998,
                generation: 8,
            },
            reason: QuietReason::Unaffordable,
        });
        let result = observer.after(&world, pre, &life, &quiet);
        assert!(
            result.is_err() || !observer.reconciled(),
            "invented refusal entered the opportunity denominator"
        );
    }

    #[test]
    fn astra_missing_end_record_is_not_a_valid_wakeup() {
        let (mut world, mut observer, _) = started();
        for _ in 0..40 {
            step(&mut world, &mut observer);
        }
        let pre = observer.before(&world);
        world.step();
        let life = world.drain_events();
        assert!(
            world
                .drain_quiet_events()
                .iter()
                .any(|e| matches!(e, QuietEvent::End { .. }))
        );
        let result = observer.after(&world, pre, &life, &[]);
        assert!(
            result.is_err() || !observer.reconciled(),
            "missing End silently became Woke/Reclassified"
        );
    }
}
