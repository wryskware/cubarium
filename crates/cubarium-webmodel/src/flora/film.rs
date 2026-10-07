//! Spatial census closures of film.rs:99,127,207 and the shared edible refuge.
use cubarium_voxel_flora::FloraConfig;

pub fn capacity(c: &FloraConfig, depth: f64, moisture: f64, distance: f64) -> f64 {
    if depth > 0.0 && depth >= c.film_wet_min_depth_m && depth <= c.film_max_depth_m {
        return c.film_capacity_per_m2;
    }
    if depth > c.film_max_depth_m || !moisture.is_finite() {
        return 0.0;
    }
    let floor = if moisture >= c.film_land_moisture_min {
        c.film_land_moisture_min
    } else if c.film_shore_band_m > 0.0
        && distance <= c.film_shore_band_m
        && moisture >= c.film_shore_moisture_min
    {
        c.film_shore_moisture_min
    } else {
        return 0.0;
    };
    cubarium_voxel_flora::land_capacity_per_m2(c, moisture, floor)
}

pub fn edible(c: &FloraConfig, stock: f64, faces: f64) -> f64 {
    if faces > 0.0 {
        c.edible_film(stock / faces) * faces
    } else {
        stock.max(0.0)
    }
}

pub fn growth(
    c: &FloraConfig,
    stock: f64,
    cap: f64,
    light: f64,
    land: bool,
    dt: f64,
) -> (f64, f64) {
    let rate = if land {
        c.film_land_growth_per_s
    } else {
        c.film_growth_per_s
    };
    let grown = if cap > 0.0 {
        (rate * light * stock * (1.0 - stock / cap) * dt).max(0.0)
    } else {
        0.0
    };
    (
        grown,
        (c.film_maintenance_per_s * stock * dt).min(stock).max(0.0),
    )
}

/// Bins retain edible stock on each kind of face rather than subtracting a global floor.
#[derive(Clone, Debug)]
pub struct Bed {
    pub faces: f64,
    pub stock: f64,
    pub capacity: f64,
    pub light: f64,
    pub land: bool,
    pub zone: usize,
}

impl super::model::Model {
    /// A settled surface census: nearest water respects wrapped x and support height.
    /// Water/moisture remain held, as in the existing zone closure.
    pub fn init_film(&mut self, faces: &[super::census::Face]) {
        use crate::fauna::behavior::ShoreBand;
        let c = &self.config;
        let cell = c.voxel_m.powi(2);
        let width = faces.iter().map(|f| f.x).max().map_or(1, |x| x + 1);
        let water: Vec<_> = faces.iter().filter(|f| f.depth_m > 0.0).collect();
        let mut bins = std::collections::BTreeMap::<(usize, bool, bool, u32, u32), Bed>::new();
        let mut face_keys = std::collections::BTreeMap::new();
        self.shore_bands.clear();
        for f in faces {
            let mut nearest = (f64::INFINITY, f64::INFINITY);
            for w in &water {
                if w.y > f.y {
                    continue;
                }
                let dx = f64::from(f.x.abs_diff(w.x).min(width - f.x.abs_diff(w.x))) * c.voxel_m;
                let dz = f64::from(f.z.abs_diff(w.z)) * c.voxel_m;
                let d = dx.hypot(dz);
                let height = (f64::from(f.y - w.y) * c.voxel_m - w.depth_m).max(0.0);
                if d < nearest.0 || (d == nearest.0 && height < nearest.1) {
                    nearest = (d, height);
                }
            }
            self.shore_bands.push(ShoreBand {
                faces: 1.0,
                depth: f.depth_m,
                moisture: f.pore,
                distance: nearest.0,
                height: nearest.1,
            });
            let cap = capacity(c, f.depth_m, f.pore, nearest.0) * cell;
            let under = f.depth_m > 0.0
                && f.depth_m >= c.film_wet_min_depth_m
                && f.depth_m <= c.film_max_depth_m;
            let light = f.sky
                * if under {
                    (-c.film_extinction_per_m * f.depth_m).exp()
                } else {
                    1.0
                };
            let zi = self.zones.of_face[&(f.x, f.y, f.z)];
            // Numerical histogram, not an ecological parameter: retain empty/refuge/rich beds.
            let fill_bin = if cap > 0.0 {
                (32.0 * f.film / cap).floor().clamp(0.0, 32.0) as u32
            } else {
                0
            };
            // One-cell distance shells retain the shore-to-inland dispersal clock.
            let shell = (nearest.0 / c.voxel_m).floor().min(f64::from(u32::MAX)) as u32;
            let key = (zi, !under, cap > 0.0, fill_bin, shell);
            face_keys.insert((f.x, f.y, f.z), key);
            let b = bins.entry(key).or_insert(Bed {
                faces: 0.0,
                stock: 0.0,
                capacity: 0.0,
                light: 0.0,
                land: !under,
                zone: zi,
            });
            b.faces += 1.0;
            b.stock += f.film;
            b.capacity += cap;
            b.light += light;
        }
        let ids: std::collections::BTreeMap<_, _> =
            bins.keys().enumerate().map(|(i, k)| (*k, i)).collect();
        let mut edges = std::collections::BTreeMap::<(usize, usize), f64>::new();
        for (&(x, y, z), key) in &face_keys {
            let a = ids[key];
            for (dx, dz) in [(-1i64, 0i64), (1, 0), (0, -1), (0, 1)] {
                let zz = i64::from(z) + dz;
                if zz < 0 {
                    continue;
                }
                let xx = (i64::from(x) + dx).rem_euclid(i64::from(width)) as u32;
                // seeds::support_near tries the same height, one up, then one down.
                let to = [Some(y), y.checked_add(1), y.checked_sub(1)]
                    .into_iter()
                    .flatten()
                    .find_map(|yy| face_keys.get(&(xx, yy, zz as u32)));
                if let Some(to) = to {
                    let b = ids[to];
                    if a != b && bins[key].capacity > 0.0 && bins[to].capacity > 0.0 {
                        *edges.entry((a, b)).or_default() += 1.0;
                    }
                }
            }
        }
        self.film_edges = edges.into_iter().map(|((a, b), n)| (a, b, n)).collect();
        self.film_beds = bins
            .into_values()
            .map(|mut b| {
                b.light /= b.faces;
                b
            })
            .collect();
        self.film_capacity = self.film_beds.iter().map(|b| b.capacity).sum();
        self.film = self.film_beds.iter().map(|b| b.stock).sum();
    }

    pub fn edible_film(&self) -> f64 {
        self.film_beds
            .iter()
            .map(|b| edible(&self.config, b.stock, b.faces))
            .sum()
    }

    pub fn draw_film(&mut self, want: f64) -> f64 {
        let total = self.edible_film();
        let taken = want.max(0.0).min(total);
        if total > 0.0 {
            for b in &mut self.film_beds {
                b.stock -= taken * edible(&self.config, b.stock, b.faces) / total;
            }
        }
        self.film = self.film_beds.iter().map(|b| b.stock).sum();
        taken
    }

    pub fn grow_film(&mut self, dt: f64) {
        let c = &self.config;
        let before: Vec<f64> = self.film_beds.iter().map(|b| b.stock / b.faces).collect();
        let day = self.day.at(self.t_s, dt).daylight;
        for b in &mut self.film_beds {
            let (raw, upkeep) = growth(c, b.stock, b.capacity, b.light * day, b.land, dt);
            let excess = if b.capacity <= 0.0 {
                b.stock
            } else if b.land {
                (b.stock - b.capacity).max(0.0)
            } else {
                0.0
            };
            let died = (c.film_dry_death_per_s * dt * excess).min((b.stock - upkeep).max(0.0));
            let pool = &mut self.z[b.zone].pool[super::model::VACANT];
            let grown = if c.film_n_tissue > 0.0 {
                raw.min((*pool / c.film_n_tissue).max(0.0))
            } else {
                raw
            };
            *pool += (upkeep + died - grown) * c.film_n_tissue;
            b.stock = (b.stock + grown - upkeep - died).max(0.0);
            self.z[b.zone].litter += died;
            self.c.fixed += grown;
            self.c.respired += upkeep;
            self.c.film_grown += grown;
            self.c.film_upkeep += upkeep;
        }
        // film.rs:340: step-start density, four cardinal supports, and the recipient's
        // land capacity. Edges are counted from the actual census, aggregated by bed.
        for &(a, b, edges) in &self.film_edges {
            let from = before[a];
            let there = before[b];
            let dst = &self.film_beds[b];
            let room = if dst.land {
                (dst.capacity / dst.faces - there).max(0.0)
            } else {
                f64::INFINITY
            };
            let flux = (c.film_spread_per_s * dt * from)
                .min(0.5 * (from - there).max(0.0))
                .min(room)
                * edges;
            let flux = flux.min(self.film_beds[a].stock).max(0.0);
            self.film_beds[a].stock -= flux;
            self.film_beds[b].stock += flux;
        }
        self.film = self.film_beds.iter().map(|b| b.stock).sum();
    }

    pub fn habitat(
        &self,
        snail: &cubarium_voxel_fauna::FounderPhysiology,
    ) -> crate::fauna::behavior::Habitat {
        use crate::fauna::behavior::{shore_eligible, Habitat};
        let c = &self.config;
        let cell = c.voxel_m.powi(2);
        let mut h = Habitat {
            area: self.zones.zones.iter().map(|z| z.area * cell).sum(),
            film_reference: c.film_capacity_per_m2 * cell,
            film_refuge: c.film_graze_refuge,
            ..Habitat::default()
        };
        h.sky = self
            .zones
            .zones
            .iter()
            .map(|z| z.sky * z.area * cell)
            .sum::<f64>()
            / h.area.max(f64::MIN_POSITIVE);
        let mut rich_weight = 0.0;
        for b in &self.film_beds {
            if b.capacity <= 0.0 {
                continue;
            }
            h.film_faces += b.faces;
            if b.land {
                h.film_land_share += b.faces;
            }
            let e = edible(c, b.stock, b.faces);
            if e > 0.0 {
                h.film_rich += e * e / b.faces;
                rich_weight += e;
            }
            h.film_growth += b.faces
                * b.light
                * if b.land {
                    c.film_land_growth_per_s
                } else {
                    c.film_growth_per_s
                };
        }
        h.film_land_share /= h.film_faces.max(1.0);
        h.film_growth /= h.film_faces.max(1.0);
        h.film_rich /= rich_weight.max(f64::MIN_POSITIVE) * h.film_reference.max(f64::MIN_POSITIVE);
        if let Some(am) = snail.amphibious {
            // Select the actual eligible shore faces first; an average is only taken afterward.
            let sites: Vec<_> = self
                .shore_bands
                .iter()
                .filter(|s| shore_eligible(&am, s.depth, s.moisture, s.distance, s.height))
                .collect();
            if !sites.is_empty() {
                let n = sites.len() as f64;
                h.shore.push(crate::fauna::behavior::ShoreBand {
                    faces: n,
                    moisture: sites.iter().map(|s| s.moisture).sum::<f64>() / n,
                    ..Default::default()
                });
            }
        }
        for z in &self.z {
            for sp in cubarium_voxel_flora::Species::ALL {
                let sc = c.species(sp);
                for co in &z.cohorts[sp.index()] {
                    if sp == cubarium_voxel_flora::Species::Glowcap && co.st.foliage > 0.0 {
                        h.glowcaps += co.n;
                    }
                    if sc.nectar_per_s > 0.0 && co.st.nectar > 0.0 {
                        h.flowers += co.n;
                    }
                }
            }
        }
        h
    }
}
