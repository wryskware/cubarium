//! The world: checkpointed state plus transient caches, and the tick.

use std::f64::consts::TAU;

use serde::{Deserialize, Serialize};

use cubarium_surface::{
    CELL_COUNT, CellId, ChartImage, FACE_EXTENT, Face, FieldGraph, MAX_SEAMS, ScalarField, SurfacePoint, Travel,
    Vec2, cell_of, chart_images, face_frame, travel_into, unfold_with,
};

use crate::config::{FounderKind, WorldConfig};
use crate::controller::{Decision, Observation, TurnGate, decide};
use crate::events::LifeEvent;
use crate::fields::Fields;
use crate::genome::{Genome, MAX_FORMS, decode};
use crate::habitat::{Habitat, Weather};
use crate::ids::{OrganismId, Slots};
use crate::organism::{DeathCause, Escrow, Mode, Organism, Origin};
use crate::pairs::{Body, NeighborLists};
use crate::rng::{Counter, Stream, normal, unit};
use crate::telemetry::Telemetry;
use crate::view::{FieldDump, OrganismView, RenderView};
use crate::water;
use crate::{DT, pairs, snapshot};

/// Everything a checkpoint must capture. Transient caches are rebuilt on load.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorldState {
    pub config: WorldConfig,
    pub tick: u64,
    pub fields: Fields,
    pub weather: Weather,
    pub organisms: Slots<Organism>,
    /// Cumulative counters since world creation.
    pub births_total: u64,
    pub deaths_total: [u64; 3],
    pub cap_rejections_total: u64,
    /// Material admitted from outside (founders and any future stimuli), for the invariant.
    pub external_material_in: f64,
    /// Running energy audit.
    pub light_in_total: f64,
    pub heat_out_total: f64,
    /// Running water budget (`design/water.md`): `Σw == rain_in_total − evap_out_total`
    /// to rounding at every tick of a world created dry.
    #[serde(default)]
    pub rain_in_total: f64,
    #[serde(default)]
    pub evap_out_total: f64,
}

impl WorldState {
    /// Range checks after decode: finite numbers, nonnegative stocks, canonical positions,
    /// unit headings (renormalize if within 1e-6, else invalid), population ≤ cap, escrows
    /// nonnegative, genome fields in range, config valid.
    ///
    /// `validate` only reports; [`World::from_state`] renormalizes headings that are within
    /// tolerance before calling it, so a heading that still fails here is genuinely invalid.
    pub fn validate(&self) -> Result<(), String> {
        self.config.validate()?;
        let cap = self.config.capacity.max_organisms as usize;
        if self.organisms.len() > cap {
            return Err(format!("population {} exceeds cap {cap}", self.organisms.len()));
        }
        for (name, v) in [
            ("external_material_in", self.external_material_in),
            ("light_in_total", self.light_in_total),
            ("heat_out_total", self.heat_out_total),
        ] {
            if !v.is_finite() {
                return Err(format!("{name} is not finite"));
            }
        }
        for (name, v) in [
            ("n", &self.fields.n),
            ("p", &self.fields.p),
            ("d", &self.fields.d),
            ("de", &self.fields.de),
        ] {
            if v.len() != CELL_COUNT {
                return Err(format!("field {name} has {} cells, expected {CELL_COUNT}", v.len()));
            }
        }
        self.fields.check(self.config.detritus.energy_cap)?;

        for blob in self.weather.light.iter().chain(self.weather.moisture.iter()) {
            if !blob.center.iter().chain(blob.axis.iter()).all(|c| c.is_finite()) || !blob.rate.is_finite() {
                return Err("weather blob has non-finite geometry".into());
            }
        }

        for (id, o) in self.organisms.iter() {
            let who = format!("organism {}:{}", id.slot, id.generation);
            for (name, v) in [("structure", o.structure), ("reserve", o.reserve)] {
                if !v.is_finite() || v < 0.0 {
                    return Err(format!("{who}: {name} = {v}"));
                }
            }
            if !o.energy.is_finite() || o.energy < 0.0 {
                return Err(format!("{who}: energy = {}", o.energy));
            }
            if !o.hunger_memory.is_finite() {
                return Err(format!("{who}: hunger memory is not finite"));
            }
            if !o.pos.is_canonical() {
                return Err(format!("{who}: position {:?} is not canonical", o.pos));
            }
            if !o.heading.is_finite() || (o.heading.length() - 1.0).abs() > HEADING_TOLERANCE {
                return Err(format!("{who}: heading {:?} is not a unit vector", o.heading));
            }
            if !o.ou.is_finite() {
                return Err(format!("{who}: OU vector is not finite"));
            }
            if o.born_tick > self.tick {
                return Err(format!("{who}: born at {} after tick {}", o.born_tick, self.tick));
            }
            if let Some(e) = &o.escrow {
                for (name, v) in [("structure", e.structure), ("reserve", e.reserve), ("energy", e.energy)] {
                    if !v.is_finite() || v < 0.0 {
                        return Err(format!("{who}: escrow {name} = {v}"));
                    }
                }
                if e.started_tick > self.tick {
                    return Err(format!("{who}: escrow started at {} after tick {}", e.started_tick, self.tick));
                }
                check_genome(&e.genome, &format!("{who} escrow"))?;
            }
            check_genome(&o.genome, &who)?;
        }
        Ok(())
    }
}

/// How far a stored heading may drift from unit length before it is invalid.
const HEADING_TOLERANCE: f64 = 1e-6;

/// Drift below this is ordinary transport rounding and is left exactly as stored.
const HEADING_REPAIR_FLOOR: f64 = 1e-12;

/// Radius, in pixels, used to unfold the sensed cell centers. Sensing reaches
/// `SENSE_DEPTH_MAX` graph hops (12 px of `sense_radius` at 4 px per cell), so the farthest
/// sensed center is about 14.5 px from any point of the own cell.
const CELL_UNFOLD_RADIUS: f64 = 20.0;

/// The deepest sensing ring the world precomputes: `ceil(12 / 4)` for the largest `sense`
/// the genome allows.
const SENSE_DEPTH_MAX: usize = 3;

/// Draws per birth on `Stream::Birth`: two for placement (direction, heading), then up to
/// six for mutation. Each birth of a parent gets its own block of counters.
const BIRTH_DRAWS: u64 = 16;

/// Largest energy-audit drift tolerated in one tick (`ΔE_total == light_in − heat_out`).
const AUDIT_TOLERANCE: f64 = 1e-9;

/// Gradient length below which the normalized gradient is reported as zero.
const GRADIENT_EPS: f64 = 1e-9;

/// Bounds from `design/m2-world-spec.md` "Organism representation" and `genome`'s docs.
fn check_genome(g: &Genome, who: &str) -> Result<(), String> {
    let range = |name: &str, v: f32, lo: f32, hi: f32| -> Result<(), String> {
        if !v.is_finite() || v < lo || v > hi {
            Err(format!("{who}: genome {name} = {v}, expected [{lo}, {hi}]"))
        } else {
            Ok(())
        }
    };
    if g.version != Genome::VERSION {
        return Err(format!("{who}: genome version {} is not {}", g.version, Genome::VERSION));
    }
    range("size", g.size, 0.5, 2.0)?;
    range("metabolism", g.metabolism, 0.5, 2.0)?;
    range("sense", g.sense, 2.0, 12.0)?;
    range("reserve", g.reserve, 0.5, 2.0)?;
    range("mouth", g.mouth, 0.2, 1.0)?;
    range("speed", g.speed, 0.3, 1.0)?;
    range("hue", g.hue, 0.0, 1.0)?;
    range("diet", g.diet, 0.0, 1.0)?;
    range("depth", g.depth, 0.0, 1.0)?;
    range("swim", g.swim, 0.0, 1.0)?;
    if g.form >= MAX_FORMS {
        return Err(format!("{who}: genome form {} is not below {MAX_FORMS}", g.form));
    }
    let d = &g.drives;
    for (name, v) in [
        ("w_food", d.w_food),
        ("w_detritus", d.w_detritus),
        ("w_persist", d.w_persist),
        ("w_crowd", d.w_crowd),
        ("w_depth", d.w_depth),
        ("turn_noise", d.turn_noise),
    ] {
        range(name, v, 0.0, 2.0)?;
    }
    for (name, v) in [
        ("seek_on", d.seek_on),
        ("seek_off", d.seek_off),
        ("feed_min", d.feed_min),
        ("rest_effort", d.rest_effort),
        ("feed_effort", d.feed_effort),
        ("bud_reserve", d.bud_reserve),
        ("bud_energy", d.bud_energy),
    ] {
        range(name, v, 0.0, 1.0)?;
    }
    if d.seek_off >= d.seek_on {
        return Err(format!("{who}: seek_off {} is not below seek_on {}", d.seek_off, d.seek_on));
    }
    range("tau_hunger_seconds", d.tau_hunger_seconds, 1.0, 60.0)?;
    for (name, v) in [("bud_min_age_seconds", d.bud_min_age_seconds), ("turn_rate_max_deg", d.turn_rate_max_deg)] {
        if !v.is_finite() || v < 0.0 {
            return Err(format!("{who}: genome {name} = {v}"));
        }
    }
    Ok(())
}

/// Per-tick counters exposed to telemetry and reset each sample.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TickCounters {
    pub births: u32,
    pub deaths: [u32; 3],
    pub cap_rejections: u32,
    pub light_in: f64,
    pub heat_out: f64,
    pub rain_in: f64,
    pub evap_out: f64,
    pub travel_fallbacks: u32,
    pub travel_ties: u32,
}

pub struct World {
    pub state: WorldState,
    graph: FieldGraph,
    habitat: Habitat,
    images: [Vec<ChartImage>; 5],
    light: Box<[f64; CELL_COUNT]>,
    moisture: Box<[f64; CELL_COUNT]>,
    /// The moisture weather blob sum per cell, the rain driver (`design/water.md`).
    rain_source: Box<[f64; CELL_COUNT]>,
    /// This tick's rain rate per cell (d/s), for the render view.
    rain: Box<[f32; CELL_COUNT]>,
    scratch: (ScalarField, ScalarField),
    water_scratch: ScalarField,
    /// `sense_rings[cell][d − 1]`: the cells at graph distance exactly `d` from `cell`, for
    /// `d = 1..=SENSE_DEPTH_MAX`. Static for the life of the world.
    sense_rings: Vec<[Vec<CellId>; SENSE_DEPTH_MAX]>,
    neighbors: NeighborLists,
    travel_buf: Travel,
    /// This tick's traveled segments per slot, for the render view.
    moved: Vec<Vec<cubarium_surface::PathSegment>>,
    /// Births and deaths committed since the observer last drained them. Transient: never
    /// checkpointed, never hashed, never read back by the tick.
    events: Vec<LifeEvent>,
    counters: TickCounters,
    initial_material: f64,
}

impl World {
    /// Create a new world: validate config, build habitat/weather, initial fields, and
    /// `founders.count` founders placed uniformly by area (rejection sampling on the five
    /// faces from `Stream::Founders`, key = index, counters 0..3 for face/u/v/heading and
    /// 4 for hue), each with the founder genome, `S = S_adult`,
    /// `R = initial_reserve_fraction · R_max`, `E = initial_energy_fraction · E_max`.
    /// Founder material is recorded in `external_material_in`.
    ///
    /// The five faces have equal area, so a uniform face draw followed by uniform `(u, v)`
    /// is already uniform by area; no rejection is needed.
    pub fn new(config: WorldConfig) -> Result<World, String> {
        config.validate()?;
        let habitat = Habitat::new(&config.habitat, config.seed);
        let weather = Weather::new(&config.weather, config.seed);
        let fields = Fields::new(&config, &habitat.light_base, &habitat.moisture_base);

        let cap = config.capacity.max_organisms;
        let mut organisms = Slots::with_capacity(cap as usize);
        let mut external_material_in = 0.0;
        // The founder roster: `(draw key, kind)` per founder. With kinds, the kind index is
        // folded into the high bits of the key so each kind's draws are their own stream and
        // a seed stays reproducible when another kind's count changes; without kinds the
        // v1 path keeps `key = index`.
        let roster: Vec<(u64, Option<&FounderKind>)> = if config.founders.kinds.is_empty() {
            (0..u64::from(config.founders.count)).map(|i| (i, None)).collect()
        } else {
            config
                .founders
                .kinds
                .iter()
                .enumerate()
                .flat_map(|(k, kind)| (0..u64::from(kind.count)).map(move |j| (((k as u64) << 32) | j, Some(kind))))
                .collect()
        };
        for (index, kind) in roster.into_iter().take(cap as usize) {
            let seed = config.seed;
            let face_index = (unit(seed, Stream::Founders, index, 0) * 5.0).floor();
            let face = Face::from_index(face_index as u8).unwrap_or(Face::Front);
            let u = unit(seed, Stream::Founders, index, 1) * FACE_EXTENT;
            let v = unit(seed, Stream::Founders, index, 2) * FACE_EXTENT;
            let heading = Vec2::from_screen_angle(unit(seed, Stream::Founders, index, 3) * TAU);
            let hue = unit(seed, Stream::Founders, index, 4) as f32;

            let mut genome = Genome::founder(kind.and_then(|k| k.hue).unwrap_or(hue), &config.drives);
            // Founders sense at the configured radius; the genome bounds still apply.
            genome.sense = config.organism.sense_radius as f32;
            if let Some(k) = kind {
                // A kind fixes the loci it names; the rest keep the v1 founder values. The
                // rig follows the kind, or the hue tercile when the kind leaves it open.
                if let Some(x) = k.diet { genome.diet = x; }
                if let Some(x) = k.depth { genome.depth = x; }
                if let Some(x) = k.speed { genome.speed = x; }
                if let Some(x) = k.size { genome.size = x; }
                if let Some(x) = k.metabolism { genome.metabolism = x; }
                if let Some(x) = k.swim { genome.swim = x; }
                if let Some(x) = k.form { genome.form = x; } else { genome.form = crate::genome::form_of_hue(genome.hue); }
            }
            genome.clamp();
            let phenotype = decode(&genome, &config.organism);
            let structure = phenotype.structure_adult;
            let reserve = config.founders.initial_reserve_fraction * phenotype.reserve_max;
            let energy = config.founders.initial_energy_fraction * phenotype.energy_max;
            external_material_in += structure + reserve;
            organisms.insert(Organism {
                pos: SurfacePoint::new(face, u, v).canonicalize(),
                heading,
                ou: Vec2::ZERO,
                structure,
                reserve,
                energy,
                born_tick: 0,
                hunger_memory: (1.0 - reserve / phenotype.reserve_max).clamp(0.0, 1.0),
                mode: Mode::Resting,
                escrow: None,
                births: 0,
                genome,
                phenotype,
                parent: None,
                origin: Origin::Founder,
                turn_counter: Counter::default(),
                fed_this_tick: false,
            });
        }

        // The residual is measured against the field material present at creation; the
        // founders arrive from outside and are booked in `external_material_in`.
        let initial_material = fields.total_material();
        let state = WorldState {
            config,
            tick: 0,
            fields,
            weather,
            organisms,
            births_total: 0,
            deaths_total: [0; 3],
            cap_rejections_total: 0,
            external_material_in,
            light_in_total: 0.0,
            heat_out_total: 0.0,
            rain_in_total: 0.0,
            evap_out_total: 0.0,
        };
        Ok(World::assemble(state, habitat, initial_material))
    }

    /// Rebuild caches around a validated state (after a snapshot load).
    ///
    /// The residual baseline is re-derived so that it reads zero at load and reports drift
    /// since the load; a snapshot does not carry the pre-load residual.
    pub fn from_state(mut state: WorldState) -> Result<World, String> {
        for (_, o) in state.organisms.iter_mut() {
            // A genome written before fauna v2 is brought to version 2 in place, escrow
            // included; the phenotype is re-decoded so the new loci take effect.
            let upgraded = o.genome.upgrade();
            if let Some(e) = &mut o.escrow {
                e.genome.upgrade();
            }
            if upgraded {
                o.phenotype = decode(&o.genome, &state.config.organism);
            }
            // Repair a heading that drifted, but never touch one that is already unit to
            // rounding: renormalizing it would change the state bit-for-bit and a reloaded
            // world would no longer replay identically to the one that wrote the snapshot.
            let len = o.heading.length();
            if (len - 1.0).abs() > HEADING_REPAIR_FLOOR && (len - 1.0).abs() <= HEADING_TOLERANCE {
                o.heading = o.heading * (1.0 / len);
            }
        }
        state.validate()?;
        let habitat = Habitat::new(&state.config.habitat, state.config.seed);
        let organism_material: f64 = state.organisms.iter().map(|(_, o)| o.material()).sum();
        let initial_material = state.fields.total_material() + organism_material - state.external_material_in;
        Ok(World::assemble(state, habitat, initial_material))
    }

    fn assemble(state: WorldState, habitat: Habitat, initial_material: f64) -> World {
        let mut world = World {
            state,
            graph: FieldGraph::new(),
            habitat,
            images: std::array::from_fn(|i| {
                let mut v = Vec::new();
                chart_images(Face::from_index(i as u8).expect("five faces"), MAX_SEAMS, &mut v);
                v
            }),
            light: Box::new([0.0; CELL_COUNT]),
            moisture: Box::new([0.0; CELL_COUNT]),
            rain_source: Box::new([0.0; CELL_COUNT]),
            rain: Box::new([0.0; CELL_COUNT]),
            scratch: (ScalarField::zeros(), ScalarField::zeros()),
            water_scratch: ScalarField::zeros(),
            sense_rings: Vec::new(),
            neighbors: NeighborLists::default(),
            travel_buf: Travel::default(),
            moved: Vec::new(),
            events: Vec::new(),
            counters: TickCounters::default(),
            initial_material,
        };
        // Make the derived light/moisture readable before the first tick advances weather.
        let cfg = &world.state.config;
        world.state.weather.sample(
            &cfg.weather,
            &world.habitat,
            &mut world.light,
            &mut world.moisture,
            &mut world.rain_source,
            cfg.habitat.moisture_min,
        );
        world.moved.resize_with(world.state.organisms.slot_count(), Vec::new);
        world.sense_rings = sense_rings(&world.graph);
        world
    }

    /// One tick in the normative order of `design/m2-world-spec.md` "Tick order".
    /// Returns the per-tick counters (also accumulated internally for telemetry).
    pub fn step(&mut self) -> &TickCounters {
        let dt = DT;
        #[cfg(debug_assertions)]
        let audit = (stored_energy(&self.state), self.state.light_in_total, self.state.heat_out_total);
        #[cfg(debug_assertions)]
        let water_audit = (
            self.state.fields.w.iter().sum::<f64>(),
            self.state.rain_in_total,
            self.state.evap_out_total,
        );
        {
            let World {
                state,
                graph,
                habitat,
                images,
                light,
                moisture,
                rain_source,
                rain,
                scratch,
                water_scratch,
                sense_rings,
                neighbors,
                travel_buf,
                moved,
                events,
                counters,
                initial_material: _,
            } = &mut *self;
            let WorldState {
                config,
                tick,
                fields,
                weather,
                organisms,
                births_total,
                deaths_total,
                cap_rejections_total,
                external_material_in: _,
                light_in_total,
                heat_out_total,
                rain_in_total,
                evap_out_total,
            } = state;
            let cfg: &WorldConfig = config;
            let org_cfg = &cfg.organism;
            let e_r = org_cfg.reserve_energy_density;
            let k_p = org_cfg.intake_half_saturation;
            let e_f = cfg.fruit.energy_density;
            let gate = TurnGate::from_config(org_cfg);
            let now = *tick;
            let seed = cfg.seed;
            let mut heat = |amount: f64| {
                counters.heat_out += amount;
                *heat_out_total += amount;
            };

            // 1. Admit due stimuli. The queue is empty in M2; the hook is the journal.

            // 2. Weather advance, then derive light, moisture and the rain source per cell.
            weather.advance(&cfg.weather, seed, now);
            weather.sample(&cfg.weather, habitat, light, moisture, rain_source, cfg.habitat.moisture_min);

            // 2b. Water: rain, downhill flow, evaporation (`design/water.md`). Before the
            //     field reactions, so growth sees this tick's wetness and flooding.
            let water_ledger = water::step(
                &mut fields.w,
                &cfg.water,
                water::Drivers { terrain: &habitat.terrain, light, rain_source },
                rain,
                graph,
                water_scratch,
            );
            counters.rain_in += water_ledger.rain_in;
            counters.evap_out += water_ledger.evap_out;
            *rain_in_total += water_ledger.rain_in;
            *evap_out_total += water_ledger.evap_out;

            // 3. Field reactions (growth, mortality, decomposition, N diffusion).
            let ledger = fields.react(cfg, light, moisture, graph, scratch);
            counters.light_in += ledger.light_in;
            *light_in_total += ledger.light_in;
            heat(ledger.heat_out);

            // 4. Pair pass.
            let mut bodies: Vec<Body> = Vec::with_capacity(organisms.len());
            for (id, o) in organisms.iter() {
                bodies.push(Body { id, pos: o.pos, sense_radius: o.phenotype.sense_radius, extent: o.phenotype.extent });
            }
            pairs::build(&bodies, images, cfg.capacity.max_neighbors as usize, neighbors);

            // 5. Observe and decide (pure per organism, from the pre-movement world).
            let mut decisions: Vec<(OrganismId, Decision)> = Vec::with_capacity(organisms.len());
            for (id, o) in organisms.iter_mut() {
                let cell = cell_of(&o.pos);
                let here = cell.index();
                let chart = o.pos.chart();
                let mut obs = Observation {
                    p_here: fields.p[here],
                    f_here: fields.f[here],
                    d_here: edible_detritus(fields.d[here], fields.de[here], e_r),
                    height: o.pos.embed()[1],
                    up: up_direction(o.pos.face),
                    ..Observation::default()
                };
                // Sensing reaches `ceil(r_sense / 4)` graph hops (`design/fauna-v2.md`): each
                // sensed cell contributes its finite-difference slope toward it.
                let depth = sense_depth(o.phenotype.sense_radius);
                for ring in &sense_rings[here][..depth] {
                    for neighbor in ring {
                        let center = neighbor.center();
                        let Some(view) = unfold_with(&images[o.pos.face.index()], o.pos, center, CELL_UNFOLD_RADIUS) else {
                            continue;
                        };
                        let Some(dir) = (view.local - chart).normalized() else {
                            continue;
                        };
                        if view.distance <= GRADIENT_EPS || view.distance.is_nan() {
                            continue;
                        }
                        let slope = dir * (1.0 / view.distance);
                        let there = neighbor.index();
                        let edible = edible_detritus(fields.d[there], fields.de[there], e_r);
                        obs.grad_p += slope * (fields.p[there] - obs.p_here);
                        obs.grad_f += slope * (fields.f[there] - obs.f_here);
                        obs.grad_d += slope * (edible - obs.d_here);
                    }
                }
                obs.grad_p = normalize_or_zero(obs.grad_p);
                obs.grad_f = normalize_or_zero(obs.grad_f);
                obs.grad_d = normalize_or_zero(obs.grad_d);

                if let Some(list) = neighbors.lists.get(id.slot as usize) {
                    for n in list {
                        let extent_sum = o.phenotype.extent + n.extent;
                        if n.distance >= extent_sum + 1.0 {
                            continue;
                        }
                        let delta = chart - n.local;
                        let len_sq = delta.length_sq();
                        if len_sq > GRADIENT_EPS {
                            obs.repulsion += delta * (extent_sum / len_sq);
                        }
                    }
                }

                // Two standard normals cost two counters each.
                let cx = o.turn_counter.take();
                let _ = o.turn_counter.take();
                let cy = o.turn_counter.take();
                let _ = o.turn_counter.take();
                let key = u64::from(id.slot);
                obs.noise = Vec2::new(
                    normal(seed, Stream::OrganismTurn, key, cx),
                    normal(seed, Stream::OrganismTurn, key, cy),
                );

                decisions.push((id, decide(o, &obs, now, dt, cfg.mechanisms.grazing, cfg.mechanisms.scavenging, gate)));
            }

            // 6. Move, transport tangents, and pay for motion, sensing and maintenance.
            moved.resize_with(organisms.slot_count(), Vec::new);
            for segments in moved.iter_mut() {
                segments.clear();
            }
            for (id, d) in &decisions {
                let Some(o) = organisms.get_mut(*id) else { continue };
                o.mode = d.mode;
                o.hunger_memory = d.hunger_memory;
                o.fed_this_tick = false;
                // Wading (`design/water.md`, `design/fauna-v2.md`): speed is divided by
                // `1 + w · (1 − swim)` of the cell the organism stands in before it moves; a
                // swimmer ignores the pool.
                let wading = 1.0 + fields.w[cell_of(&o.pos).index()] * (1.0 - o.phenotype.swim);
                let speed = d.effort * o.phenotype.speed_max / wading;
                travel_into(o.pos, d.heading * (speed * dt), travel_buf);
                o.pos = travel_buf.end;
                o.heading = travel_buf.map.apply(d.heading).normalized().unwrap_or(d.heading);
                o.ou = travel_buf.map.apply(d.ou);
                counters.travel_ties += travel_buf.ties;
                counters.travel_fallbacks += u32::from(travel_buf.fallback);
                if let Some(segments) = moved.get_mut(id.slot as usize) {
                    segments.extend_from_slice(&travel_buf.segments);
                }
                // `paid = min(cost · dt, E)`: an organism that cannot pay simply runs out.
                let cost = (o.phenotype.maintenance * o.structure
                    + org_cfg.move_cost * o.structure * speed
                    + org_cfg.sense_cost * o.phenotype.sense_radius)
                    * dt;
                let paid = cost.min(o.energy).max(0.0);
                o.energy -= paid;
                heat(paid);
            }

            // 7. Settle feeding once per cell, proportionally, from the pre-transfer fields.
            let mut fruit = vec![0.0f64; CELL_COUNT];
            let mut graze = vec![0.0f64; CELL_COUNT];
            let mut scavenge = vec![0.0f64; CELL_COUNT];
            let mut detritus_energy_density = vec![0.0f64; CELL_COUNT];
            let mut requests: Vec<(usize, OrganismId, f64, f64, f64)> = Vec::new();
            for (id, d) in &decisions {
                if d.fruit_effort <= 0.0 && d.graze_effort <= 0.0 && d.scavenge_effort <= 0.0 {
                    continue;
                }
                let Some(o) = organisms.get(*id) else { continue };
                let cell = cell_of(&o.pos).index();
                let headroom = (o.phenotype.reserve_max - o.reserve).max(0.0);
                // Type-II intake: what a mouth can take falls off as the cell empties, so a
                // poor cell is poor food even to an organism standing in it. `K_P = 0` gives
                // back the linear law exactly. The rate is the diet's: `graze_rate` on
                // producer and fruit, `scavenge_rate` on detritus (`design/fauna-v2.md`).
                let bite = |rate: f64, effort: f64, room: f64, food: f64| {
                    if effort <= 0.0 {
                        return 0.0;
                    }
                    let total = food + k_p;
                    let saturation = if total > 0.0 { food / total } else { 0.0 };
                    (rate * effort * dt * saturation).clamp(0.0, room.max(0.0))
                };
                // Fruit settles first and takes its headroom, grazing next, scavenging
                // gets the rest, so intake alone can never push the reserve past `R_max`.
                // All read the cell's pre-settlement stock.
                let f = bite(o.phenotype.graze_rate, d.fruit_effort, headroom, fields.f[cell]);
                let g = bite(o.phenotype.graze_rate, d.graze_effort, headroom - f, fields.p[cell]);
                let s = bite(
                    o.phenotype.scavenge_rate,
                    d.scavenge_effort,
                    headroom - f - g,
                    edible_detritus(fields.d[cell], fields.de[cell], e_r),
                );
                if f <= 0.0 && g <= 0.0 && s <= 0.0 {
                    continue;
                }
                fruit[cell] += f;
                graze[cell] += g;
                scavenge[cell] += s;
                requests.push((cell, *id, f, g, s));
            }
            // Turn the per-cell request sums into proportional shares, and capture the
            // detritus energy density, all before a single transfer is applied.
            let mut contested: Vec<usize> = requests.iter().map(|r| r.0).collect();
            contested.sort_unstable();
            contested.dedup();
            for &cell in &contested {
                let (available_f, available_p, available_d) = (fields.f[cell], fields.p[cell], fields.d[cell]);
                fruit[cell] = share(fruit[cell], available_f);
                graze[cell] = share(graze[cell], available_p);
                scavenge[cell] = share(scavenge[cell], available_d);
                detritus_energy_density[cell] = if available_d > 0.0 { fields.de[cell] / available_d } else { 0.0 };
            }

            let eta_m = org_cfg.assimilation_material;
            let eta_e = org_cfg.assimilation_energy;
            let e_p = cfg.producer.energy_density;
            for &(cell, id, f, g, s) in &requests {
                let Some(o) = organisms.get_mut(id) else { continue };
                let mut eaten = 0.0;
                if f > 0.0 {
                    // Frugivory: F -> reserve (η_m) and F -> D (the rest, energy-free). Fruit
                    // carries `e_f` per unit, richer than leaf.
                    let q = (f * fruit[cell]).clamp(0.0, fields.f[cell]);
                    if q > 0.0 {
                        let to_reserve = eta_m * q;
                        fields.f[cell] -= q;
                        fields.d[cell] += q - to_reserve;
                        o.reserve += to_reserve;
                        let spare = (e_f - e_r * eta_m) * q;
                        let room = (o.phenotype.energy_max - o.energy).max(0.0);
                        let gained = (eta_e * spare).clamp(0.0, room);
                        o.energy += gained;
                        heat(spare - gained);
                        eaten += q;
                    }
                }
                if g > 0.0 {
                    // Grazing: P -> reserve (η_m) and P -> D (the rest, energy-free).
                    let q = (g * graze[cell]).clamp(0.0, fields.p[cell]);
                    if q > 0.0 {
                        let to_reserve = eta_m * q;
                        fields.p[cell] -= q;
                        fields.d[cell] += q - to_reserve;
                        o.reserve += to_reserve;
                        // The food carried `e_p · q`; `e_r · η_m · q` of it is now stored in
                        // the reserve, and `η_e` of the difference is usable energy.
                        let spare = (e_p - e_r * eta_m) * q;
                        let room = (o.phenotype.energy_max - o.energy).max(0.0);
                        let gained = (eta_e * spare).clamp(0.0, room);
                        o.energy += gained;
                        heat(spare - gained);
                        eaten += q;
                    }
                }
                if s > 0.0 {
                    // Scavenging: poor detritus assimilates proportionally less material,
                    // so the reserve is never credited with energy the food did not hold.
                    let rho = detritus_energy_density[cell];
                    let eta = if e_r > 0.0 { eta_m * (rho / e_r).min(1.0) } else { eta_m };
                    let q = (s * scavenge[cell]).clamp(0.0, fields.d[cell]);
                    if q > 0.0 && eta > 0.0 {
                        let to_reserve = eta * q;
                        fields.d[cell] -= to_reserve;
                        o.reserve += to_reserve;
                        let carried = (rho * q).min(fields.de[cell]);
                        fields.de[cell] -= carried;
                        let spare = carried - e_r * to_reserve;
                        let room = (o.phenotype.energy_max - o.energy).max(0.0);
                        let gained = (eta_e * spare).clamp(0.0, room);
                        o.energy += gained;
                        heat(spare - gained);
                        eaten += q;
                    }
                }
                o.fed_this_tick = eaten > 0.0;
            }

            // 8. Physiology: oxidation, growth, gestation, budding, death checks.
            let gestation_ticks = ticks_from_seconds(org_cfg.gestation_seconds, dt);
            let max_age_ticks = ticks_from_seconds(org_cfg.max_age_seconds, dt);
            let cap = cfg.capacity.max_organisms as usize;
            let population = organisms.len();
            let mut births: Vec<OrganismId> = Vec::new();
            let mut deaths: Vec<(OrganismId, DeathCause)> = Vec::new();
            for (id, d) in &decisions {
                let Some(o) = organisms.get_mut(*id) else { continue };

                if o.energy < org_cfg.oxidation_threshold * o.phenotype.energy_max && o.reserve > 0.0 {
                    let burned = (org_cfg.oxidation_rate * dt).min(o.reserve);
                    o.reserve -= burned;
                    fields.n[cell_of(&o.pos).index()] += burned;
                    // The reserve material carried `e_r` per unit; `η_ox` of it becomes usable.
                    let released = e_r * burned;
                    let room = (o.phenotype.energy_max - o.energy).max(0.0);
                    let gained = (released * org_cfg.oxidation_efficiency).min(room);
                    o.energy += gained;
                    heat(released - gained);
                }

                if o.structure < o.phenotype.structure_adult
                    && o.reserve > org_cfg.growth_reserve_min * o.phenotype.reserve_max
                {
                    let mut grown =
                        (org_cfg.growth_rate * dt).min(o.phenotype.structure_adult - o.structure).min(o.reserve);
                    // Building is paid for up front: what the energy cannot cover is not built.
                    if org_cfg.build_cost > 0.0 {
                        grown = grown.min(o.energy / org_cfg.build_cost);
                    }
                    if grown > 0.0 {
                        o.reserve -= grown;
                        o.structure += grown;
                        let cost = (org_cfg.build_cost * grown).min(o.energy);
                        o.energy -= cost;
                        // Structure holds no chemical energy: the reserve's energy is released.
                        heat(cost + e_r * grown);
                    }
                }

                let due = o.escrow.as_ref().is_some_and(|e| now.saturating_sub(e.started_tick) >= gestation_ticks);
                if due {
                    births.push(*id);
                } else if d.bud && o.escrow.is_none() {
                    if population + births.len() < cap {
                        let structure = org_cfg.child_structure_fraction * o.phenotype.structure_adult;
                        let reserve = org_cfg.child_reserve_fraction * o.phenotype.reserve_max;
                        let energy = org_cfg.child_energy_fraction * o.phenotype.energy_max;
                        let build = org_cfg.build_cost * structure;
                        if o.reserve >= structure + reserve && o.energy >= build + energy {
                            o.reserve -= structure + reserve;
                            o.energy -= build + energy;
                            heat(build);
                            let genome = o.genome.clone();
                            o.escrow = Some(Escrow { structure, reserve, energy, started_tick: now, genome });
                        }
                    } else {
                        counters.cap_rejections += 1;
                        *cap_rejections_total += 1;
                    }
                }

                let cause = if o.energy <= 0.0 && o.reserve <= 0.0 {
                    Some(DeathCause::Starvation)
                } else if o.age_ticks(now) >= max_age_ticks {
                    Some(DeathCause::Age)
                } else if o.structure < org_cfg.min_structure {
                    Some(DeathCause::Collapse)
                } else {
                    None
                };
                if let Some(cause) = cause {
                    deaths.push((*id, cause));
                    // A parent that dies this tick miscarries: the escrow goes to detritus.
                    births.retain(|b| b != id);
                }
            }

            // 9. Commit: remove the dead, then place births against the freed capacity.
            let e_d_max = cfg.detritus.energy_cap;
            for (id, cause) in &deaths {
                let Some(o) = organisms.remove(*id) else { continue };
                let cell = cell_of(&o.pos).index();
                // The body: structure carries no energy, the reserve carries `e_r` per unit.
                let material = o.structure + o.reserve;
                let energy = o.energy + e_r * o.reserve;
                fields.d[cell] += material;
                let kept = energy.min(e_d_max * material);
                fields.de[cell] += kept;
                heat(energy - kept);
                // A gestation that never finished decays with its own clamp.
                if let Some(es) = &o.escrow {
                    let material = es.structure + es.reserve;
                    let energy = e_r * material + es.energy;
                    fields.d[cell] += material;
                    let kept = energy.min(e_d_max * material);
                    fields.de[cell] += kept;
                    heat(energy - kept);
                }
                let slot = match cause {
                    DeathCause::Starvation => 0,
                    DeathCause::Age => 1,
                    DeathCause::Collapse => 2,
                };
                counters.deaths[slot] += 1;
                deaths_total[slot] += 1;
                events.push(LifeEvent::Death {
                    tick: now + 1,
                    id: *id,
                    age_ticks: o.age_ticks(now + 1),
                    cause: *cause,
                    births: o.births,
                    genome: o.genome.digest(),
                });
            }

            for parent_id in &births {
                let full = organisms.len() >= cap;
                let placement = {
                    let Some(parent) = organisms.get_mut(*parent_id) else { continue };
                    let Some(escrow) = parent.escrow.take() else { continue };
                    if full {
                        // A refused birth returns its escrow to the parent untouched.
                        parent.reserve += escrow.structure + escrow.reserve;
                        parent.energy += escrow.energy;
                        counters.cap_rejections += 1;
                        *cap_rejections_total += 1;
                        continue;
                    }
                    let index = u64::from(parent.births);
                    parent.births += 1;
                    let key = u64::from(parent_id.slot);
                    // Each birth owns a block of `BIRTH_DRAWS` counters: placement first,
                    // then mutation, so neither can collide with the next birth's draws.
                    let base = index * BIRTH_DRAWS;
                    let direction = Vec2::from_screen_angle(unit(seed, Stream::Birth, key, base) * TAU);
                    let heading = Vec2::from_screen_angle(unit(seed, Stream::Birth, key, base + 1) * TAU);
                    (parent.pos, direction, heading, escrow, parent.age_ticks(now + 1), parent.births, key, base + 2)
                };
                let (from, direction, heading, escrow, parent_age_ticks, parent_births, key, mut counter) = placement;
                travel_into(from, direction * cfg.drives.birth_offset_px, travel_buf);
                counters.travel_ties += travel_buf.ties;
                counters.travel_fallbacks += u32::from(travel_buf.fallback);
                let heading = travel_buf.map.apply(heading).normalized().unwrap_or(Vec2::new(1.0, 0.0));
                let mut genome = escrow.genome.clone();
                // Sparse mutation (`design/fauna-v2.md`): the draws follow the placement draws
                // in the parent's birth stream; `form` never changes; an exact copy records
                // nothing.
                let mutations = if cfg.mechanisms.mutation {
                    let draws = genome.mutate(cfg.mutation.probability, cfg.mutation.step, || {
                        let u = unit(seed, Stream::Birth, key, counter);
                        counter += 1;
                        u
                    });
                    genome.clamp();
                    draws
                } else {
                    Vec::new()
                };
                let phenotype = decode(&genome, org_cfg);
                // The escrowed material that becomes structure gives up its reserve energy.
                heat(e_r * escrow.structure);
                let child = Organism {
                    pos: travel_buf.end,
                    heading,
                    ou: Vec2::ZERO,
                    structure: escrow.structure,
                    reserve: escrow.reserve,
                    energy: escrow.energy,
                    born_tick: now + 1,
                    hunger_memory: (1.0 - escrow.reserve / phenotype.reserve_max).clamp(0.0, 1.0),
                    mode: Mode::Resting,
                    escrow: None,
                    births: 0,
                    genome,
                    phenotype,
                    parent: Some(*parent_id),
                    origin: Origin::Descendant,
                    turn_counter: Counter::default(),
                    fed_this_tick: false,
                };
                let genome_digest = child.genome.digest();
                let origin = child.origin;
                let child_id = organisms.insert(child);
                events.push(LifeEvent::Birth {
                    tick: now + 1,
                    id: child_id,
                    parent: *parent_id,
                    parent_age_ticks,
                    parent_births,
                    genome: genome_digest,
                    origin,
                    mutations,
                });
                let slot = child_id.slot as usize;
                if moved.len() <= slot {
                    moved.resize_with(slot + 1, Vec::new);
                }
                moved[slot].clear();
                counters.births += 1;
                *births_total += 1;
            }

            *tick += 1;
        }

        // 10. Invariants; the render view and telemetry are pulled by the host.
        #[cfg(debug_assertions)]
        {
            if let Err(e) = self.check_invariants() {
                panic!("invariant violated after tick {}: {e}", self.state.tick);
            }
            // The energy audit is an exact identity: every joule is light, heat, or stored.
            let (before, light, heat) = audit;
            let booked = (self.state.light_in_total - light) - (self.state.heat_out_total - heat);
            let drift = (stored_energy(&self.state) - before) - booked;
            assert!(drift.abs() < AUDIT_TOLERANCE, "energy audit drifted by {drift:e} in tick {}", self.state.tick);
            // The water budget is the same kind of identity: `Δ Σw == rain_in − evap_out`.
            let (w_before, rain, evap) = water_audit;
            let w_booked = (self.state.rain_in_total - rain) - (self.state.evap_out_total - evap);
            let w_drift = (self.state.fields.w.iter().sum::<f64>() - w_before) - w_booked;
            assert!(w_drift.abs() < AUDIT_TOLERANCE, "water budget drifted by {w_drift:e} in tick {}", self.state.tick);
        }
        &self.counters
    }

    /// Water budget residual (`design/water.md`): `Σw − (rain_in_total − evap_out_total)`.
    /// Zero to rounding for a world created dry; telemetry consumers check it like the
    /// mass residual.
    pub fn water_residual(&self) -> f64 {
        self.state.fields.w.iter().sum::<f64>() - (self.state.rain_in_total - self.state.evap_out_total)
    }

    /// Mass invariant: `Σ fields + Σ organisms (incl. escrow) − external_material_in − initial`.
    /// Should stay within `1e-9 · initial` per hour of simulated time; telemetry reports it.
    pub fn mass_residual(&self) -> f64 {
        let organisms: f64 = self.state.organisms.iter().map(|(_, o)| o.material()).sum();
        self.state.fields.total_material() + organisms - self.state.external_material_in - self.initial_material
    }

    /// Full invariant check (fields finite/nonnegative, organisms finite, positions canonical,
    /// escrows consistent, population ≤ cap). Called every tick in debug builds and every
    /// telemetry sample in release; a failure is a fatal implementation error.
    pub fn check_invariants(&self) -> Result<(), String> {
        let cfg = &self.state.config;
        self.state.fields.check(cfg.detritus.energy_cap)?;
        let cap = cfg.capacity.max_organisms as usize;
        if self.state.organisms.len() > cap {
            return Err(format!("population {} exceeds cap {cap}", self.state.organisms.len()));
        }
        for (id, o) in self.state.organisms.iter() {
            let who = format!("organism {}:{}", id.slot, id.generation);
            if !o.structure.is_finite() || o.structure < 0.0 {
                return Err(format!("{who}: structure = {}", o.structure));
            }
            if !o.reserve.is_finite() || o.reserve < 0.0 {
                return Err(format!("{who}: reserve = {}", o.reserve));
            }
            if !o.energy.is_finite() || o.energy < 0.0 {
                return Err(format!("{who}: energy = {}", o.energy));
            }
            if !o.hunger_memory.is_finite() {
                return Err(format!("{who}: hunger memory is not finite"));
            }
            if !o.pos.is_canonical() {
                return Err(format!("{who}: position {:?} is not canonical", o.pos));
            }
            if !o.heading.is_finite() || (o.heading.length() - 1.0).abs() > HEADING_TOLERANCE {
                return Err(format!("{who}: heading {:?} is not a unit vector", o.heading));
            }
            if !o.ou.is_finite() {
                return Err(format!("{who}: OU vector is not finite"));
            }
            if let Some(e) = &o.escrow {
                if !(e.structure.is_finite() && e.reserve.is_finite() && e.energy.is_finite()) {
                    return Err(format!("{who}: escrow is not finite"));
                }
                if e.structure < 0.0 || e.reserve < 0.0 || e.energy < 0.0 {
                    return Err(format!("{who}: escrow is negative"));
                }
                if e.started_tick > self.state.tick {
                    return Err(format!("{who}: escrow starts after the current tick"));
                }
            }
        }
        Ok(())
    }

    pub fn render_view(&self) -> RenderView {
        // The same derivation the tick uses for the escrow's due date, so what a renderer
        // shows as "nearly born" is the tick the world will actually commit the birth on.
        let gestation_ticks = ticks_from_seconds(self.state.config.organism.gestation_seconds, DT);
        let tick = self.state.tick;
        let organisms = self
            .state
            .organisms
            .iter()
            .map(|(id, o)| OrganismView {
                id,
                pos: o.pos,
                heading: o.heading,
                lobes: o.phenotype.lobes.clone(),
                hue: o.phenotype.hue,
                mode: o.mode,
                fed: o.fed_this_tick,
                juvenile: o.structure < 0.7 * o.phenotype.structure_adult,
                form: o.phenotype.form,
                gestation: o.escrow.as_ref().map(|e| {
                    // A zero-tick gestation (a config that rounds below one tick) is due
                    // the moment it starts, so it reads as complete rather than as NaN.
                    if gestation_ticks == 0 {
                        1.0
                    } else {
                        (tick.saturating_sub(e.started_tick) as f64 / gestation_ticks as f64)
                            .clamp(0.0, 1.0) as f32
                    }
                }),
                moved: self.moved.get(id.slot as usize).cloned().unwrap_or_default(),
            })
            .collect();
        RenderView {
            tick: self.state.tick,
            producer: self.state.fields.p.clone(),
            detritus: self.state.fields.d.clone(),
            fruit: self.state.fields.f.clone(),
            water: self.state.fields.w.clone(),
            rain: self.rain.to_vec(),
            producer_max: self.state.config.producer.max,
            organisms,
        }
    }

    /// Take the births and deaths committed since the last drain, in commit order (a tick's
    /// deaths before its births, each in slot order). The buffer is transient: nothing in the
    /// world reads it back, and a host that never drains it simply lets it grow.
    pub fn drain_events(&mut self) -> Vec<LifeEvent> {
        std::mem::take(&mut self.events)
    }

    /// Every cell's material fields plus its live organism count, for the observer's
    /// field dump. Cells are in `CellId` index order, 1,280 entries each.
    pub fn field_dump(&self) -> FieldDump {
        let mut organisms = vec![0u16; CELL_COUNT];
        for (_, o) in self.state.organisms.iter() {
            organisms[cell_of(&o.pos).index()] += 1;
        }
        let fields = &self.state.fields;
        FieldDump {
            tick: self.state.tick,
            n: fields.n.clone(),
            p: fields.p.clone(),
            d: fields.d.clone(),
            de: fields.de.clone(),
            w: fields.w.clone(),
            f: fields.f.clone(),
            organisms,
        }
    }

    /// Each cell's four graph neighbors in `Edge` order (`Top, Right, Bottom, Left`), as raw
    /// cell indices with `None` at the open rim. Static for the life of the world: an
    /// analyzer reads it once and computes graph distances without this crate.
    pub fn cell_neighbors(&self) -> Vec<[Option<u16>; 4]> {
        CellId::all()
            .map(|cell| {
                let n = self.graph.neighbors(cell);
                std::array::from_fn(|e| n[e].map(|c| c.0))
            })
            .collect()
    }

    /// Produce a telemetry sample and reset the per-sample counters.
    pub fn telemetry(&mut self) -> Telemetry {
        let mass_residual = self.mass_residual();
        let mut population_by_face = [0u32; 5];
        let mut occupied = vec![false; CELL_COUNT];
        let mut occupied_cells = 0;
        let mut escrows = 0;
        let (mut organism_material, mut organism_energy) = (0.0, 0.0);
        let (mut mode_resting, mut mode_seeking, mut mode_feeding) = (0, 0, 0);
        let mut population_by_form = [0u32; 8];
        let mut height_by_form = [0.0f64; 8];
        for (_, o) in self.state.organisms.iter() {
            population_by_face[o.pos.face.index()] += 1;
            let form = (o.phenotype.form as usize).min(7);
            population_by_form[form] += 1;
            height_by_form[form] += o.pos.embed()[1];
            let cell = cell_of(&o.pos).index();
            if !occupied[cell] {
                occupied[cell] = true;
                occupied_cells += 1;
            }
            if o.escrow.is_some() {
                escrows += 1;
            }
            organism_material += o.material();
            organism_energy += o.energy;
            match o.mode {
                Mode::Resting => mode_resting += 1,
                Mode::Seeking => mode_seeking += 1,
                Mode::Feeding => mode_feeding += 1,
            }
        }
        let fields = &self.state.fields;
        let mut producer_by_face = [0.0f64; 5];
        let mut detritus_by_face = [0.0f64; 5];
        let mut water_by_face = [0.0f64; 5];
        for cell in CellId::all() {
            let face = cell.face().index();
            producer_by_face[face] += fields.p[cell.index()];
            detritus_by_face[face] += fields.d[cell.index()];
            water_by_face[face] += fields.w[cell.index()];
        }
        let mean_height_by_form =
            std::array::from_fn(|i| if population_by_form[i] > 0 { height_by_form[i] / f64::from(population_by_form[i]) } else { 0.0 });
        let sample = Telemetry {
            tick: self.state.tick,
            population: self.state.organisms.len() as u32,
            births: self.counters.births,
            deaths_starvation: self.counters.deaths[0],
            deaths_age: self.counters.deaths[1],
            deaths_collapse: self.counters.deaths[2],
            escrows,
            cap_rejections: self.counters.cap_rejections,
            nutrient: fields.n.iter().sum(),
            producer: fields.p.iter().sum(),
            detritus: fields.d.iter().sum(),
            detritus_energy: fields.de.iter().sum(),
            fruit: fields.f.iter().sum(),
            organism_material,
            organism_energy,
            light_in: self.counters.light_in,
            heat_out: self.counters.heat_out,
            mass_residual,
            population_by_face,
            producer_by_face,
            detritus_by_face,
            water: fields.w.iter().sum(),
            water_by_face,
            rain_in: self.counters.rain_in,
            evap_out: self.counters.evap_out,
            occupied_cells,
            travel_fallbacks: self.counters.travel_fallbacks,
            travel_ties: self.counters.travel_ties,
            pairs_considered: self.neighbors.pairs_considered,
            pairs_unfolded: self.neighbors.pairs_unfolded,
            neighbor_truncations: self.neighbors.lists_truncated,
            mode_resting,
            mode_seeking,
            mode_feeding,
            population_by_form,
            mean_height_by_form,
            state_hash: snapshot::state_hash(&self.state),
        };
        self.counters = TickCounters::default();
        self.neighbors.pairs_considered = 0;
        self.neighbors.pairs_unfolded = 0;
        self.neighbors.lists_truncated = 0;
        sample
    }

    pub fn config(&self) -> &WorldConfig {
        &self.state.config
    }

    pub fn tick(&self) -> u64 {
        self.state.tick
    }

    pub fn population(&self) -> usize {
        self.state.organisms.len()
    }

    /// Death causes in `deaths_total` order.
    pub const DEATH_CAUSES: [DeathCause; 3] = [DeathCause::Starvation, DeathCause::Age, DeathCause::Collapse];
}

/// The audited energy total of `design/m2-world-spec.md` "Units and quantities":
/// `Σ_cells (e_p·P + e_f·F + De) + Σ_organisms (E + e_r·R) + Σ_escrow (e_r·(S_c + R_c) + E_c)`.
/// Reserve material carries chemical energy; structure does not; fruit carries `e_f`.
#[cfg(any(debug_assertions, test))]
fn stored_energy(state: &WorldState) -> f64 {
    let e_p = state.config.producer.energy_density;
    let e_f = state.config.fruit.energy_density;
    let e_r = state.config.organism.reserve_energy_density;
    let cells: f64 = state.fields.p.iter().map(|p| e_p * p).sum::<f64>()
        + state.fields.f.iter().map(|f| e_f * f).sum::<f64>()
        + state.fields.de.iter().sum::<f64>();
    let organisms: f64 = state
        .organisms
        .iter()
        .map(|(_, o)| {
            o.energy
                + e_r * o.reserve
                + o.escrow.as_ref().map_or(0.0, |e| e_r * (e.structure + e.reserve) + e.energy)
        })
        .sum();
    cells + organisms
}

/// Edible detritus `D_eff = D · min(1, ρ / e_r)` with `ρ = De / D` (zero when `D == 0`),
/// from `design/m2-world-spec.md` "Controller". Detritus too energy-poor to pay for its own
/// reserve storage is not food: this is the same `min(1, ρ / e_r)` factor that scales
/// scavenging assimilation, so what an organism sees and what it can digest agree.
fn edible_detritus(detritus: f64, energy: f64, e_r: f64) -> f64 {
    if detritus <= 0.0 {
        return 0.0;
    }
    let rho = energy / detritus;
    if e_r > 0.0 { detritus * (rho / e_r).min(1.0) } else { detritus }
}

/// The unit chart direction of increasing embedded height at a point of `face`: the chart
/// gradient of `y`, `(tangent_u.y, tangent_v.y)` normalized. Zero on the level top face,
/// `−v` on the four side faces.
fn up_direction(face: Face) -> Vec2 {
    let frame = face_frame(face);
    normalize_or_zero(Vec2::new(frame.tangent_u[1], frame.tangent_v[1]))
}

/// Sensing depth in graph hops for a sensing radius: `ceil(r_sense / 4)`, at least 1 and at
/// most `SENSE_DEPTH_MAX` (`design/fauna-v2.md` "Controller v2").
fn sense_depth(sense_radius: f64) -> usize {
    let hops = (sense_radius / cubarium_surface::CELL_PIXELS).ceil();
    if hops.is_finite() { (hops as usize).clamp(1, SENSE_DEPTH_MAX) } else { 1 }
}

/// For every cell, the cells at graph distance exactly 1, 2 and 3 (breadth-first over the
/// field graph, seams included, never across the open rim).
fn sense_rings(graph: &FieldGraph) -> Vec<[Vec<CellId>; SENSE_DEPTH_MAX]> {
    CellId::all()
        .map(|origin| {
            let mut seen = vec![false; CELL_COUNT];
            seen[origin.index()] = true;
            let mut rings: [Vec<CellId>; SENSE_DEPTH_MAX] = Default::default();
            let mut frontier = vec![origin];
            for ring in rings.iter_mut() {
                let mut next = Vec::new();
                for cell in &frontier {
                    for n in graph.neighbors(*cell).iter().flatten() {
                        if !seen[n.index()] {
                            seen[n.index()] = true;
                            next.push(*n);
                        }
                    }
                }
                next.sort_unstable_by_key(|c| c.index());
                *ring = next.clone();
                frontier = next;
            }
            rings
        })
        .collect()
}

/// Unit gradient, or zero when the gradient carries no direction.
fn normalize_or_zero(v: Vec2) -> Vec2 {
    if v.length() > GRADIENT_EPS { v.normalized().unwrap_or(Vec2::ZERO) } else { Vec2::ZERO }
}

/// The proportional share each request receives when the cell cannot serve them all.
fn share(requested: f64, available: f64) -> f64 {
    if requested > available && requested > 0.0 { available / requested } else { 1.0 }
}

fn ticks_from_seconds(seconds: f64, dt: f64) -> u64 {
    let ticks = (seconds / dt).round();
    if ticks.is_finite() && ticks > 0.0 { ticks as u64 } else { 0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    use cubarium_surface::CellId;

    use crate::events::LifeEvent;

    /// The M2 fixture: v1 founders (one omnivore genotype, `diet` 0.7, drawn hues), so the
    /// intake, gestation and accounting tests below read as they were written. The fauna v2
    /// kinds have their own tests, which set `founders.kinds` explicitly.
    fn config() -> WorldConfig {
        let mut cfg = WorldConfig::default();
        cfg.founders.kinds.clear();
        cfg
    }

    /// The founder diet as the world widens it from the `f32` genome.
    fn founder_diet() -> f64 {
        f64::from(Genome::founder(0.5, &config().drives).diet)
    }

    /// Move `count` founders onto one cell and empty them out, so they contest its producer.
    fn crowd_onto_cell(world: &mut World, cell: CellId, producer: f64) -> Vec<OrganismId> {
        let ids: Vec<OrganismId> = world.state.organisms.iter().map(|(id, _)| id).collect();
        let base = cell.center();
        for (k, id) in ids.iter().enumerate() {
            let o = world.state.organisms.get_mut(*id).expect("live founder");
            o.pos = SurfacePoint::new(base.face, base.u + k as f64 * 0.5, base.v);
            o.reserve = 0.0;
            o.hunger_memory = 1.0;
            o.mode = Mode::Seeking;
            assert_eq!(cell_of(&o.pos), cell, "test bodies must share the cell");
        }
        world.state.fields.p[cell.index()] = producer;
        ids
    }

    #[test]
    fn founders_are_created_with_recorded_external_material() {
        let world = World::new(config()).expect("default config is valid");
        assert_eq!(world.population(), config().founders.count as usize);
        let expected: f64 = world.state.organisms.iter().map(|(_, o)| o.structure + o.reserve).sum();
        assert!((world.state.external_material_in - expected).abs() < 1e-12);
        assert!(world.mass_residual().abs() < 1e-12);
        world.check_invariants().expect("a fresh world is consistent");
        // Founders land on every face: 72 draws over five faces effectively never miss one.
        let mut faces = [0u32; 5];
        for (_, o) in world.state.organisms.iter() {
            faces[o.pos.face.index()] += 1;
        }
        assert!(faces.iter().all(|&n| n > 0), "{faces:?}");
    }

    #[test]
    fn six_hundred_ticks_conserve_material_and_keep_the_population() {
        let mut world = World::new(config()).expect("default config is valid");
        for _ in 0..600 {
            world.step();
        }
        world.check_invariants().expect("invariants hold after 600 ticks");
        assert!(world.population() > 0, "the world died out");
        assert!(world.mass_residual().abs() < 1e-9, "mass residual {}", world.mass_residual());
        let sample = world.telemetry();
        assert_eq!(sample.tick, 600);
        assert!(sample.light_in > 0.0 && sample.heat_out > 0.0);
        assert!(sample.occupied_cells > 0);
    }

    #[test]
    fn replay_is_deterministic_and_seed_sensitive() {
        let mut a = World::new(config()).expect("valid");
        let mut b = World::new(config()).expect("valid");
        let mut c = World::new(WorldConfig { seed: config().seed + 1, ..config() }).expect("valid");
        for _ in 0..300 {
            a.step();
            b.step();
            c.step();
        }
        assert_eq!(a.telemetry().state_hash, b.telemetry().state_hash);
        assert_ne!(a.telemetry().state_hash, c.telemetry().state_hash);
    }

    #[test]
    fn contested_feeding_splits_the_cell_and_conserves_material() {
        let mut cfg = config();
        cfg.founders.count = 3;
        // Low enough that three full mouthfuls (3 x 0.0025 m) cannot all be served.
        cfg.drives.feed_min = 0.001;
        // Linear requests: the spec's proportional allocation is unchanged by the intake
        // saturation, and a saturating mouth on a cell this poor asks for far too little to
        // contest it (three organisms would need `K_P` near zero or a crowd of ~180).
        cfg.organism.intake_half_saturation = 0.0;
        let mut world = World::new(cfg).expect("valid");
        let cell = CellId::new(Face::Front, 0, 0);
        let ids = crowd_onto_cell(&mut world, cell, 0.004);

        let before = world.mass_residual();
        let available = world.state.fields.p[cell.index()];
        world.step();

        let gains: Vec<f64> = ids.iter().map(|id| world.state.organisms.get(*id).expect("alive").reserve).collect();
        assert!(gains.iter().all(|&g| g > 0.0), "{gains:?}");
        for pair in gains.windows(2) {
            assert!((pair[0] - pair[1]).abs() < 1e-15, "unequal shares {gains:?}");
        }
        // The cell is emptied: the requests exceeded what it held.
        assert!(world.state.fields.p[cell.index()] >= 0.0);
        assert!(world.state.fields.p[cell.index()] < 1e-12, "{}", world.state.fields.p[cell.index()]);
        // Each organism assimilated η_m of its share; the rest became detritus in the cell.
        let taken: f64 = gains.iter().sum();
        let eta = world.config().organism.assimilation_material;
        assert!((taken - eta * available).abs() < 1e-6, "{taken} vs {}", eta * available);
        assert!((world.mass_residual() - before).abs() < 1e-12, "{} -> {}", before, world.mass_residual());
        for (_, o) in world.state.organisms.iter() {
            assert!(o.fed_this_tick);
        }
    }

    #[test]
    fn the_render_view_reports_gestation_only_while_an_escrow_is_held() {
        let mut cfg = config();
        cfg.founders.count = 1;
        let gestation_ticks = ticks_from_seconds(cfg.organism.gestation_seconds, DT);
        assert!(gestation_ticks > 1, "this test needs a multi-tick gestation");
        let mut world = World::new(cfg).expect("valid");
        let id = world.state.organisms.iter().map(|(id, _)| id).next().expect("one founder");

        // No escrow: nothing to show.
        assert_eq!(world.render_view().organisms[0].gestation, None);

        // Started this tick: no progress yet.
        world.state.tick = 100;
        let genome = world.state.organisms.get(id).expect("alive").genome.clone();
        world.state.organisms.get_mut(id).expect("alive").escrow = Some(Escrow {
            structure: 0.4,
            reserve: 0.2,
            energy: 0.5,
            started_tick: 100,
            genome,
        });
        assert_eq!(world.render_view().organisms[0].gestation, Some(0.0));

        // Halfway through, to within a tick of rounding.
        world.state.tick = 100 + gestation_ticks / 2;
        let half = world.render_view().organisms[0].gestation.expect("gestating");
        assert!((half - 0.5).abs() < 1.0 / gestation_ticks as f32, "{half}");

        // Exactly due, and then well past it: clamped at 1, never above.
        world.state.tick = 100 + gestation_ticks;
        assert_eq!(world.render_view().organisms[0].gestation, Some(1.0));
        world.state.tick = 100 + gestation_ticks * 9;
        assert_eq!(world.render_view().organisms[0].gestation, Some(1.0));

        // And it goes away with the escrow.
        world.state.organisms.get_mut(id).expect("alive").escrow = None;
        assert_eq!(world.render_view().organisms[0].gestation, None);
    }

    #[test]
    fn a_birth_at_the_cap_refunds_the_escrow_to_the_parent() {
        let mut cfg = config();
        cfg.capacity.max_organisms = 1;
        cfg.founders.count = 1;
        // Keep the parent's stocks still so the refund is the only change to its reserve.
        cfg.mechanisms.grazing = false;
        cfg.mechanisms.scavenging = false;
        let mut world = World::new(cfg).expect("valid");
        let id = world.state.organisms.iter().map(|(id, _)| id).next().expect("one founder");

        // A gestation that finished long ago, so this tick is the birth tick.
        world.state.tick = 700;
        let (structure, reserve, energy, before_reserve, before_energy) = {
            let o = world.state.organisms.get_mut(id).expect("alive");
            let escrow = Escrow {
                structure: 0.4,
                reserve: 0.2,
                energy: 0.5,
                started_tick: 0,
                genome: o.genome.clone(),
            };
            let snapshot = (escrow.structure, escrow.reserve, escrow.energy, o.reserve, o.energy);
            o.escrow = Some(escrow);
            snapshot
        };

        world.step();

        let o = world.state.organisms.get(id).expect("alive");
        assert!(o.escrow.is_none(), "the escrow must be released");
        assert_eq!(o.reserve, before_reserve + (structure + reserve));
        assert!(o.energy > before_energy, "energy {} vs {before_energy}", o.energy);
        assert!(o.energy < before_energy + energy, "movement still costs energy");
        assert_eq!(world.population(), 1);
        assert_eq!(world.state.births_total, 0);
        assert_eq!(world.state.cap_rejections_total, 1);
    }

    #[test]
    fn a_completed_gestation_places_a_child() {
        let mut cfg = config();
        cfg.founders.count = 1;
        cfg.mechanisms.grazing = false;
        cfg.mechanisms.scavenging = false;
        let mut world = World::new(cfg).expect("valid");
        let parent = world.state.organisms.iter().map(|(id, _)| id).next().expect("one founder");
        world.state.tick = 700;
        {
            let o = world.state.organisms.get_mut(parent).expect("alive");
            o.escrow = Some(Escrow {
                structure: 0.4,
                reserve: 0.2,
                energy: 0.5,
                started_tick: 0,
                genome: o.genome.clone(),
            });
        }
        let residual = world.mass_residual();
        world.step();

        assert_eq!(world.population(), 2);
        assert_eq!(world.state.births_total, 1);
        let child = world
            .state
            .organisms
            .iter()
            .find(|(_, o)| o.origin == Origin::Descendant)
            .map(|(id, o)| (id, o.clone()))
            .expect("a child");
        assert_eq!(child.1.parent, Some(parent));
        assert_eq!(child.1.born_tick, 701);
        assert_eq!(child.1.structure, 0.4);
        assert_eq!(child.1.reserve, 0.2);
        assert!((child.1.heading.length() - 1.0).abs() < 1e-12);
        let parent_pos = world.state.organisms.get(parent).expect("alive").pos;
        let offset = cubarium_surface::surface_distance(parent_pos, child.1.pos, 16.0).expect("nearby");
        assert!((offset - 2.5).abs() < 0.1, "child placed {offset} px away");
        assert!((world.mass_residual() - residual).abs() < 1e-12);
    }

    #[test]
    fn render_view_and_telemetry_describe_the_world() {
        let mut world = World::new(config()).expect("valid");
        world.step();
        let view = world.render_view();
        assert_eq!(view.tick, 1);
        assert_eq!(view.producer.len(), CELL_COUNT);
        assert_eq!(view.detritus.len(), CELL_COUNT);
        assert_eq!(view.organisms.len(), world.population());
        assert!(view.organisms.iter().all(|o| !o.lobes.is_empty()));
        assert!(view.organisms.iter().any(|o| !o.moved.is_empty()), "resting still drifts a little");

        let sample = world.telemetry();
        assert_eq!(sample.population, world.population() as u32);
        assert_eq!(sample.population_by_face.iter().sum::<u32>(), sample.population);
        assert!((sample.producer_by_face.iter().sum::<f64>() - sample.producer).abs() < 1e-9);
        assert!((sample.detritus_by_face.iter().sum::<f64>() - sample.detritus).abs() < 1e-9);
        assert!(sample.producer_by_face.iter().all(|&p| p > 0.0));
        assert_eq!(sample.mode_resting + sample.mode_seeking + sample.mode_feeding, sample.population);
        assert!(sample.pairs_considered > 0);
        // The sample resets the per-sample counters.
        let empty = world.telemetry();
        assert_eq!(empty.pairs_considered, 0);
        assert_eq!(empty.light_in, 0.0);
    }

    #[test]
    fn the_energy_audit_closes_every_tick_and_cumulatively() {
        let mut world = World::new(config()).expect("valid");
        let opening = stored_energy(&world.state);
        let mut worst: f64 = 0.0;
        for _ in 0..6000 {
            let before = stored_energy(&world.state);
            let (light, heat) = (world.state.light_in_total, world.state.heat_out_total);
            world.step();
            let booked = (world.state.light_in_total - light) - (world.state.heat_out_total - heat);
            let drift = (stored_energy(&world.state) - before) - booked;
            worst = worst.max(drift.abs());
        }
        assert!(worst < 1e-9, "worst per-tick energy drift {worst:e}");
        // The per-tick identity is exact to rounding; the cumulative sum of 6000 ticks of
        // rounding is bounded relative to the energy being differenced, not absolutely.
        let booked = world.state.light_in_total - world.state.heat_out_total;
        let total = stored_energy(&world.state);
        let overall = (total - opening) - booked;
        assert!(overall.abs() < 1e-9 * total.max(1.0), "cumulative energy drift {overall:e} over 6000 ticks");
        // A real leak would be many orders larger than accumulated rounding.
        assert!(overall.abs() / 6000.0 < 1e-10, "systematic energy drift {:e} per tick", overall.abs() / 6000.0);
        assert!(world.state.light_in_total > 0.0 && world.state.heat_out_total > 0.0);
    }

    #[test]
    fn the_intake_request_saturates_at_half_at_k_p() {
        // One organism alone on a frozen cell: its reserve gain is exactly `η_m · q`.
        fn gain(half_saturation: f64, producer: f64) -> f64 {
            let mut cfg = config();
            cfg.founders.count = 1;
            cfg.mechanisms.scavenging = false;
            cfg.producer.growth = 0.0;
            cfg.producer.mortality = 0.0;
            cfg.detritus.decomposition = 0.0;
            cfg.nutrient.diffusion = 0.0;
            // A rich cell would ripen a trace of fruit before the bite and shift `P`.
            cfg.fruit.ripen = 0.0;
            cfg.organism.intake_half_saturation = half_saturation;
            let mut world = World::new(cfg).expect("valid");
            let id = world.state.organisms.iter().map(|(id, _)| id).next().expect("one founder");
            let cell = CellId::new(Face::Front, 0, 0);
            {
                let o = world.state.organisms.get_mut(id).expect("alive");
                o.pos = cell.center();
                o.reserve = 0.0;
                o.hunger_memory = 1.0;
                o.mode = Mode::Seeking;
            }
            world.state.fields.p[cell.index()] = producer;
            world.step();
            world.state.organisms.get(id).expect("alive").reserve
        }

        let org = config().organism;
        let k_p = org.intake_half_saturation;
        assert!(k_p > 0.0, "the default is a saturating mouth");

        // `K_P = 0` is the linear law: a full mouthful of `graze_rate · dt = k_mouth · diet ·
        // dt` (`design/fauna-v2.md`), assimilated at η_m (to rounding: the world multiplies
        // the same factors in its own order).
        let linear = gain(0.0, k_p);
        let mouthful = org.assimilation_material * (org.mouth_rate * founder_diet()) * DT;
        assert!((linear - mouthful).abs() < 1e-15 * mouthful, "{linear} vs {mouthful}");

        // At `P = K_P` the type-II term is exactly one half.
        let saturating = gain(k_p, k_p);
        assert_eq!(saturating, 0.5 * linear);

        // And it is monotone in the cell's stock: more food, bigger bite, never more than one.
        let richer = gain(k_p, 3.0 * k_p);
        assert!(saturating < richer && richer < linear);
        assert!((richer - 0.75 * linear).abs() < 1e-15 * linear, "{richer} vs {}", 0.75 * linear);
    }

    #[test]
    fn poor_detritus_assimilates_less_and_still_closes() {
        let mut cfg = config();
        cfg.founders.count = 2;
        cfg.mechanisms.grazing = false;
        // Freeze the fields so the detritus the organisms bite into is exactly what is set
        // here: the type-II request reads the cell's pre-settlement stock, and both test
        // cells are on a side face, so the fall step would slide a slice of it away first.
        cfg.producer.growth = 0.0;
        cfg.producer.mortality = 0.0;
        cfg.detritus.decomposition = 0.0;
        cfg.detritus.fall = 0.0;
        cfg.nutrient.diffusion = 0.0;
        let e_r = cfg.organism.reserve_energy_density;
        let mut world = World::new(cfg).expect("valid");
        let ids: Vec<OrganismId> = world.state.organisms.iter().map(|(id, _)| id).collect();
        // Two cells with the same detritus but energy densities of e_r/8 and e_r/2.
        let cells = [CellId::new(Face::Front, 0, 0), CellId::new(Face::Front, 4, 4)];
        let densities = [e_r / 8.0, e_r / 2.0];
        // Enough detritus that even the poor cell's edible share clears `feed_min`.
        let detritus = 2.0;
        for (k, id) in ids.iter().enumerate() {
            let o = world.state.organisms.get_mut(*id).expect("alive");
            o.pos = cells[k].center();
            o.reserve = 0.0;
            o.hunger_memory = 1.0;
            o.mode = Mode::Seeking;
            let c = cells[k].index();
            world.state.fields.p[c] = 0.0;
            world.state.fields.d[c] = detritus;
            world.state.fields.de[c] = densities[k] * detritus;
        }
        let before = stored_energy(&world.state);
        let (light, heat) = (world.state.light_in_total, world.state.heat_out_total);
        world.step();

        let gains: Vec<f64> = ids.iter().map(|id| world.state.organisms.get(*id).expect("alive").reserve).collect();
        assert!(gains[0] > 0.0 && gains[1] > 0.0, "{gains:?}");
        // Poor detritus loses twice: `η` scales with `ρ/e_r` (a factor of four here) and the
        // type-II request scales with `D_eff/(D_eff + K_P)` on top of it.
        let k_p = world.config().organism.intake_half_saturation;
        let edible: Vec<f64> = densities.iter().map(|rho| detritus * (rho / e_r).min(1.0)).collect();
        let bite = |food: f64| food / (food + k_p);
        let expected = 4.0 * bite(edible[1]) / bite(edible[0]);
        assert!(expected > 4.0, "the saturating request must widen the gap, not close it");
        assert!((gains[1] / gains[0] - expected).abs() < 1e-6, "ratio {} vs {expected}", gains[1] / gains[0]);
        // The poor detritus never credits more reserve energy than the food carried.
        for (k, id) in ids.iter().enumerate() {
            let o = world.state.organisms.get(*id).expect("alive");
            let c = cells[k].index();
            assert!(world.state.fields.de[c] >= 0.0);
            assert!(o.reserve * e_r <= densities[k] * detritus + 1e-12);
        }
        let booked = (world.state.light_in_total - light) - (world.state.heat_out_total - heat);
        assert!(((stored_energy(&world.state) - before) - booked).abs() < 1e-9);
    }

    #[test]
    fn energy_free_detritus_is_not_food() {
        let mut cfg = config();
        cfg.founders.count = 2;
        cfg.mechanisms.grazing = false;
        // `De = 2.0` on `D = 1.0` needs a cap that admits it; the point is `ρ ≥ e_r`.
        cfg.detritus.energy_cap = 2.0;
        let mut world = World::new(cfg).expect("valid");
        let ids: Vec<OrganismId> = world.state.organisms.iter().map(|(id, _)| id).collect();
        let cells = [CellId::new(Face::Front, 0, 0), CellId::new(Face::Front, 8, 8)];
        // Same detritus, no energy versus fully charged.
        let energies = [0.0, 2.0];
        for (k, id) in ids.iter().enumerate() {
            let o = world.state.organisms.get_mut(*id).expect("alive");
            o.pos = cells[k].center();
            o.reserve = 0.0;
            o.hunger_memory = 1.0;
            o.mode = Mode::Seeking;
            let c = cells[k].index();
            world.state.fields.p[c] = 0.0;
            world.state.fields.d[c] = 1.0;
            world.state.fields.de[c] = energies[k];
        }
        world.step();

        let spent = world.state.organisms.get(ids[0]).expect("alive");
        assert_eq!(spent.mode, Mode::Seeking, "energy-free detritus must not read as food");
        assert_eq!(spent.reserve, 0.0);
        assert!(!spent.fed_this_tick);

        let rich = world.state.organisms.get(ids[1]).expect("alive");
        assert_eq!(rich.mode, Mode::Feeding, "charged detritus is food");
        assert!(rich.reserve > 0.0);
        assert!(rich.fed_this_tick);
    }

    #[test]
    fn both_intake_channels_respect_the_reserve_ceiling() {
        let mut cfg = config();
        cfg.founders.count = 1;
        let mut world = World::new(cfg).expect("valid");
        let id = world.state.organisms.iter().map(|(id, _)| id).next().expect("one founder");
        let cell = CellId::new(Face::Front, 0, 0);
        // Headroom of 0.0014 m: more than grazing's saturated bite alone (about 0.0012 m at
        // `diet` 0.7), less than the two channels' bites together
        // (`graze_rate + scavenge_rate = mouth_rate`, a full `k_mouth · dt = 0.0025 m`).
        let (reserve_max, headroom) = {
            let o = world.state.organisms.get_mut(id).expect("alive");
            o.pos = cell.center();
            o.hunger_memory = 1.0;
            o.mode = Mode::Seeking;
            o.reserve = o.phenotype.reserve_max - 0.0014;
            (o.phenotype.reserve_max, 0.0014)
        };
        let c = cell.index();
        world.state.fields.p[c] = 1.0;
        world.state.fields.d[c] = 1.0;
        world.state.fields.de[c] = 1.0;
        let before = world.state.organisms.get(id).expect("alive").reserve;
        world.step();

        let o = world.state.organisms.get(id).expect("alive");
        assert!(o.reserve <= reserve_max, "reserve {} exceeds {reserve_max}", o.reserve);
        assert!(o.reserve - before <= headroom + 1e-12);
        // Grazing alone could add at most η_m times its saturated request; more than that
        // proves the scavenging channel ran too.
        let org = &world.config().organism;
        let k_p = org.intake_half_saturation;
        let grazing_only = org.assimilation_material * (0.0025 * founder_diet()) * (1.0 / (1.0 + k_p));
        assert!(o.reserve - before > grazing_only, "{} vs {grazing_only}", o.reserve - before);
        assert!(o.fed_this_tick);
    }

    #[test]
    fn energy_never_goes_negative_while_starving() {
        let mut cfg = config();
        cfg.producer.growth = 0.0;
        cfg.producer.initial_fraction = 0.0;
        // No initial litter either (`detritus.initial_dark`): a foodless world has nothing
        // to scavenge.
        cfg.detritus.initial_dark = 0.0;
        // Dry: standing in a pool slows a body (`design/water.md` "Wading") and so cuts its
        // movement cost, which stretches starvation past this test's five minutes. The test
        // is about the audit while starving, not about pools.
        cfg.water.rain_rate = 0.0;
        let mut world = World::new(cfg).expect("valid");
        let opening = stored_energy(&world.state);
        for _ in 0..6000 {
            world.step();
            for (_, o) in world.state.organisms.iter() {
                assert!(o.energy >= 0.0, "energy {} went negative", o.energy);
            }
            if world.population() == 0 {
                break;
            }
        }
        assert_eq!(world.population(), 0, "a foodless world must empty out");
        assert!(world.state.deaths_total[0] > 0, "starvation is the cause: {:?}", world.state.deaths_total);
        assert!(world.mass_residual().abs() < 1e-9);
        let closing = stored_energy(&world.state);
        let booked = world.state.light_in_total - world.state.heat_out_total;
        let drift = (closing - opening) - booked;
        println!("starvation audit: opening {opening:e} closing {closing:e} booked {booked:e} drift {drift:e}");
        assert!(drift.abs() < 1e-9 * opening.max(1.0), "drift {drift:e}");
    }

    #[test]
    fn life_events_account_for_every_birth_and_death() {
        let cfg = config();
        // Budding needs the age gate, then a full gestation, before a child appears.
        let earliest_parent_age =
            ((cfg.drives.bud_min_age_seconds + cfg.organism.gestation_seconds) / DT).floor() as u64;
        let mut world = World::new(cfg).expect("valid");

        let mut births = 0u64;
        let mut deaths = 0u64;
        let mut seen: Vec<OrganismId> = Vec::new();
        for _ in 0..12_000 {
            world.step();
            for event in world.drain_events() {
                assert_eq!(event.tick(), world.tick(), "an event is dated off its commit tick");
                match event {
                    LifeEvent::Birth { id, parent, parent_age_ticks, parent_births, origin, genome, .. } => {
                        assert_ne!(id, parent);
                        assert_eq!(origin, Origin::Descendant, "founders emit no birth event");
                        assert!(parent_births >= 1, "the parent's own birth is counted");
                        assert!(
                            parent_age_ticks >= earliest_parent_age,
                            "parent aged {parent_age_ticks} ticks cannot have gestated yet"
                        );
                        assert_ne!(genome, 0);
                        assert_eq!(world.state.organisms.get(id).map(|o| o.parent), Some(Some(parent)));
                        assert!(!seen.contains(&id), "organism id {id:?} was born twice");
                        seen.push(id);
                        births += 1;
                    }
                    LifeEvent::Death { id, age_ticks, births: had, .. } => {
                        assert!(age_ticks > 0);
                        assert!(world.state.organisms.get(id).is_none(), "a dead organism is gone");
                        let _ = had;
                        deaths += 1;
                    }
                }
            }
            // A second drain in the same tick yields nothing.
            assert!(world.drain_events().is_empty());
        }

        assert!(births > 0 && deaths > 0, "the run produced {births} births and {deaths} deaths");
        assert_eq!(births, world.state.births_total, "birth events do not match births_total");
        assert_eq!(
            deaths,
            world.state.deaths_total.iter().sum::<u64>(),
            "death events do not match deaths_total"
        );
        assert!(world.drain_events().is_empty());
    }

    #[test]
    fn the_field_dump_and_cell_graph_describe_every_cell() {
        let mut world = World::new(config()).expect("valid");
        world.step();
        let dump = world.field_dump();
        assert_eq!(dump.tick, 1);
        assert_eq!(dump.n, world.state.fields.n);
        assert_eq!(dump.p, world.state.fields.p);
        assert_eq!(dump.d, world.state.fields.d);
        assert_eq!(dump.de, world.state.fields.de);
        assert_eq!(dump.organisms.len(), CELL_COUNT);
        let counted: u32 = dump.organisms.iter().map(|&c| u32::from(c)).sum();
        assert_eq!(counted, world.population() as u32, "every organism is counted once");
        for (_, o) in world.state.organisms.iter() {
            assert!(dump.organisms[cell_of(&o.pos).index()] > 0);
        }

        let neighbors = world.cell_neighbors();
        assert_eq!(neighbors.len(), CELL_COUNT);
        // The open rim leaves 64 cells with three neighbors; everyone else has four.
        let rim = neighbors.iter().filter(|n| n.iter().any(Option::is_none)).count();
        assert_eq!(rim, 64);
        for (i, n) in neighbors.iter().enumerate() {
            for &there in n.iter().flatten() {
                assert!(
                    neighbors[usize::from(there)].iter().flatten().any(|&back| usize::from(back) == i),
                    "cell {i} -> {there} is not reciprocal"
                );
            }
        }
    }

    #[test]
    fn from_state_rebuilds_and_zeroes_the_residual() {
        let mut world = World::new(config()).expect("valid");
        for _ in 0..20 {
            world.step();
        }
        let state = world.state.clone();
        let reloaded = World::from_state(state).expect("a stepped state is valid");
        assert!(reloaded.mass_residual().abs() < 1e-12);
        assert_eq!(reloaded.tick(), world.tick());
        assert_eq!(reloaded.population(), world.population());
    }

    /// `design/water.md`: the water budget is an exact identity for a world created dry,
    /// `Σw == rain_in_total − evap_out_total`, checked every tick and cumulatively.
    #[test]
    fn the_water_budget_closes_every_tick_and_cumulatively() {
        let mut world = World::new(config()).expect("valid");
        assert_eq!(world.state.fields.w.iter().sum::<f64>(), 0.0, "a new world is dry");
        let mut worst: f64 = 0.0;
        for _ in 0..2000 {
            let before: f64 = world.state.fields.w.iter().sum();
            let (rain, evap) = (world.state.rain_in_total, world.state.evap_out_total);
            world.step();
            let booked = (world.state.rain_in_total - rain) - (world.state.evap_out_total - evap);
            let drift = (world.state.fields.w.iter().sum::<f64>() - before) - booked;
            worst = worst.max(drift.abs());
        }
        assert!(worst < 1e-9, "worst per-tick water drift {worst:e}");
        let total: f64 = world.state.fields.w.iter().sum();
        assert!(world.water_residual().abs() < 1e-9 * total.max(1.0), "residual {:e}", world.water_residual());
        assert!(world.state.rain_in_total > 0.0, "it must have rained somewhere in 100 s");
        assert!(world.state.evap_out_total > 0.0);
        assert!(total > 0.0);
        // The view and telemetry carry the same water.
        let view = world.render_view();
        assert_eq!(view.water, world.state.fields.w);
        assert_eq!(view.rain.len(), CELL_COUNT);
        let sample = world.telemetry();
        assert!((sample.water - total).abs() < 1e-12);
        assert!((sample.water_by_face.iter().sum::<f64>() - total).abs() < 1e-9);
    }

    /// `design/water.md` "Wading": an organism's speed is divided by `1 + w` of its cell.
    #[test]
    fn wading_halves_the_speed_at_unit_depth() {
        fn traveled(depth: f64) -> f64 {
            let mut cfg = config();
            cfg.founders.count = 1;
            // Freeze the water so the depth the organism wades through is exactly `depth`.
            cfg.water.rain_rate = 0.0;
            cfg.water.flow = 0.0;
            cfg.water.evap = 0.0;
            let mut world = World::new(cfg).expect("valid");
            let id = world.state.organisms.iter().map(|(id, _)| id).next().expect("founder");
            let cell = {
                let o = world.state.organisms.get_mut(id).expect("alive");
                o.mode = Mode::Seeking;
                o.hunger_memory = 1.0;
                o.reserve = 0.0;
                cell_of(&o.pos)
            };
            world.state.fields.w[cell.index()] = depth;
            // Keep the cell's food below the feeding threshold so the mode stays Seeking.
            world.state.fields.p[cell.index()] = 0.0;
            world.state.fields.d[cell.index()] = 0.0;
            world.step();
            let view = world.render_view();
            view.organisms[0].moved.iter().map(|s| s.length()).sum::<f64>()
        }
        let dry = traveled(0.0);
        let wading = traveled(1.0);
        let deep = traveled(3.0);
        assert!(dry > 0.0, "a seeking organism moves");
        assert!((dry / wading - 2.0).abs() < 1e-9, "dry {dry} wading {wading}");
        assert!((dry / deep - 4.0).abs() < 1e-9, "dry {dry} deep {deep}");
    }

    /// Not a test of anything: prints where the water stands after two simulated hours of a
    /// default world (soil / foliage / canopy by cell height, the soil floor row, the top
    /// face's wettest cells), for the short-run report in `design/water.md`'s slice.
    /// `cargo test -p cubarium-core --release --lib water_by_band -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn report_water_by_band_after_two_hours() {
        let seed: u64 = std::env::var("CUBARIUM_SEED").ok().and_then(|s| s.parse().ok()).unwrap_or(1);
        let mut cfg = config();
        cfg.seed = seed;
        let mut world = World::new(cfg).expect("valid");
        let mut pop_min = world.population();
        // Ponds on the level top face come and go with the showers, so sample them over
        // time as well as at the end: how often at least one interior top cell holds more
        // than 0.5, and the most such cells seen at once.
        let (mut top_pond_ticks, mut top_pond_peak, mut samples) = (0u64, 0usize, 0u64);
        for tick in 0..(7200.0 / DT) as u64 {
            world.step();
            pop_min = pop_min.min(world.population());
            if tick % 20 == 0 {
                samples += 1;
                let ponds = CellId::all()
                    .filter(|c| c.face() == Face::Top && (1..15).contains(&c.cx()) && (1..15).contains(&c.cy()))
                    .filter(|c| world.state.fields.w[c.index()] > 0.5)
                    .count();
                if ponds > 0 {
                    top_pond_ticks += 1;
                }
                top_pond_peak = top_pond_peak.max(ponds);
            }
        }
        println!(
            "top-face interior ponds (>0.5) over the run: present in {:.0}% of once-a-second samples, peak {top_pond_peak} cells at once",
            100.0 * top_pond_ticks as f64 / samples as f64
        );
        let w = &world.state.fields.w;
        let (mut soil, mut foliage, mut canopy, mut floor) = ((0.0, 0), (0.0, 0), (0.0, 0), (0.0, 0));
        let mut top_cells: Vec<(f64, CellId)> = Vec::new();
        for cell in CellId::all() {
            let h = cell.center().embed()[1];
            let i = cell.index();
            if cell.face() == Face::Top {
                canopy.0 += w[i];
                canopy.1 += 1;
                top_cells.push((w[i], cell));
            } else if h < -0.33 {
                soil.0 += w[i];
                soil.1 += 1;
                if cell.cy() == 15 {
                    floor.0 += w[i];
                    floor.1 += 1;
                }
            } else {
                foliage.0 += w[i];
                foliage.1 += 1;
            }
        }
        let total: f64 = w.iter().sum();
        top_cells.sort_by(|a, b| b.0.total_cmp(&a.0));
        let pools_floor = (0..64).filter(|_| true).count();
        let _ = pools_floor;
        let floor_cells: Vec<f64> = CellId::all().filter(|c| c.face() != Face::Top && c.cy() == 15).map(|c| w[c.index()]).collect();
        let floor_pools = floor_cells.iter().filter(|&&x| x > 0.5).count();
        let floor_dry = floor_cells.iter().filter(|&&x| x < 0.3).count();
        let floor_max = floor_cells.iter().cloned().fold(0.0, f64::max);
        let floor_min = floor_cells.iter().cloned().fold(f64::MAX, f64::min);
        let top_pools = top_cells.iter().filter(|(x, _)| *x > 0.5).count();
        println!(
            "water after 2 h (seed {seed}): total {total:.2}; soil {:.2} ({:.0}%), foliage {:.2} ({:.0}%), canopy {:.2} ({:.0}%)",
            soil.0, 100.0 * soil.0 / total, foliage.0, 100.0 * foliage.0 / total, canopy.0, 100.0 * canopy.0 / total
        );
        println!(
            "soil floor row (64 cells): {:.2} total, mean {:.3}, min {floor_min:.3}, max {floor_max:.3}, {floor_pools} cells deeper than 0.5, {floor_dry} cells under 0.3 (dry gaps)",
            floor.0, floor.0 / floor.1 as f64
        );
        println!("population: min {pop_min}, end {}", world.population());
        println!(
            "top face: mean {:.3}, {top_pools} cells deeper than 0.5, wettest {:?}",
            canopy.0 / canopy.1 as f64,
            top_cells.iter().take(3).map(|(x, c)| (format!("{x:.3}"), c.cx(), c.cy())).collect::<Vec<_>>()
        );
        println!(
            "budget: rain_in {:.2} evap_out {:.2} residual {:e}; population {}",
            world.state.rain_in_total, world.state.evap_out_total, world.water_residual(), world.population()
        );
    }

    // --- Fauna v2 (`design/fauna-v2.md`) -------------------------------------------------

    /// One default-kind founder of each kind, by `form`.
    fn one_of_each_kind(world: &World) -> Vec<Organism> {
        let mut out = Vec::new();
        for form in [2u8, 0, 1, 3] {
            out.push(world.state.organisms.iter().find(|(_, o)| o.phenotype.form == form).map(|(_, o)| o.clone()).expect("a founder of each kind"));
        }
        out
    }

    #[test]
    fn the_default_kinds_place_twenty_one_founders_with_the_tables_genomes() {
        let world = World::new(WorldConfig::default()).expect("valid");
        assert_eq!(world.population(), 21, "4 + 8 + 6 + 3 founders");
        let mut by_form = [0usize; 8];
        for (_, o) in world.state.organisms.iter() {
            by_form[o.phenotype.form as usize] += 1;
            assert_eq!(o.genome.version, Genome::VERSION);
        }
        assert_eq!(by_form[..4], [8, 6, 4, 3], "lantern 0 = grazer, sail 1 = glider, mossback 2 = burrower, skimmer 3");
        let Ok([burrower, grazer, glider, skimmer]) = <[Organism; 4]>::try_from(one_of_each_kind(&world)) else {
            panic!("four kinds");
        };
        let g = |o: &Organism| (o.genome.diet, o.genome.depth, o.genome.speed, o.genome.size, o.genome.swim, o.genome.hue);
        assert_eq!(g(&burrower), (0.10, 0.10, 0.6, 1.0, 0.0, 0.15));
        assert_eq!(g(&grazer), (0.85, 0.55, 1.0, 1.0, 0.0, 0.50));
        assert_eq!(g(&glider), (0.90, 1.00, 1.0, 0.8, 0.0, 0.85));
        assert_eq!(g(&skimmer), (0.20, 0.10, 0.9, 0.9, 1.0, 0.65));
        // Unnamed loci keep the v1 founder values, and the phenotype carries the kind.
        assert_eq!((burrower.genome.metabolism, burrower.genome.mouth, burrower.genome.reserve), (0.7, 1.0, 1.0), "the burrower kind fixes metabolism; the rest stay v1");
        assert_eq!(grazer.genome.metabolism, 1.0);
        assert_eq!(burrower.genome.sense, WorldConfig::default().organism.sense_radius as f32);
        assert!((glider.phenotype.h_pref - 1.0).abs() < 1e-12);
        assert!((burrower.phenotype.h_pref + 0.8).abs() < 1e-6);
        assert_eq!(skimmer.phenotype.swim, 1.0);
        assert!((grazer.phenotype.graze_rate - 0.85f32 as f64 * grazer.phenotype.mouth_rate).abs() < 1e-12);
        // The founders' material is booked and the world is consistent.
        world.check_invariants().expect("a fresh kinds world is consistent");
        assert!(world.mass_residual().abs() < 1e-12);
    }

    #[test]
    fn an_empty_kind_list_falls_back_to_v1_founders_and_kinds_are_deterministic() {
        let v1 = World::new(config()).expect("valid");
        assert_eq!(v1.population(), config().founders.count as usize);
        for (_, o) in v1.state.organisms.iter() {
            assert_eq!((o.genome.diet, o.genome.depth, o.genome.swim), (0.7, 0.5, 0.0));
            assert_eq!(o.genome.form, crate::genome::form_of_hue(o.genome.hue), "v1 founders take the hue tercile");
        }
        let a = World::new(WorldConfig::default()).expect("valid");
        let b = World::new(WorldConfig::default()).expect("valid");
        assert_eq!(a.state, b.state, "two kinds worlds from one seed are identical");
        // A kind's own draws do not move when another kind's count changes.
        let mut fewer = WorldConfig::default();
        fewer.founders.kinds[0].count = 2;
        let c = World::new(fewer).expect("valid");
        let gliders = |w: &World| {
            let mut v: Vec<(u32, SurfacePoint)> = w.state.organisms.iter().filter(|(_, o)| o.phenotype.form == 1).map(|(id, o)| (id.slot, o.pos)).collect();
            v.sort_by_key(|x| x.0);
            v.into_iter().map(|x| x.1).collect::<Vec<_>>()
        };
        assert_eq!(gliders(&a), gliders(&c), "glider placements are their own stream");
    }

    #[test]
    fn a_v1_genome_is_upgraded_in_place_on_load() {
        let mut world = World::new(config()).expect("valid");
        for _ in 0..5 {
            world.step();
        }
        let mut state = world.state.clone();
        for (_, o) in state.organisms.iter_mut() {
            o.genome.version = 1;
            o.genome.form = crate::genome::FORM_UNSET;
        }
        let reloaded = World::from_state(state).expect("a v1-genome state is upgraded, not refused");
        for (_, o) in reloaded.state.organisms.iter() {
            assert_eq!(o.genome.version, Genome::VERSION);
            assert_eq!(o.genome.form, crate::genome::form_of_hue(o.genome.hue));
            assert_eq!(o.phenotype.form, o.genome.form, "the phenotype is re-decoded");
        }
    }

    /// A single organism on a frozen cell of a kinds-free world, with the given genome
    /// loci, its cell stocked as asked. Returns the world, the id and the cell index.
    fn frozen_feeder(diet: f32, p: f64, f: f64, d: f64, de: f64) -> (World, OrganismId, usize) {
        let mut cfg = config();
        cfg.founders.count = 1;
        cfg.producer.growth = 0.0;
        cfg.producer.mortality = 0.0;
        cfg.detritus.decomposition = 0.0;
        cfg.detritus.fall = 0.0;
        cfg.nutrient.diffusion = 0.0;
        cfg.fruit.ripen = 0.0;
        cfg.fruit.drop = 0.0;
        cfg.water.rain_rate = 0.0;
        cfg.mechanisms.mutation = false;
        let mut world = World::new(cfg).expect("valid");
        let id = world.state.organisms.iter().map(|(id, _)| id).next().expect("one founder");
        let cell = CellId::new(Face::Front, 4, 4);
        {
            let o = world.state.organisms.get_mut(id).expect("alive");
            o.genome.diet = diet;
            o.phenotype = decode(&o.genome, &world.state.config.organism);
            o.pos = cell.center();
            o.reserve = 0.0;
            o.hunger_memory = 1.0;
            o.mode = Mode::Seeking;
        }
        let c = cell.index();
        world.state.fields.p[c] = p;
        world.state.fields.f[c] = f;
        world.state.fields.d[c] = d;
        world.state.fields.de[c] = de;
        (world, id, c)
    }

    #[test]
    fn frugivory_comes_first_and_the_diet_gates_hold() {
        // A pure grazer on a cell with fruit and producer eats the fruit first: with
        // linear intake and headroom for exactly one mouthful, the fruit bite fills it and
        // the producer, whose request comes second, gets nothing.
        let (mut world, id, c) = frozen_feeder(1.0, 1.0, 1.0, 1.0, 1.0);
        world.state.config.organism.intake_half_saturation = 0.0;
        {
            let o = world.state.organisms.get_mut(id).expect("alive");
            o.reserve = o.phenotype.reserve_max - o.phenotype.graze_rate * DT;
        }
        let (p0, f0, d0) = (world.state.fields.p[c], world.state.fields.f[c], world.state.fields.d[c]);
        // The hand-stocked cell moved the residual once; the step must not move it again.
        let residual = world.mass_residual();
        let before = stored_energy(&world.state);
        let (light, heat) = (world.state.light_in_total, world.state.heat_out_total);
        world.step();
        let o = world.state.organisms.get(id).expect("alive");
        assert_eq!(o.mode, Mode::Feeding);
        assert!(o.fed_this_tick);
        let q = f0 - world.state.fields.f[c];
        assert!(q > 0.0, "fruit was eaten");
        assert!((q - o.phenotype.graze_rate * DT).abs() < 1e-12, "a full fruit mouthful {q}");
        assert_eq!(world.state.fields.p[c], p0, "the producer waited its turn and got nothing");
        // Frugivory: η_m of the bite to reserve, the rest to detritus; scavenging is gated off
        // for a pure grazer, so detritus only grew.
        let eta_m = world.config().organism.assimilation_material;
        let r0 = o.phenotype.reserve_max - o.phenotype.graze_rate * DT;
        assert!((o.reserve - (r0 + eta_m * q)).abs() < 1e-12);
        assert!((world.state.fields.d[c] - (d0 + q - eta_m * q)).abs() < 1e-12);
        let booked = (world.state.light_in_total - light) - (world.state.heat_out_total - heat);
        assert!(((stored_energy(&world.state) - before) - booked).abs() < 1e-9, "fruit energy is audited");
        assert!((world.mass_residual() - residual).abs() < 1e-12, "frugivory conserves material");

        // A pure scavenger never grazes or eats fruit, however rich the cell.
        let (mut world, id, c) = frozen_feeder(0.0, 1.0, 1.0, 0.0, 0.0);
        world.step();
        let o = world.state.organisms.get(id).expect("alive");
        assert_eq!(o.mode, Mode::Seeking, "no detritus, and leaf is not its food");
        assert_eq!((world.state.fields.p[c], world.state.fields.f[c]), (1.0, 1.0));
        assert_eq!(o.reserve, 0.0);

        // A pure grazer never scavenges.
        let (mut world, id, c) = frozen_feeder(1.0, 0.0, 0.0, 1.0, 1.0);
        world.step();
        let o = world.state.organisms.get(id).expect("alive");
        assert_eq!(o.mode, Mode::Seeking);
        assert_eq!(world.state.fields.d[c], 1.0);
        assert_eq!(o.reserve, 0.0);

        // An omnivore below the fruit diet leaves fruit alone but grazes.
        let (mut world, id, c) = frozen_feeder(0.4, 1.0, 1.0, 0.0, 0.0);
        world.step();
        let o = world.state.organisms.get(id).expect("alive");
        assert_eq!(o.mode, Mode::Feeding);
        assert_eq!(world.state.fields.f[c], 1.0, "fruit needs diet ≥ 0.5");
        assert!(world.state.fields.p[c] < 1.0);
    }

    #[test]
    fn fruit_is_conserved_material_over_a_default_run() {
        let mut world = World::new(WorldConfig::default()).expect("valid");
        let opening = stored_energy(&world.state);
        let mut worst: f64 = 0.0;
        for _ in 0..2000 {
            let before = stored_energy(&world.state);
            let (light, heat) = (world.state.light_in_total, world.state.heat_out_total);
            world.step();
            let booked = (world.state.light_in_total - light) - (world.state.heat_out_total - heat);
            worst = worst.max(((stored_energy(&world.state) - before) - booked).abs());
            assert!(world.mass_residual().abs() < 1e-9, "mass residual {}", world.mass_residual());
        }
        assert!(worst < 1e-9, "worst per-tick energy drift {worst:e} with fruit in the sum");
        let total_fruit: f64 = world.state.fields.f.iter().sum();
        assert!(total_fruit > 0.0, "a lit default world ripens some fruit in 100 s");
        let booked = world.state.light_in_total - world.state.heat_out_total;
        let overall = (stored_energy(&world.state) - opening) - booked;
        assert!(overall.abs() < 1e-9 * stored_energy(&world.state).max(1.0), "cumulative {overall:e}");
        // The view, the dump and telemetry all carry the same fruit.
        let view = world.render_view();
        assert_eq!(view.fruit, world.state.fields.f);
        assert_eq!(world.field_dump().f, world.state.fields.f);
        let sample = world.telemetry();
        assert!((sample.fruit - total_fruit).abs() < 1e-12);
    }

    #[test]
    fn the_depth_term_points_up_the_side_faces_and_vanishes_on_top() {
        assert_eq!(up_direction(Face::Top), Vec2::ZERO);
        for face in [Face::Front, Face::Right, Face::Back, Face::Left] {
            let up = up_direction(face);
            assert!((up - Vec2::new(0.0, -1.0)).length() < 1e-12, "{face:?}: {up:?}");
            // Moving along `up` really raises the embedded height.
            let low = SurfacePoint::new(face, 32.0, 40.0);
            let higher = SurfacePoint::new(face, 32.0 + up.x * 4.0, 40.0 + up.y * 4.0);
            assert!(higher.embed()[1] > low.embed()[1]);
        }
        // A canopy-bound organism low on a wall heads up; a soil-bound one high up heads down.
        // No food anywhere (no producers, no litter), so nothing stops it to feed on the way.
        let mut cfg = config();
        cfg.founders.count = 1;
        cfg.producer.initial_fraction = 0.0;
        cfg.producer.growth = 0.0;
        cfg.detritus.initial_dark = 0.0;
        cfg.water.rain_rate = 0.0;
        cfg.drives.w_persist = 0.0;
        cfg.drives.turn_noise = 0.0;
        for (depth, expect_dy_sign) in [(1.0f32, -1.0f64), (0.0, 1.0)] {
            let mut world = World::new(cfg.clone()).expect("valid");
            let id = world.state.organisms.iter().map(|(id, _)| id).next().expect("founder");
            let start = SurfacePoint::new(Face::Front, 32.0, 32.0);
            {
                let o = world.state.organisms.get_mut(id).expect("alive");
                o.genome.depth = depth;
                o.genome.drives.w_persist = 0.0;
                o.genome.drives.turn_noise = 0.0;
                o.phenotype = decode(&o.genome, &world.state.config.organism);
                o.pos = start;
                o.heading = Vec2::new(1.0, 0.0);
                o.reserve = 0.0;
                o.hunger_memory = 1.0;
                o.mode = Mode::Seeking;
            }
            for _ in 0..200 {
                world.step();
            }
            let o = world.state.organisms.get(id).expect("alive");
            let dv = o.pos.v - start.v;
            assert!(dv * expect_dy_sign > 0.5, "depth {depth}: moved {dv} in v on Front (expected sign {expect_dy_sign})");
            assert!((o.heading.y * expect_dy_sign) > 0.9, "heading {:?} settled toward the band", o.heading);
        }
    }

    #[test]
    fn sensing_reaches_the_configured_depth_and_finds_food_two_cells_out() {
        assert_eq!(sense_depth(6.0), 2);
        assert_eq!(sense_depth(4.0), 1);
        assert_eq!(sense_depth(12.0), 3);
        assert_eq!(sense_depth(0.0), 1);
        let rings = sense_rings(&FieldGraph::new());
        let origin = CellId::new(Face::Front, 8, 8);
        assert_eq!(rings[origin.index()][0].len(), 4);
        assert_eq!(rings[origin.index()][1].len(), 8);
        assert_eq!(rings[origin.index()][2].len(), 12);
        for (d, ring) in rings[origin.index()].iter().enumerate() {
            for c in ring {
                let dist = (i32::from(c.cx()) - 8).abs() + (i32::from(c.cy()) - 8).abs();
                assert_eq!(dist as usize, d + 1, "{c:?} is not at graph distance {}", d + 1);
            }
        }
        // A seam-adjacent cell's rings cross onto the neighbouring face and never the rim.
        let corner = CellId::new(Face::Front, 15, 15);
        assert!(rings[corner.index()][0].iter().any(|c| c.face() == Face::Right));
        assert!(rings[corner.index()].iter().flatten().all(|c| c.face() != Face::Top));

        // Food two cells away, none adjacent: a 6 px sensor turns toward it; a 4 px one
        // sees a flat neighbourhood and holds its heading.
        let mut cfg = config();
        cfg.founders.count = 1;
        cfg.producer.initial_fraction = 0.0;
        cfg.producer.growth = 0.0;
        // Freeze everything that could leak a trace of the rich cell into the one-hop
        // neighbourhood before the observation (mortality → detritus → fall; ripening):
        // gradients are normalized, so any nonzero difference steers at full strength.
        cfg.producer.mortality = 0.0;
        cfg.detritus.fall = 0.0;
        cfg.detritus.initial_dark = 0.0;
        cfg.fruit.ripen = 0.0;
        cfg.water.rain_rate = 0.0;
        for (sense, turns) in [(6.0f32, true), (4.0, false)] {
            let mut world = World::new(cfg.clone()).expect("valid");
            let id = world.state.organisms.iter().map(|(id, _)| id).next().expect("founder");
            let here = CellId::new(Face::Front, 8, 8);
            {
                let o = world.state.organisms.get_mut(id).expect("alive");
                o.genome.sense = sense;
                o.genome.depth = 0.5;
                o.genome.drives.w_persist = 0.0;
                o.genome.drives.turn_noise = 0.0;
                o.genome.drives.w_depth = 0.0;
                o.phenotype = decode(&o.genome, &world.state.config.organism);
                o.pos = here.center();
                o.heading = Vec2::new(1.0, 0.0);
                o.reserve = 0.0;
                o.hunger_memory = 1.0;
                o.mode = Mode::Seeking;
            }
            // Rich cells straight "up" the chart, two hops away, nothing at one hop.
            world.state.fields.p[CellId::new(Face::Front, 8, 6).index()] = 1.5;
            world.step();
            let o = world.state.organisms.get(id).expect("alive");
            if turns {
                assert!(o.heading.y < -0.05, "a 6 px sensor turned toward food two cells up: {:?}", o.heading);
            } else {
                assert!((o.heading - Vec2::new(1.0, 0.0)).length() < 1e-9, "a 4 px sensor saw nothing: {:?}", o.heading);
            }
        }
    }

    #[test]
    fn a_swimmer_ignores_pools_while_a_wader_is_slowed() {
        fn traveled(swim: f32, depth: f64) -> f64 {
            let mut cfg = config();
            cfg.founders.count = 1;
            cfg.water.rain_rate = 0.0;
            cfg.water.flow = 0.0;
            cfg.water.evap = 0.0;
            let mut world = World::new(cfg).expect("valid");
            let id = world.state.organisms.iter().map(|(id, _)| id).next().expect("founder");
            let cell = {
                let o = world.state.organisms.get_mut(id).expect("alive");
                o.genome.swim = swim;
                o.phenotype = decode(&o.genome, &world.state.config.organism);
                o.mode = Mode::Seeking;
                o.hunger_memory = 1.0;
                o.reserve = 0.0;
                cell_of(&o.pos)
            };
            world.state.fields.w[cell.index()] = depth;
            world.state.fields.p[cell.index()] = 0.0;
            world.state.fields.f[cell.index()] = 0.0;
            world.state.fields.d[cell.index()] = 0.0;
            world.step();
            world.render_view().organisms[0].moved.iter().map(|s| s.length()).sum::<f64>()
        }
        let dry = traveled(0.0, 0.0);
        assert!((dry / traveled(0.0, 1.0) - 2.0).abs() < 1e-9, "a wader halves at unit depth");
        assert!((traveled(1.0, 1.0) - dry).abs() < 1e-12, "a swimmer moves as if dry");
        assert!((traveled(1.0, 3.0) - dry).abs() < 1e-12);
        assert!((dry / traveled(0.5, 1.0) - 1.5).abs() < 1e-9, "half a swimmer wades at 1 + w/2");
    }

    #[test]
    fn mutation_records_its_loci_and_never_touches_form() {
        let mut cfg = WorldConfig::default();
        cfg.mutation.probability = 1.0;
        let mut world = World::new(cfg).expect("valid");
        let (mut births, mut mutated, mut loci) = (0u64, 0u64, std::collections::HashSet::new());
        for _ in 0..24_000 {
            world.step();
            for event in world.drain_events() {
                if let LifeEvent::Birth { id, parent, mutations, .. } = event {
                    births += 1;
                    let child = world.state.organisms.get(id).expect("newborn").clone();
                    // Copies are exact where no mutation is recorded, and the parent (if it
                    // still lives) shares the child's rig whatever else changed.
                    if let Some(p) = world.state.organisms.get(parent) {
                        assert_eq!(child.genome.form, p.genome.form, "form must never mutate");
                        assert_eq!(child.phenotype.form, p.phenotype.form);
                    }
                    if !mutations.is_empty() {
                        mutated += 1;
                    }
                    for m in &mutations {
                        assert!(crate::genome::MUTABLE_LOCI.contains(&m.locus), "{} is not mutable", m.locus);
                        assert_ne!(m.from, m.to);
                        loci.insert(m.locus);
                    }
                    assert!(mutations.len() <= 2, "at most two loci per birth: {mutations:?}");
                    let mut g = child.genome.clone();
                    assert!(!g.clamp(), "a mutated child is always in range");
                }
            }
        }
        assert!(births >= 20, "the run produced {births} births");
        assert!(mutated as f64 >= 0.8 * births as f64, "with p_mut = 1 nearly every child differs ({mutated}/{births})");
        assert!(loci.len() >= 5, "many different loci were touched: {loci:?}");
        world.check_invariants().expect("mutated worlds stay consistent");

        // With mutation off every child is an exact copy of its parent's escrowed genome.
        let mut cfg = WorldConfig::default();
        cfg.mechanisms.mutation = false;
        let mut world = World::new(cfg).expect("valid");
        let mut seen = 0;
        for _ in 0..24_000 {
            world.step();
            for event in world.drain_events() {
                if let LifeEvent::Birth { id, parent, mutations, .. } = event {
                    assert!(mutations.is_empty());
                    if let Some(p) = world.state.organisms.get(parent) {
                        assert_eq!(world.state.organisms.get(id).expect("newborn").genome, p.genome);
                        seen += 1;
                    }
                }
            }
        }
        assert!(seen > 0, "some exact copies were compared");
    }

    #[test]
    fn telemetry_counts_each_form_and_its_mean_height() {
        let mut world = World::new(WorldConfig::default()).expect("valid");
        world.step();
        let sample = world.telemetry();
        assert_eq!(sample.population_by_form.iter().sum::<u32>(), sample.population);
        assert_eq!(sample.population_by_form[..4], [8, 6, 4, 3]);
        for form in 0..4 {
            let expected: f64 = world.state.organisms.iter().filter(|(_, o)| o.phenotype.form == form as u8).map(|(_, o)| o.pos.embed()[1]).sum::<f64>() / f64::from(sample.population_by_form[form]);
            assert!((sample.mean_height_by_form[form] - expected).abs() < 1e-12);
            assert!(sample.mean_height_by_form[form].abs() <= 1.0);
        }
        assert_eq!(sample.mean_height_by_form[7], 0.0, "an empty form reports zero");
        let view = world.render_view();
        assert!(view.organisms.iter().all(|o| o.form < 4));
    }

    /// Not a test of anything: the fauna v2 short-run reporter for the slice report.
    /// `CUBARIUM_SEED=1 CUBARIUM_HOURS=2 cargo test -p cubarium-core --release --lib
    /// report_fauna_by_form -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn report_fauna_by_form_after_a_short_run() {
        let seed: u64 = std::env::var("CUBARIUM_SEED").ok().and_then(|s| s.parse().ok()).unwrap_or(1);
        let hours: f64 = std::env::var("CUBARIUM_HOURS").ok().and_then(|s| s.parse().ok()).unwrap_or(2.0);
        let mut cfg = WorldConfig { seed, ..WorldConfig::default() };
        // Diagnostic counterfactual only (never a default): `CUBARIUM_ENERGY_CAP` overrides
        // `detritus.energy_cap`, which sets how edible detritus can be.
        if let Some(cap) = std::env::var("CUBARIUM_ENERGY_CAP").ok().and_then(|s| s.parse::<f64>().ok()) {
            cfg.detritus.energy_cap = cap;
            println!("counterfactual: detritus.energy_cap = {cap}");
        }
        let e_r = cfg.organism.reserve_energy_density;
        let mut world = World::new(cfg).expect("valid");
        let names = ["lantern/grazer", "sail/glider", "mossback/burrower", "skimmer"];
        // Edible litter on the soil floor at tick 0, against the feeding threshold.
        {
            let f = &world.state.fields;
            let floor: Vec<f64> = CellId::all()
                .filter(|c| c.face() != Face::Top && c.cy() == 15)
                .map(|c| edible_detritus(f.d[c.index()], f.de[c.index()], e_r))
                .collect();
            let mean = floor.iter().sum::<f64>() / floor.len() as f64;
            let max = floor.iter().cloned().fold(0.0, f64::max);
            let feed_min = world.config().drives.feed_min;
            println!(
                "tick 0 soil floor row: mean D_eff {mean:.3}, max {max:.3}, {} of 64 cells at or above feed_min {feed_min}",
                floor.iter().filter(|&&x| x >= feed_min).count()
            );
        }
        let (mut pop_min, mut pop_max) = (world.population(), world.population());
        let mut min_by_form = [u32::MAX; 4];
        let mut form_of: std::collections::HashMap<OrganismId, u8> = world.state.organisms.iter().map(|(id, o)| (id, o.phenotype.form)).collect();
        let mut deaths_by_form = [[0u32; 3]; 4];
        let mut death_time_by_form = [0.0f64; 4];
        let mut death_age_by_form = [0.0f64; 4];
        // `CUBARIUM_TRACE_FORM=<form>` prints that kind's state once a simulated minute for
        // the first half hour: where it is, what it holds, what it stands on.
        let trace: Option<u8> = std::env::var("CUBARIUM_TRACE_FORM").ok().and_then(|s| s.parse().ok());
        let ticks = (hours * 3600.0 / DT) as u64;
        for tick in 0..ticks {
            if let Some(form) = trace
                && tick % 1200 == 0
                && tick <= 36_000
            {
                let f = &world.state.fields;
                let members: Vec<&Organism> = world.state.organisms.iter().filter(|(_, o)| o.phenotype.form == form).map(|(_, o)| o).collect();
                if !members.is_empty() {
                    let n = members.len() as f64;
                    let mean = |g: &dyn Fn(&Organism) -> f64| members.iter().map(|o| g(o)).sum::<f64>() / n;
                    let modes = members.iter().fold([0; 3], |mut m, o| {
                        m[match o.mode { Mode::Resting => 0, Mode::Seeking => 1, Mode::Feeding => 2 }] += 1;
                        m
                    });
                    println!(
                        "    t {:>4.0}s form {form}: n {} h {:+.2} R/Rmax {:.2} E/Emax {:.2} m_h {:.2} modes rest/seek/feed {:?} D_eff here {:.3} P here {:.3} fed {}",
                        tick as f64 * DT,
                        members.len(),
                        mean(&|o| o.pos.embed()[1]),
                        mean(&|o| o.reserve / o.phenotype.reserve_max),
                        mean(&|o| o.energy / o.phenotype.energy_max),
                        mean(&|o| o.hunger_memory),
                        modes,
                        mean(&|o| { let c = cell_of(&o.pos).index(); edible_detritus(f.d[c], f.de[c], e_r) }),
                        mean(&|o| f.p[cell_of(&o.pos).index()]),
                        members.iter().filter(|o| o.fed_this_tick).count(),
                    );
                }
            }
            world.step();
            for event in world.drain_events() {
                match event {
                    LifeEvent::Birth { id, .. } => {
                        if let Some(o) = world.state.organisms.get(id) {
                            form_of.insert(id, o.phenotype.form);
                        }
                    }
                    LifeEvent::Death { id, cause, age_ticks, .. } => {
                        if let Some(&form) = form_of.get(&id)
                            && (form as usize) < 4
                        {
                            let slot = match cause {
                                DeathCause::Starvation => 0,
                                DeathCause::Age => 1,
                                DeathCause::Collapse => 2,
                            };
                            deaths_by_form[form as usize][slot] += 1;
                            death_time_by_form[form as usize] += (tick + 1) as f64 * DT;
                            death_age_by_form[form as usize] += age_ticks as f64 * DT;
                        }
                    }
                }
            }
            pop_min = pop_min.min(world.population());
            pop_max = pop_max.max(world.population());
            if tick % 100 == 0 {
                let mut by_form = [0u32; 4];
                for (_, o) in world.state.organisms.iter() {
                    if (o.phenotype.form as usize) < 4 {
                        by_form[o.phenotype.form as usize] += 1;
                    }
                }
                for f in 0..4 {
                    min_by_form[f] = min_by_form[f].min(by_form[f]);
                }
            }
        }
        let sample = world.telemetry();
        println!("seed {seed}, {hours} h: population end {} min {pop_min} max {pop_max}; residual {:e}", sample.population, sample.mass_residual);
        for f in 0..4 {
            let deaths: u32 = deaths_by_form[f].iter().sum();
            let mean_death = if deaths > 0 { death_time_by_form[f] / f64::from(deaths) / 60.0 } else { 0.0 };
            let mean_age = if deaths > 0 { death_age_by_form[f] / f64::from(deaths) / 60.0 } else { 0.0 };
            println!(
                "  {:<18} end {:>3}  min {:>3}  mean height {:+.3}  starved {:>3} (age/collapse {} / {}), mean age at death {mean_age:.1} min, mean death time {mean_death:.1} min",
                names[f], sample.population_by_form[f], min_by_form[f], sample.mean_height_by_form[f], deaths_by_form[f][0], deaths_by_form[f][1], deaths_by_form[f][2]
            );
        }
        let fields = &world.state.fields;
        let (mut soil, mut foliage, mut canopy) = (0.0, 0.0, 0.0);
        for cell in CellId::all() {
            let f = fields.f[cell.index()];
            if cell.face() == Face::Top {
                canopy += f;
            } else if cell.center().embed()[1] < -0.33 {
                soil += f;
            } else {
                foliage += f;
            }
        }
        let fruiting = fields.f.iter().filter(|&&x| x > 0.15).count();
        let ripe_cells = fields.p.iter().filter(|&&p| p > world.config().fruit.fruit_min * world.config().producer.max).count();
        let max_f = fields.f.iter().cloned().fold(0.0, f64::max);
        let max_p = fields.p.iter().cloned().fold(0.0, f64::max);
        println!(
            "  fruit total {:.3} ({fruiting} cells above 0.15, max F {max_f:.3}; {ripe_cells} cells with P above fruit_min·P_max, max P {max_p:.3}): soil {:.3} foliage {:.3} canopy {:.3}; P {:.1} D {:.1} N {:.1} water {:.1}",
            sample.fruit, soil, foliage, canopy, sample.producer, sample.detritus, sample.nutrient, sample.water
        );
    }
}
