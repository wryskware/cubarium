//! **The closed loop** (`design/webmodel/loop.md`): the producer half and the animal half on
//! one clock, exchanging [`Supply`], [`Take`] and [`Returns`] every step, with one ledger
//! across both that closes, and the budget sheet read off it.
//!
//! **The step, in the sim's phase order.** The sim's tick is `water::step; flora.step_clocked;
//! fauna.step(&world, &mut flora); world.advance_tick()` (`cubarium-voxel-sim/src/lib.rs:
//! 660-662`): the plants grow first, then every animal acts, its bites withdrawn from the plant
//! layer as it acts; deposits (dung, corpses) land between ticks and decompose in the next
//! plant step ("a tick's decomposition sees the previous inter-tick's deposits",
//! `cubarium-voxel-flora/src/lib.rs:5775-5782`). So one step of `dt` (the plant half's, 10 s):
//! 1. **supply**: the plant half steps (growth, seeds, deaths, the dead pools' decay, the
//!    film) and offers what it holds ([`crate::flora::model::Model::offer`]);
//! 2. **take**: the animal half steps `dt / fauna.dt` times (1 s each, its latches need it)
//!    against the offer less what it has already taken this step, so the take is bounded by
//!    the offer as the sim's bounded withdrawals are;
//! 3. **returns**: the take comes off the plant half's zones pro rata to their stock
//!    ([`crate::flora::model::Model::take_from`]), the animals settle the mineral it carried
//!    ([`FaunaModel::settle_mineral`]), and dung, corpses and mineral go back
//!    ([`crate::flora::model::Model::give`]) before the next plant step.

pub mod sheet;
pub mod start;

use cubarium_voxel_fauna::Founder;
use cubarium_voxel_flora::Species;

use crate::exchange::{Returns, Supply, Take};
use crate::fauna::{FaunaModel, LineageLedger};
use crate::fauna::rng::{Draws, Rng};
use crate::flora::model::{Counters, Model, NS};

/// The loop's own books: what crossed between the halves, cumulative.
#[derive(Clone, Debug, Default)]
pub struct Ledger {
    /// What the animals booked as taken, and what the plant half gave (the same unless an
    /// offer ran short, which the bounded substeps prevent; the gap is `unmet`).
    pub booked: Take,
    pub got: Take,
    pub unmet: f64,
    pub returned: Returns,
    /// Steps run, and world seconds.
    pub steps: u64,
    pub seconds: f64,
}

/// **The whole web**: both halves and the loop's books.
#[derive(Clone)]
pub struct Web {
    pub flora: Model,
    pub fauna: FaunaModel,
    /// Animal steps per plant step.
    pub sub: usize,
    /// **Scenario lever, not a sim knob**: a seedling browser's hazard per second on each
    /// species' juveniles (`W < donor_min`), applied as [`Take::seedlings`].
    pub browse: [f64; NS],
    pub ledger: Ledger,
    /// What each book held at the join, and its flow terms then (the halves may have run
    /// before they were joined): `[organic, mineral, plant mineral, plant organic, animal organic]`.
    held0: [f64; 5],
    flows0: [f64; 5],
}

/// One unit of mineral per unit of each flow the books carry: the closure check's terms.
#[derive(Clone, Copy, Debug, Default)]
pub struct Closure {
    pub organic_held: f64,
    pub organic_expected: f64,
    pub mineral_held: f64,
    pub mineral_expected: f64,
    /// Each half's own books (the take and the returns as their boundary flows): where a gap
    /// lies when the joint books do not close.
    pub flora_gap: f64,
    pub fauna_gap: f64,
    /// The plant half's own mineral books: held less (start − out in bites + back in returns).
    pub flora_mineral_gap: f64,
}

/// The animals' respiration by lineage: upkeep, motor (with strikes), digestion's heat, the
/// gestation loss and the bank's upkeep.
pub fn respired(l: &LineageLedger) -> f64 {
    l.flows.upkeep + l.flows.motor + l.flows.digestion_heat + l.gestation_loss + l.bank_upkeep
}

fn sub_take(left: &mut Supply, t: &Take) {
    for k in 0..NS {
        left.foliage[k] = (left.foliage[k] - t.foliage[k]).max(0.0);
    }
    left.litter = (left.litter - t.litter).max(0.0);
    left.carrion = (left.carrion - t.carrion).max(0.0);
    left.film = (left.film - t.film).max(0.0);
    left.nectar = (left.nectar - t.nectar).max(0.0);
    left.fruit = (left.fruit - t.fruit).max(0.0);
}

fn add_take(a: &mut Take, t: &Take) {
    for k in 0..NS {
        a.foliage[k] += t.foliage[k];
        a.seedlings[k] += t.seedlings[k];
    }
    a.litter += t.litter;
    a.carrion += t.carrion;
    a.film += t.film;
    a.nectar += t.nectar;
    a.fruit += t.fruit;
}

impl Web {
    /// Join the halves: the animal clock is set to the plant clock, and the plant half's
    /// clamp (`take`, `returns`) is emptied (the loop exchanges through `offer`, `take_from`
    /// and `give`).
    pub fn new(mut flora: Model, mut fauna: FaunaModel) -> Web {
        flora.take = Take::default();
        flora.returns = Returns::default();
        fauna.t_s = flora.t_s;
        let sub = ((flora.p.dt / fauna.dt).round() as usize).max(1);
        fauna.dt = flora.p.dt / sub as f64;
        let mut w = Web { flora, fauna, sub, browse: [0.0; NS], ledger: Ledger::default(), held0: [0.0; 5], flows0: [0.0; 5] };
        w.held0 = w.held();
        w.flows0 = w.flows();
        w
    }

    /// World minute.
    pub fn minute(&self) -> f64 {
        self.flora.minute()
    }

    /// The world day, seconds.
    pub fn day_s(&self) -> f64 {
        self.flora.day.day_s()
    }

    /// Organic matter both halves hold.
    pub fn organic(&self) -> f64 {
        self.flora.organic() + self.fauna.organic_held()
    }

    /// Mineral both halves hold: site pools, stand inventories, banks, and the animals'.
    pub fn mineral(&self) -> f64 {
        let (a, b, c) = self.flora.mineral();
        a + b + c + self.fauna.mineral
    }

    /// **One step** (the module doc's order).
    pub fn step(&mut self) {
        self.step_recording(None);
    }

    /// Step to world minute `minute`, keeping each step's offer to the animals (for
    /// [`Web::twin_replay`]).
    pub fn run_to_recording(&mut self, minute: f64, rec: &mut Vec<Supply>) {
        while self.minute() < minute - 1e-9 {
            self.step_recording(Some(rec));
        }
    }

    /// **The stochastic twin on a recorded supply**: the animals made whole bodies with the
    /// twin's draws (seeded), stepped against the mean field's offers step by step (`offers`,
    /// from [`Web::run_to_recording`] started at this state), their own take within each step
    /// coming off as in the loop. The plants do not answer the twin's take: an approximation
    /// that holds while the animals take a small share of each stock (the budget sheet shows
    /// it), and that keeps a twin at the animal half's cost.
    pub fn twin_replay(&self, seed: u64, offers: &[Supply]) -> FaunaModel {
        let mut f = self.fauna.clone();
        to_whole(&mut f);
        f.draws = Draws::Twin(Rng::new(seed));
        for offer in offers {
            let mut left = offer.clone();
            for _ in 0..self.sub {
                let (t, _) = f.step(&left);
                sub_take(&mut left, &t);
            }
        }
        f
    }

    fn step_recording(&mut self, rec: Option<&mut Vec<Supply>>) {
        // 1. Supply.
        self.flora.step();
        let offer = self.flora.offer();
        if let Some(r) = rec {
            r.push(offer.clone());
        }
        // 2. Take: the animals' steps against what is left.
        let mut left = offer.clone();
        let mut take = Take::default();
        let mut ret = Returns::default();
        for _ in 0..self.sub {
            let (t, r) = self.fauna.step(&left);
            sub_take(&mut left, &t);
            add_take(&mut take, &t);
            ret.litter += r.litter;
            ret.carrion += r.carrion;
        }
        let dt = self.flora.p.dt;
        for k in 0..NS {
            let h = self.browse[k];
            if h > 0.0 {
                take.seedlings[k] = offer.seedlings[k] * (1.0 - (-h * dt).exp());
            }
        }
        // 3. Returns.
        let cap0 = self.flora.c.cap_mineral_taken;
        let (got, mineral) = self.flora.take_from(&take);
        let eaten = mineral + (self.flora.c.cap_mineral_taken - cap0);
        ret.mineral = self.fauna.settle_mineral(eaten);
        self.flora.give(&ret);
        let l = &mut self.ledger;
        l.unmet += (take.organic() - got.organic()).max(0.0);
        add_take(&mut l.booked, &take);
        add_take(&mut l.got, &got);
        l.returned.litter += ret.litter;
        l.returned.carrion += ret.carrion;
        l.returned.mineral += ret.mineral;
        l.steps += 1;
        l.seconds += dt;
    }

    /// Step to world minute `minute`.
    pub fn run_to(&mut self, minute: f64) {
        while self.minute() < minute - 1e-9 {
            self.step();
        }
    }

    /// **The ledger across both halves**: organic held against
    /// `start + fixed + seep − plants' respiration − decomposition − animals' respiration −
    /// the fresh kills' decay − browsed seedlings + unmet`, and mineral held against
    /// `start + the caps' mineral eaten (the fungal place keeps none here) + the hold lever's
    /// top-up`. The take and the returns are internal and cancel.
    pub fn closure(&self) -> Closure {
        let (h, f) = (self.held(), self.flows());
        let expected: [f64; 5] = std::array::from_fn(|k| self.held0[k] + f[k] - self.flows0[k]);
        Closure {
            organic_held: h[0],
            organic_expected: expected[0],
            mineral_held: h[1],
            mineral_expected: expected[1],
            flora_mineral_gap: h[2] - expected[2],
            flora_gap: h[3] - expected[3],
            fauna_gap: h[4] - expected[4],
        }
    }

    /// The books' stocks: `[organic, mineral, plant mineral, plant organic, animal organic]`.
    fn held(&self) -> [f64; 5] {
        let (a, b, c) = self.flora.mineral();
        [self.organic(), self.mineral(), a + b + c, self.flora.organic(), self.fauna.organic_held()]
    }

    /// The books' cumulative flow terms, in [`Web::held`]'s order.
    fn flows(&self) -> [f64; 5] {
        let c = &self.flora.c;
        let animals: f64 = self.fauna.ledger.iter().map(respired).sum();
        let l = &self.ledger;
        [
            c.fixed + c.seep - c.respired - c.decomposed - animals - self.fauna.carcass_decayed - c.browsed_organic + l.unmet,
            c.cap_mineral_taken + c.held_in,
            -c.taken_mineral + c.returned_mineral + c.held_in,
            c.fixed + c.seep + c.returned - c.respired - c.decomposed - c.taken.organic(),
            l.booked.organic() - l.returned.litter - l.returned.carrion - animals - self.fauna.carcass_decayed,
        ]
    }

    /// The animals made whole bodies and their draws the stochastic twin's (seeded): the
    /// plant half stays the mean field, coupled to the twin's take.
    pub fn twin(&self, seed: u64) -> Web {
        let mut w = self.clone();
        let before = w.fauna.organic_held();
        to_whole(&mut w.fauna);
        // Rounding the cohorts is a one-off change of stock, not a flow: the books start from it.
        w.held0[0] += w.fauna.organic_held() - before;
        w.held0[4] += w.fauna.organic_held() - before;
        w.fauna.draws = Draws::Twin(Rng::new(seed));
        w
    }

    /// A snapshot of everything the budget sheet reads.
    pub fn snap(&self) -> Snap {
        Snap {
            minute: self.minute(),
            flora: self.flora.c.clone(),
            fauna: self.fauna.ledger.clone(),
            census: Founder::ALL.map(|f| self.fauna.census(f)),
            stands: self.flora.stands().map(|s| s.0),
            offer: self.flora.offer(),
            litter: self.flora.z.iter().map(|z| z.litter + z.dead_wood).sum(),
            carrion: self.flora.carrion,
            film: self.flora.film,
            caps_total: self.flora.z.iter().flat_map(|z| z.cohorts[Species::Glowcap.index()].iter()).map(|c| c.n * c.st.foliage).sum(),
            carcass: (self.fauna.carcass_laid, self.fauna.carcass_decayed, self.fauna.carcass_left, self.fauna.carcass_out),
            web: self.ledger.clone(),
            closure: self.closure(),
            fauna_mineral: (self.fauna.mineral, self.fauna.mineral_wanted()),
            enabled: self.fauna.enabled,
        }
    }
}

/// What the budget sheet reads at one time.
#[derive(Clone, Debug)]
pub struct Snap {
    pub minute: f64,
    pub flora: Counters,
    pub fauna: [LineageLedger; Founder::COUNT],
    pub census: [crate::fauna::Census; Founder::COUNT],
    pub stands: [f64; NS],
    pub offer: Supply,
    /// Litter and dead wood, together.
    pub litter: f64,
    pub carrion: f64,
    pub film: f64,
    pub caps_total: f64,
    /// The hunters' fresh kills: laid, decayed, left to carrion, beyond reach.
    pub carcass: (f64, f64, f64, f64),
    pub web: Ledger,
    pub closure: Closure,
    /// The animals' mineral inventory and what their tissue would hold at `n_tissue`.
    pub fauna_mineral: (f64, f64),
    pub enabled: [bool; Founder::COUNT],
}

/// The model's cohorts made whole bodies for the twin: each lineage's cohort sizes rounded so
/// the lineage keeps its rounded total (largest remainder), egg cohorts likewise. (The animal
/// half's `q2` does the same, `examples/fauna_model.rs`.)
pub fn to_whole(m: &mut FaunaModel) {
    for v in m.pops.iter_mut() {
        let total: f64 = v.iter().map(|c| c.n).sum();
        let want = total.round();
        let mut fl: Vec<(usize, f64)> = v.iter().enumerate().map(|(i, c)| (i, c.n - c.n.floor())).collect();
        for c in v.iter_mut() {
            c.n = c.n.floor();
        }
        let have: f64 = v.iter().map(|c| c.n).sum();
        fl.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        for (i, _) in fl.into_iter().take((want - have).max(0.0) as usize) {
            v[i].n += 1.0;
        }
        v.retain(|c| c.n > 0.0);
    }
    for e in m.eggs.iter_mut() {
        e.n = e.n.round();
    }
    m.eggs.retain(|e| e.n > 0.0);
}
