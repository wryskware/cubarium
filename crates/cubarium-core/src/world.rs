//! The world: checkpointed state plus transient caches, and the tick.

use std::f64::consts::TAU;

use serde::{Deserialize, Serialize};

use cubarium_surface::{
    CELL_COUNT, ChartImage, FACE_EXTENT, Face, FieldGraph, MAX_SEAMS, ScalarField, SurfacePoint, Travel, Vec2, cell_of,
    chart_images, travel_into, unfold_with,
};

use crate::config::WorldConfig;
use crate::controller::{Decision, Observation, decide};
use crate::fields::Fields;
use crate::genome::{Genome, decode};
use crate::habitat::{Habitat, Weather};
use crate::ids::{OrganismId, Slots};
use crate::organism::{DeathCause, Escrow, Mode, Organism, Origin};
use crate::pairs::{Body, NeighborLists};
use crate::rng::{Counter, Stream, normal, unit};
use crate::telemetry::Telemetry;
use crate::view::{OrganismView, RenderView};
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

/// Radius, in pixels, used to unfold the four neighboring cell centers when sensing.
/// A neighbor center is at most about 8.5 px away from any point of the own cell.
const CELL_UNFOLD_RADIUS: f64 = 16.0;

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
    let d = &g.drives;
    for (name, v) in [
        ("w_food", d.w_food),
        ("w_detritus", d.w_detritus),
        ("w_persist", d.w_persist),
        ("w_crowd", d.w_crowd),
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
    scratch: (ScalarField, ScalarField),
    neighbors: NeighborLists,
    travel_buf: Travel,
    /// This tick's traveled segments per slot, for the render view.
    moved: Vec<Vec<cubarium_surface::PathSegment>>,
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
        for index in 0..u64::from(config.founders.count.min(cap)) {
            let seed = config.seed;
            let face_index = (unit(seed, Stream::Founders, index, 0) * 5.0).floor();
            let face = Face::from_index(face_index as u8).unwrap_or(Face::Front);
            let u = unit(seed, Stream::Founders, index, 1) * FACE_EXTENT;
            let v = unit(seed, Stream::Founders, index, 2) * FACE_EXTENT;
            let heading = Vec2::from_screen_angle(unit(seed, Stream::Founders, index, 3) * TAU);
            let hue = unit(seed, Stream::Founders, index, 4) as f32;

            let genome = Genome::founder(hue, &config.drives);
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
        };
        Ok(World::assemble(state, habitat, initial_material))
    }

    /// Rebuild caches around a validated state (after a snapshot load).
    ///
    /// The residual baseline is re-derived so that it reads zero at load and reports drift
    /// since the load; a snapshot does not carry the pre-load residual.
    pub fn from_state(mut state: WorldState) -> Result<World, String> {
        for (_, o) in state.organisms.iter_mut() {
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
            scratch: (ScalarField::zeros(), ScalarField::zeros()),
            neighbors: NeighborLists::default(),
            travel_buf: Travel::default(),
            moved: Vec::new(),
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
            cfg.habitat.moisture_min,
        );
        world.moved.resize_with(world.state.organisms.slot_count(), Vec::new);
        world
    }

    /// One tick in the normative order of `design/m2-world-spec.md` "Tick order".
    /// Returns the per-tick counters (also accumulated internally for telemetry).
    pub fn step(&mut self) -> &TickCounters {
        let dt = DT;
        #[cfg(debug_assertions)]
        let audit = (stored_energy(&self.state), self.state.light_in_total, self.state.heat_out_total);
        {
            let World {
                state,
                graph,
                habitat,
                images,
                light,
                moisture,
                scratch,
                neighbors,
                travel_buf,
                moved,
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
            } = state;
            let cfg: &WorldConfig = config;
            let org_cfg = &cfg.organism;
            let now = *tick;
            let seed = cfg.seed;
            let mut heat = |amount: f64| {
                counters.heat_out += amount;
                *heat_out_total += amount;
            };

            // 1. Admit due stimuli. The queue is empty in M2; the hook is the journal.

            // 2. Weather advance, then derive light and moisture per cell.
            weather.advance(&cfg.weather, seed, now);
            weather.sample(&cfg.weather, habitat, light, moisture, cfg.habitat.moisture_min);

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
                let mut obs = Observation { p_here: fields.p[here], d_here: fields.d[here], ..Observation::default() };
                for neighbor in graph.neighbors(cell).iter().flatten() {
                    let center = neighbor.center();
                    let Some(view) = unfold_with(&images[o.pos.face.index()], o.pos, center, CELL_UNFOLD_RADIUS) else {
                        continue;
                    };
                    let Some(dir) = (view.local - chart).normalized() else {
                        continue;
                    };
                    let there = neighbor.index();
                    obs.grad_p += dir * (fields.p[there] - obs.p_here);
                    obs.grad_d += dir * (fields.d[there] - obs.d_here);
                }
                obs.grad_p = normalize_or_zero(obs.grad_p);
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

                decisions.push((id, decide(o, &obs, now, dt, cfg.mechanisms.grazing, cfg.mechanisms.scavenging)));
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
                let speed = d.effort * o.phenotype.speed_max;
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
            let mut graze = vec![0.0f64; CELL_COUNT];
            let mut scavenge = vec![0.0f64; CELL_COUNT];
            let mut detritus_energy_density = vec![0.0f64; CELL_COUNT];
            let mut requests: Vec<(usize, OrganismId, f64, f64)> = Vec::new();
            for (id, d) in &decisions {
                if d.graze_effort <= 0.0 && d.scavenge_effort <= 0.0 {
                    continue;
                }
                let Some(o) = organisms.get(*id) else { continue };
                let cell = cell_of(&o.pos).index();
                let headroom = (o.phenotype.reserve_max - o.reserve).max(0.0);
                let bite = |effort: f64, room: f64| {
                    if effort > 0.0 { (o.phenotype.mouth_rate * effort * dt).clamp(0.0, room.max(0.0)) } else { 0.0 }
                };
                // Grazing settles first and takes its headroom; scavenging gets the rest,
                // so intake alone can never push the reserve past `R_max`.
                let g = bite(d.graze_effort, headroom);
                let s = bite(d.scavenge_effort, headroom - g);
                if g <= 0.0 && s <= 0.0 {
                    continue;
                }
                graze[cell] += g;
                scavenge[cell] += s;
                requests.push((cell, *id, g, s));
            }
            // Turn the per-cell request sums into proportional shares, and capture the
            // detritus energy density, all before a single transfer is applied.
            let mut contested: Vec<usize> = requests.iter().map(|r| r.0).collect();
            contested.sort_unstable();
            contested.dedup();
            for &cell in &contested {
                let (available_p, available_d) = (fields.p[cell], fields.d[cell]);
                graze[cell] = share(graze[cell], available_p);
                scavenge[cell] = share(scavenge[cell], available_d);
                detritus_energy_density[cell] = if available_d > 0.0 { fields.de[cell] / available_d } else { 0.0 };
            }

            let eta_m = org_cfg.assimilation_material;
            let eta_e = org_cfg.assimilation_energy;
            let e_p = cfg.producer.energy_density;
            let e_r = org_cfg.reserve_energy_density;
            for &(cell, id, g, s) in &requests {
                let Some(o) = organisms.get_mut(id) else { continue };
                let mut eaten = 0.0;
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
                    let direction = Vec2::from_screen_angle(unit(seed, Stream::Birth, key, index * 2) * TAU);
                    let heading = Vec2::from_screen_angle(unit(seed, Stream::Birth, key, index * 2 + 1) * TAU);
                    (parent.pos, direction, heading, escrow)
                };
                let (from, direction, heading, escrow) = placement;
                travel_into(from, direction * cfg.drives.birth_offset_px, travel_buf);
                counters.travel_ties += travel_buf.ties;
                counters.travel_fallbacks += u32::from(travel_buf.fallback);
                let heading = travel_buf.map.apply(heading).normalized().unwrap_or(Vec2::new(1.0, 0.0));
                let genome = escrow.genome.clone();
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
                let child_id = organisms.insert(child);
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
        }
        &self.counters
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
                moved: self.moved.get(id.slot as usize).cloned().unwrap_or_default(),
            })
            .collect();
        RenderView {
            tick: self.state.tick,
            producer: self.state.fields.p.clone(),
            detritus: self.state.fields.d.clone(),
            producer_max: self.state.config.producer.max,
            organisms,
        }
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
        for (_, o) in self.state.organisms.iter() {
            population_by_face[o.pos.face.index()] += 1;
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
            organism_material,
            organism_energy,
            light_in: self.counters.light_in,
            heat_out: self.counters.heat_out,
            mass_residual,
            population_by_face,
            occupied_cells,
            travel_fallbacks: self.counters.travel_fallbacks,
            travel_ties: self.counters.travel_ties,
            pairs_considered: self.neighbors.pairs_considered,
            pairs_unfolded: self.neighbors.pairs_unfolded,
            neighbor_truncations: self.neighbors.lists_truncated,
            mode_resting,
            mode_seeking,
            mode_feeding,
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
/// `Σ_cells (e_p·P + De) + Σ_organisms (E + e_r·R) + Σ_escrow (e_r·(S_c + R_c) + E_c)`.
/// Reserve material carries chemical energy; structure does not.
#[cfg(any(debug_assertions, test))]
fn stored_energy(state: &WorldState) -> f64 {
    let e_p = state.config.producer.energy_density;
    let e_r = state.config.organism.reserve_energy_density;
    let cells: f64 =
        state.fields.p.iter().map(|p| e_p * p).sum::<f64>() + state.fields.de.iter().sum::<f64>();
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

    fn config() -> WorldConfig {
        WorldConfig::default()
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
    fn poor_detritus_assimilates_less_and_still_closes() {
        let mut cfg = config();
        cfg.founders.count = 2;
        cfg.mechanisms.grazing = false;
        let e_r = cfg.organism.reserve_energy_density;
        let mut world = World::new(cfg).expect("valid");
        let ids: Vec<OrganismId> = world.state.organisms.iter().map(|(id, _)| id).collect();
        // Two cells with the same detritus but energy densities of e_r/8 and e_r/2.
        let cells = [CellId::new(Face::Front, 0, 0), CellId::new(Face::Front, 4, 4)];
        let densities = [e_r / 8.0, e_r / 2.0];
        for (k, id) in ids.iter().enumerate() {
            let o = world.state.organisms.get_mut(*id).expect("alive");
            o.pos = cells[k].center();
            o.reserve = 0.0;
            o.hunger_memory = 1.0;
            o.mode = Mode::Seeking;
            let c = cells[k].index();
            world.state.fields.p[c] = 0.0;
            world.state.fields.d[c] = 0.5;
            world.state.fields.de[c] = densities[k] * 0.5;
        }
        let before = stored_energy(&world.state);
        let (light, heat) = (world.state.light_in_total, world.state.heat_out_total);
        world.step();

        let gains: Vec<f64> = ids.iter().map(|id| world.state.organisms.get(*id).expect("alive").reserve).collect();
        assert!(gains[0] > 0.0 && gains[1] > 0.0, "{gains:?}");
        // η scales with ρ/e_r, so a four-times richer detritus assimilates four times more.
        assert!((gains[1] / gains[0] - 4.0).abs() < 1e-6, "ratio {}", gains[1] / gains[0]);
        // The poor detritus never credits more reserve energy than the food carried.
        for (k, id) in ids.iter().enumerate() {
            let o = world.state.organisms.get(*id).expect("alive");
            let c = cells[k].index();
            assert!(world.state.fields.de[c] >= 0.0);
            assert!(o.reserve * e_r <= densities[k] * 0.5 + 1e-12);
        }
        let booked = (world.state.light_in_total - light) - (world.state.heat_out_total - heat);
        assert!(((stored_energy(&world.state) - before) - booked).abs() < 1e-9);
    }

    #[test]
    fn both_intake_channels_respect_the_reserve_ceiling() {
        let mut cfg = config();
        cfg.founders.count = 1;
        let mut world = World::new(cfg).expect("valid");
        let id = world.state.organisms.iter().map(|(id, _)| id).next().expect("one founder");
        let cell = CellId::new(Face::Front, 0, 0);
        // Headroom of 0.004 m against two full mouthfuls of 0.0025 m.
        let (reserve_max, headroom) = {
            let o = world.state.organisms.get_mut(id).expect("alive");
            o.pos = cell.center();
            o.hunger_memory = 1.0;
            o.mode = Mode::Seeking;
            o.reserve = o.phenotype.reserve_max - 0.004;
            (o.phenotype.reserve_max, 0.004)
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
        // Grazing alone could add at most η_m · 0.0025; more than that proves both channels ran.
        let grazing_only = world.config().organism.assimilation_material * 0.0025;
        assert!(o.reserve - before > grazing_only, "{} vs {grazing_only}", o.reserve - before);
        assert!(o.fed_this_tick);
    }

    #[test]
    fn energy_never_goes_negative_while_starving() {
        let mut cfg = config();
        cfg.producer.growth = 0.0;
        cfg.producer.initial_fraction = 0.0;
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
        let booked = world.state.light_in_total - world.state.heat_out_total;
        assert!(((stored_energy(&world.state) - opening) - booked).abs() < 1e-9);
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
}
