//! **Meadow levers** (`design/webmodel/meadow.md`): candidate mechanisms that could give the
//! grass a lasting share of the drained ground while the woody species still spread. Each
//! is a **scenario lever of the model, not a sim knob**: the sim has none of them today
//! (the doc cites what it has instead). Every lever is off at its default, so a model built
//! with [`Meadow::default`] is the calibrated model unchanged.
//!
//! - **a. crop** — grazers crop woody seedlings as they graze the sward: a hazard on each
//!   woody juvenile in the grazer's band, proportional to the sward foliage grazed per face
//!   of its zone in the last step (the open loop's logged take, spread as the take is).
//! - **b. dry spells** — every `dry_every_s` a spell of `dry_len_s` in which the drained
//!   zones' root-box water reads `dry_water` of its replayed value: uncured woody seedlings
//!   starve on the sim's own law while the turf cures and waits (`cure_max_s`).
//! - **c. sward** — a dense turf suppresses woody germination through it (thatch: the
//!   overtopping rate over `1 + sward_germ · fill`, `fill` the turf's foliage over its cap),
//!   and woody juveniles share the topsoil water with the turf around them (their water
//!   times `1 − sward_draw · turf share of the zone`): Walter's two layers, seedling stage.
//! - **d. lifespan** — adult woody stands die at a constant hazard `1 / lifespan_s`, which
//!   opens their faces (gap dynamics).
//! - **f. contest** — a woody seed that comes up through turf no longer kills it at once:
//!   the recruit stands in the sward (a separate stock, [`super::model::ZoneState::contest`])
//!   under the turf's shade (`contest_tau` at a full sward) and in its topsoil water (lever
//!   c's `sward_draw`), exposed to the crop (a) and dry spells (b). When the recruit escapes
//!   above it (`W ≥ contest_escape · donor_min`) the turf is **not** overtopped (CONTEST,
//!   Wrysk 2026-10-05: "woody plants just naturally win over grass because they start
//!   shading it"): it stays on as the face's understorey ([`super::model::ZoneState::under`]),
//!   under the crown's shade, and dies only when its own budget fails. A recruit that dies
//!   first leaves the turf in place. `contest=sim` is the sim's rule: escape when the crown
//!   leaves its seedling cap (above the sward), the sward's shade by the shade law, and the
//!   canopy gate read under it.
//! - **e. thin soil** — a woody species needs at least `root_soil_min` soil voxels under
//!   its face; a zone's woody newborns come up only on the share of its non-woody faces
//!   with that much soil ([`Meadow::deep`], from the census's [`super::census::Face::soil_depth`]).

use cubarium_voxel_flora::Species;

use super::census::Face;
use super::zones::Zones;

const NS: usize = Species::COUNT;

/// The meadow levers. All off by default.
#[derive(Clone, Debug)]
pub struct Meadow {
    /// The species the levers act on (the woody overtoppers: bloomcrown, lanternberry,
    /// vaulttree).
    pub woody: [bool; NS],
    /// **a.** Hazard per second on a woody juvenile in the grazer's band per unit of sward
    /// foliage grazed per face per second in its zone (units: 1 / organic unit). 0: off.
    pub crop_per_graze: f64,
    /// **a.** The grazer's band, as a share of each species' `donor_min`: juveniles with
    /// `W` under it are cropped.
    pub crop_wood_max: f64,
    /// **a.** The species whose grazed foliage counts as the sward (ground covers).
    pub sward: [bool; NS],
    /// **b.** A dry spell every this many seconds (0: off), lasting `dry_len_s`, starting
    /// `dry_phase_s` after the run's start.
    pub dry_every_s: f64,
    pub dry_len_s: f64,
    pub dry_phase_s: f64,
    /// **b.** The share of the replayed root-box water the affected zones read in a spell.
    pub dry_water: f64,
    /// **b.** Whether the wet zones dry too (false: drained zones only).
    pub dry_wet_zones: bool,
    /// **c.** Thatch: woody overtopping through turf divided by `1 + sward_germ · fill`.
    pub sward_germ: f64,
    /// **c.** Topsoil water drawn by the turf: a woody juvenile's water times
    /// `1 − sward_draw · (turf stands / zone faces)`.
    pub sward_draw: f64,
    /// **d.** Mean adult lifespan per species, seconds (0: immortal, as in the sim).
    pub lifespan_s: [f64; NS],
    /// **f.** The recruit's escape size as a share of its `donor_min` (0: off, the sim's
    /// immediate overtopping before CONTEST). Negative: the end of its capped seedling stage
    /// (`w0 · wood_max`), where its crown rises above the sward — the sim's rule.
    pub contest_escape: f64,
    /// **f.** The optical depth of a full sward (turf foliage at its cap) over a contested
    /// recruit; it scales with the zone's turf fill. Negative: the sim's shade law on the
    /// zone's mean turf mat ([`super::model::Model::contest_shade`]).
    pub contest_tau: f64,
    /// **e.** Soil voxels a species needs under its face (0: none).
    pub root_soil_min: [u32; NS],
    /// **e.** Per zone and species: faces with at least `root_soil_min` soil
    /// ([`Meadow::set_deep`]); empty when the lever is off.
    pub deep: Vec<[f64; NS]>,
}

impl Default for Meadow {
    fn default() -> Meadow {
        let mut woody = [false; NS];
        for sp in [Species::Bloomcrown, Species::Lanternberry, Species::Vaulttree] {
            woody[sp.index()] = true;
        }
        let mut sward = [false; NS];
        for sp in [Species::Springturf, Species::Velvetpad, Species::Stonecushion] {
            sward[sp.index()] = true;
        }
        Meadow {
            woody,
            crop_per_graze: 0.0,
            crop_wood_max: 1.0,
            sward,
            dry_every_s: 0.0,
            dry_len_s: 0.0,
            dry_phase_s: 0.0,
            dry_water: 1.0,
            dry_wet_zones: false,
            sward_germ: 0.0,
            sward_draw: 0.0,
            contest_escape: 0.0,
            contest_tau: 0.0,
            lifespan_s: [0.0; NS],
            root_soil_min: [0; NS],
            deep: Vec::new(),
        }
    }
}

impl Meadow {
    /// Whether a dry spell holds at `t_s` for a run that started at `t0_s`.
    pub fn dry_now(&self, t_s: f64, t0_s: f64) -> bool {
        if !(self.dry_every_s > 0.0) || !(self.dry_len_s > 0.0) || self.dry_water >= 1.0 {
            return false;
        }
        let since = t_s - t0_s - self.dry_phase_s;
        since >= 0.0 && since.rem_euclid(self.dry_every_s) < self.dry_len_s
    }

    /// **e.** Count, per zone and species, the faces with enough soil for it.
    pub fn set_deep(&mut self, zones: &Zones, faces: &[Face]) {
        if self.root_soil_min.iter().all(|&d| d == 0) {
            self.deep.clear();
            return;
        }
        let mut deep = vec![[0.0f64; NS]; zones.zones.len()];
        for f in faces {
            let zi = zones.of_face[&(f.x, f.y, f.z)];
            for s in 0..NS {
                if f.soil_depth >= self.root_soil_min[s] {
                    deep[zi][s] += 1.0;
                }
            }
        }
        self.deep = deep;
    }

    /// **e.** The share of a zone's faces not held by the levers' species that have enough
    /// soil for species `s`, with `counts` stands per species in the zone of `area` faces.
    /// Each woody stand sits on a face deep enough for itself: one of a species needing at
    /// least as much soil as `s` sits on a face deep enough for `s`; one needing less, on
    /// such a face with chance `deep[s] / deep[own]`. The faces nobody woody holds are
    /// taken as mixed at random.
    pub fn deep_share(&self, zi: usize, s: usize, area: f64, counts: &[f64]) -> f64 {
        if self.root_soil_min[s] == 0 {
            return 1.0;
        }
        let Some(d) = self.deep.get(zi) else { return 1.0 };
        let (mut woody, mut on_deep) = (0.0, 0.0);
        for t in (0..NS).filter(|&t| self.woody[t]) {
            woody += counts[t];
            on_deep += if self.root_soil_min[t] >= self.root_soil_min[s] {
                counts[t]
            } else if d[t] > 0.0 {
                counts[t] * (d[s] / d[t]).min(1.0)
            } else {
                0.0
            };
        }
        let free = (area - woody).max(0.0);
        if free <= 0.0 {
            return 0.0;
        }
        ((d[s] - on_deep).max(0.0) / free).clamp(0.0, 1.0)
    }
}
