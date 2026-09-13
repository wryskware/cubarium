//! Versioned output of the Godot art project. File I/O stays in the host.

use anyhow::{Context, Result, ensure};
use cubarium_render::{Pose, Sprite};
use cubarium_surface::Vec2;
use std::path::Path;

/// Clip states in atlas order; every creature has exactly these four rows.
pub const STATES: [&str; 4] = ["rest", "move", "feed", "bud"];

/// Fewest and most samples a baked clip may carry (pack v4 makes the counts data).
pub const MIN_FRAMES: usize = 2;
pub const MAX_FRAMES: usize = 32;

pub struct Clip {
    pub frames: Vec<Sprite>,
    pub seconds: f64,
    pub looping: bool,
}

impl Clip {
    /// The nearest earlier sample: the frame a discrete player would hold. Looping clips
    /// wrap; a non-looping clip clamps to its last frame at and past `seconds`.
    pub fn at(&self, seconds: f64) -> &Sprite {
        let phase = if !seconds.is_finite() {
            0.0
        } else if self.looping {
            seconds.rem_euclid(self.seconds) / self.seconds
        } else {
            (seconds / self.seconds).clamp(0.0, 1.0)
        };
        let index = if self.looping {
            (phase * self.frames.len() as f64).floor() as usize
        } else {
            (phase * (self.frames.len() - 1) as f64).round() as usize
        };
        &self.frames[index.min(self.frames.len() - 1)]
    }

    /// The pose at `seconds`: the two samples bracketing it and how far between them it
    /// lies, for [`cubarium_render::stamp_pose`].
    ///
    /// **Normative.** With `n` samples: a looping clip maps `seconds` to `u = (seconds
    /// mod length) / length · n`, `first = frames[floor(u)]`, `second = frames[(floor(u)
    /// + 1) mod n]`, `mix = fract(u)` — so the last sample blends back into the first and
    /// the loop is continuous at the wrap. A non-looping clip maps `seconds` to `u =
    /// clamp(seconds / length, 0, 1) · (n − 1)`, `second = frames[min(floor(u) + 1, n −
    /// 1)]`, so `seconds ≥ length` is exactly the last sample (mix 0) and `seconds ≤ 0`
    /// exactly the first. A non-finite `seconds` is the first sample. `mix` is 0 exactly
    /// when `seconds` lands on a sample.
    pub fn sample(&self, seconds: f64) -> Pose<'_> {
        let n = self.frames.len();
        if n < 2 || !seconds.is_finite() || !(self.seconds.is_finite() && self.seconds > 0.0) {
            return Pose::still(&self.frames[0]);
        }
        let u = if self.looping {
            seconds.rem_euclid(self.seconds) / self.seconds * n as f64
        } else {
            (seconds / self.seconds).clamp(0.0, 1.0) * (n - 1) as f64
        };
        let i = (u.floor() as usize).min(n - 1);
        let j = if self.looping { (i + 1) % n } else { (i + 1).min(n - 1) };
        let mix = (u - i as f64) as f32;
        Pose { first: &self.frames[i], second: &self.frames[j], mix }
    }
}

/// Where a plant belongs in the stratified cube (`design/stratified-world.md`). A band
/// is a placement hint for the presenter; the simulation knows nothing of plants.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Band {
    Soil,
    Foliage,
    Canopy,
    Water,
}

impl Band {
    pub fn parse(name: &str) -> Option<Band> {
        Some(match name {
            "soil" => Band::Soil,
            "foliage" => Band::Foliage,
            "canopy" => Band::Canopy,
            "water" => Band::Water,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            Band::Soil => "soil",
            Band::Foliage => "foliage",
            Band::Canopy => "canopy",
            Band::Water => "water",
        }
    }
}

/// One authored plant: three growth stages, each a looping sway clip, and for some
/// plants a fourth looping `fruit` clip showing the full-grown plant in flower or fruit.
/// Names are asset names, not species; nothing here has a simulated effect.
pub struct Plant {
    pub name: String,
    pub band: Band,
    pub stages: [Clip; 3],
    pub fruit: Option<Clip>,
    /// Authored growth clips (pack v5), in bake order; empty for a pack before v5 and for
    /// every plant whose scene has none. A plant may have at most one clip per step.
    pub transitions: Vec<Transition>,
}

impl Plant {
    /// The authored growth clip from stage `from` to stage `to = from + 1`, if the pack
    /// carries one. A pair without a clip (and every v1–v4 pack) returns `None`, which is
    /// the presenter's cue to keep its reveal mask.
    pub fn transition(&self, from: u8, to: u8) -> Option<&Clip> {
        self.transitions.iter().find(|t| t.from == from && t.to == to).map(|t| &t.clip)
    }
}

/// One authored **growth transition** (pack v5): a non-looping clip that carries a plant
/// from stage `from` to stage `to == from + 1`, baked from a `grow<from><to>` animation in
/// the plant's scene and sampled inclusively (its first frame is the source stage's neutral
/// pose, its last the target's), so [`Clip::sample`] at 0 and at `seconds` are exactly the
/// two endpoints. Growth pacing stays the presenter's; the clip only says what growing
/// looks like.
pub struct Transition {
    pub from: u8,
    pub to: u8,
    /// Always `looping: false`.
    pub clip: Clip,
}

/// One authored tall plant (pack v3): a column of 16×16 tiles stacked every cell. `trunk`
/// is a segment periodic in `y` with period 4 px, so overlapping segments draw the same
/// pixels; `base` caps the column at the horizon and `crown` tops it. A climber such as
/// `vinecoil` has a trunk only and is drawn over another column's trunk. All clips loop.
pub struct TallPlant {
    pub name: String,
    pub base: Option<Clip>,
    pub trunk: Clip,
    pub crown: Option<Clip>,
    /// The crown's own art only: the crown with its *tail* — the rows in which it merely
    /// repeats the trunk pattern so as to join the top segment seamlessly — cleared where
    /// the trunk paints ([`Sprite::subtract`] over [`TallPlant::tail_row`]`..16` against the
    /// same-index trunk frame). Drawn at a fractional height over a crisp trunk while the
    /// column grows. Same frame count and timing as `crown` (the loader requires it).
    pub cap: Option<Clip>,
    /// The first crown-tile row (from the top) of the tail: from here down, in every frame,
    /// every texel the trunk paints the crown paints identically. Measured from the baked
    /// pixels at load, not assumed, so a dome pixel that happens to share the trunk's
    /// color above the tail is never mistaken for trunk. 16 for a plant with no crown.
    pub tail_row: usize,
}

/// The lowest tail row the loader accepts: the top trunk segment reaches four rows into
/// the crown's tile, so a "tail" starting above row 4 would claim rows with nothing
/// beneath them.
pub const CROWN_TAIL_MIN_ROW: usize = 4;

/// The first row from which, in every frame, every texel `trunk` paints `crown` paints
/// identically (transparent texels normalized): the crown's trunk-joining tail. Returns
/// `height` when the bottom row itself does not join.
fn tail_row_of(crown: &Clip, trunk: &Clip) -> usize {
    let height = crown.frames.first().map_or(16, |s| s.height());
    let mut tail = height;
    'rows: for row in (0..height).rev() {
        for (c, t) in crown.frames.iter().zip(&trunk.frames) {
            if !c.paints_like(t, row..row + 1) {
                break 'rows;
            }
        }
        tail = row;
    }
    tail
}

/// One 8×8 tileable ground-cover texture (pack v3), four slowly breathing frames, meant to
/// be tiled under the plants of its band. Alpha carries coverage.
pub struct GroundTile {
    pub name: String,
    pub band: Band,
    /// 8×8 sprites with pivot (4, 4).
    pub frames: Vec<Sprite>,
    pub seconds: f64,
}

pub struct ArtPack {
    /// Creature rig names in atlas order (`creature_names` in pack v2; the fixed
    /// lantern/sail/mossback trio for pack v1). A rig is a look, not a species.
    creature_names: Vec<String>,
    /// Creature-major, then rest/move/feed/bud. These are visual studies, not diets.
    pub clips: Vec<Clip>,
    /// Legacy motifs: rosette, fern, lichen (pack v1 and v2).
    pub habitat: Vec<Sprite>,
    /// Pack v2 plants in bake order; empty for a v1 pack.
    pub plants: Vec<Plant>,
    /// Pack v3 tall plants in bake order; empty before v3.
    pub tall: Vec<TallPlant>,
    /// Pack v3 ground-cover tiles in bake order; empty before v3.
    pub ground: Vec<GroundTile>,
}

impl ArtPack {
    pub fn load(directory: &Path) -> Result<Self> {
        let path = directory.join("pack.json");
        let file = std::fs::File::open(&path)
            .with_context(|| format!("open {} (bake art/project.godot first)", path.display()))?;
        let meta: serde_json::Value = serde_json::from_reader(file)?;
        let version = meta["version"].as_u64().context("pack needs a version")?;
        ensure!(
            (1..=5).contains(&version)
                && meta["tile"] == 16
                && meta["pivot"] == serde_json::json!([8, 8])
                && meta["facing"] == "+x",
            "unsupported art pack layout"
        );
        // Pack v4 makes the sample counts data; earlier packs always carried eight
        // creature samples and four plant samples, and still say so.
        let frames = frame_count(&meta["frames"], "frames")?;
        ensure!(version >= 4 || frames == 8, "packs before v4 carry eight creature frames");
        ensure!(
            meta["creatures"] == "creatures.png" && meta["habitat"] == "habitat.png",
            "unsupported atlas filenames"
        );
        ensure!(
            meta["habitat_names"] == serde_json::json!(["rosette", "fern", "lichen"]),
            "unsupported habitat order"
        );
        let creature_names: Vec<String> = match meta.get("creature_names") {
            Some(list) => list
                .as_array()
                .context("creature_names must be a list")?
                .iter()
                .map(|v| v.as_str().map(str::to_owned).context("creature name must be a string"))
                .collect::<Result<_>>()?,
            None => ["lantern", "sail", "mossback"].map(str::to_owned).into(),
        };
        ensure!(!creature_names.is_empty(), "art pack names no creatures");
        ensure!(
            creature_names.len() <= 64,
            "art pack names {} creatures; at most 64 are supported",
            creature_names.len()
        );
        for (i, name) in creature_names.iter().enumerate() {
            ensure!(!name.is_empty(), "creature {i} has an empty name");
            ensure!(
                creature_names[..i].iter().all(|other| other != name),
                "duplicate creature name {name}"
            );
        }
        let (width, height, rgba) = read_rgba(&directory.join("creatures.png"))?;
        ensure!(
            (width, height) == (16 * frames, 16 * STATES.len() * creature_names.len()),
            "creature atlas must be {}×{} RGBA8 for {} creatures of {frames} frames, got {width}×{height}",
            16 * frames,
            16 * STATES.len() * creature_names.len(),
            creature_names.len()
        );
        let rows = meta["clips"].as_array().context("missing clips")?;
        ensure!(
            rows.len() == STATES.len() * creature_names.len(),
            "art pack must contain {} clips for {} creatures, has {}",
            STATES.len() * creature_names.len(),
            creature_names.len(),
            rows.len()
        );
        let mut clips = Vec::new();
        for (row, entry) in rows.iter().enumerate() {
            ensure!(
                entry["name"] == creature_names[row / 4].as_str()
                    && entry["state"] == STATES[row % 4]
                    && entry["row"].as_u64() == Some(row as u64),
                "unexpected clip order at row {row}"
            );
            let seconds = entry["seconds"].as_f64().context("clip needs seconds")?;
            ensure!(
                seconds.is_finite() && seconds > 0.0,
                "invalid clip duration"
            );
            let looping = entry["loop"].as_bool().context("clip needs loop flag")?;
            ensure!(
                looping == (row % 4 != 3),
                "bud must be nonlooping; rest/move/feed must loop"
            );
            let frames = (0..frames)
                .map(|column| tile(&rgba, width, column * 16, row * 16))
                .collect::<Result<Vec<_>>>()?;
            clips.push(Clip {
                frames,
                seconds,
                looping,
            });
        }
        let (width, height, rgba) = read_rgba(&directory.join("habitat.png"))?;
        ensure!(
            (width, height) == (48, 16),
            "habitat atlas must be 48×16 RGBA8"
        );
        let habitat = (0..3)
            .map(|column| tile(&rgba, width, column * 16, 0))
            .collect::<Result<Vec<_>>>()?;
        let plant_frames = if version >= 2 {
            let n = frame_count(&meta["plant_frames"], "plant_frames")?;
            ensure!(version >= 4 || n == 4, "packs before v4 carry four plant frames");
            n
        } else {
            4
        };
        let plants = if version >= 2 {
            load_plants(directory, &meta, plant_frames, version)?
        } else {
            Vec::new()
        };
        let (tall, ground) = if version >= 3 {
            (load_tall(directory, &meta, plant_frames)?, load_ground(directory, &meta)?)
        } else {
            (Vec::new(), Vec::new())
        };
        Ok(Self { creature_names, clips, habitat, plants, tall, ground })
    }

    /// Samples per creature clip (every creature clip in a pack has the same count).
    pub fn creature_frames(&self) -> usize {
        self.clips.first().map_or(0, |c| c.frames.len())
    }

    /// Samples per plant and tall-plant clip (all share one count); 0 before pack v2.
    pub fn plant_frames(&self) -> usize {
        self.plants.first().map_or(0, |p| p.stages[0].frames.len())
    }

    /// The tall plant with this asset name, if the pack has it.
    pub fn tall_plant(&self, name: &str) -> Option<&TallPlant> {
        self.tall.iter().find(|p| p.name == name)
    }

    /// The ground tile for a band, if the pack has one.
    pub fn ground_for(&self, band: Band) -> Option<&GroundTile> {
        self.ground.iter().find(|g| g.band == band)
    }

    /// Creature rig names in atlas order; `creature(form, ..)` indexes this list.
    pub fn creature_names(&self) -> &[String] {
        &self.creature_names
    }

    /// Number of creature rigs in the pack (`clips.len() / 4`).
    pub fn creature_count(&self) -> usize {
        self.creature_names.len()
    }

    /// The plant with this asset name, if the pack has it.
    pub fn plant(&self, name: &str) -> Option<&Plant> {
        self.plants.iter().find(|p| p.name == name)
    }

    /// The frame of creature `form`'s clip `state` (0 rest, 1 move, 2 feed, 3 bud) at
    /// `seconds`. Panics on a `form` or `state` beyond the pack; callers map a genome's
    /// `form` into range first (`form % creature_count()`).
    pub fn creature(&self, form: usize, state: usize, seconds: f64) -> &Sprite {
        self.clips[form * 4 + state].at(seconds)
    }
}

/// A sample count from the pack, within [`MIN_FRAMES`]`..=`[`MAX_FRAMES`].
fn frame_count(value: &serde_json::Value, what: &str) -> Result<usize> {
    let n = value.as_u64().with_context(|| format!("pack needs {what}"))? as usize;
    ensure!(
        (MIN_FRAMES..=MAX_FRAMES).contains(&n),
        "{what} must be {MIN_FRAMES}..={MAX_FRAMES}, got {n}"
    );
    Ok(n)
}

/// A plant being assembled from its rows: name, band, stage clips, fruit, growth clips.
type PendingPlant = Option<(String, Band, Vec<Clip>, Option<Clip>, Vec<Transition>)>;

/// Pack v2: `plants.png` is `plant_frames` tiles wide, one row per (plant, stage) in
/// plant-major order — stages 0, 1, 2 and then, for plants that have one, `"fruit"`.
/// Pack v5 appends to each plant, after its own stage and fruit rows, one row per authored
/// growth transition (`stage: "grow"` with `from`, `to` and `loop: false`); the row order
/// stays plant-major, so a v5 pack differs from a v4 one only by the inserted rows.
fn load_plants(
    directory: &Path,
    meta: &serde_json::Value,
    plant_frames: usize,
    version: u64,
) -> Result<Vec<Plant>> {
    ensure!(meta["plant_atlas"] == "plants.png", "unsupported plant atlas layout");
    let rows = meta["plants"].as_array().context("pack v2 needs a plants array")?;
    ensure!(!rows.is_empty(), "pack v2 has no plant rows");
    let (width, height, rgba) = read_rgba(&directory.join("plants.png"))?;
    ensure!(
        width == 16 * plant_frames && height == 16 * rows.len(),
        "plant atlas must be {}×{} RGBA8 for {} rows, got {width}×{height}",
        16 * plant_frames,
        16 * rows.len(),
        rows.len()
    );
    // Every frame goes through `tile`, so a transition frame passes the same 9-pixel
    // extent budget as a stage frame (`Sprite::from_rgba` rejects a wider one).
    let clip_at = |row: usize, seconds: f64, looping: bool| -> Result<Clip> {
        ensure!(seconds.is_finite() && seconds > 0.0, "invalid plant clip duration");
        let frames = (0..plant_frames)
            .map(|column| tile(&rgba, width, column * 16, row * 16))
            .collect::<Result<Vec<_>>>()?;
        Ok(Clip { frames, seconds, looping })
    };
    let mut plants: Vec<Plant> = Vec::new();
    // Rows arrive plant-major; a plant closes when the next name appears.
    let mut pending: PendingPlant = None;
    let close = |pending: &mut PendingPlant, plants: &mut Vec<Plant>| -> Result<()> {
        if let Some((name, band, stages, fruit, transitions)) = pending.take() {
            ensure!(stages.len() == 3, "plant {name} must have exactly three stages");
            let mut it = stages.into_iter();
            let stages = [it.next().unwrap(), it.next().unwrap(), it.next().unwrap()];
            ensure!(plants.iter().all(|p| p.name != name), "duplicate plant {name}");
            plants.push(Plant { name, band, stages, fruit, transitions });
        }
        Ok(())
    };
    for (row, entry) in rows.iter().enumerate() {
        ensure!(entry["row"].as_u64() == Some(row as u64), "plant rows must be sequential");
        let name = entry["name"].as_str().context("plant row needs a name")?;
        let band = Band::parse(entry["band"].as_str().context("plant row needs a band")?)
            .with_context(|| format!("unknown band on plant {name}"))?;
        let seconds = entry["seconds"].as_f64().context("plant row needs seconds")?;
        ensure!(entry["frames"] == plant_frames as u64, "plant rows carry {plant_frames} frames");
        if pending.as_ref().is_none_or(|(n, ..)| n != name) {
            close(&mut pending, &mut plants)?;
            pending = Some((name.to_string(), band, Vec::new(), None, Vec::new()));
        }
        let slot = pending.as_mut().unwrap();
        ensure!(slot.1 == band, "plant {name} changes band between rows");
        match &entry["stage"] {
            v if v.as_u64() == Some(slot.2.len() as u64)
                && slot.3.is_none()
                && slot.4.is_empty() =>
            {
                slot.2.push(clip_at(row, seconds, true)?)
            }
            v if v == "fruit" && slot.2.len() == 3 && slot.3.is_none() && slot.4.is_empty() => {
                slot.3 = Some(clip_at(row, seconds, true)?)
            }
            // Pack v5: a growth transition, after this plant's stage and fruit rows.
            v if v == "grow" && slot.2.len() == 3 => {
                ensure!(version >= 5, "growth transitions need pack v5, not v{version}");
                let step = |key: &str| -> Result<u8> {
                    let value = entry[key]
                        .as_u64()
                        .with_context(|| format!("plant {name}: growth row {row} needs {key}"))?;
                    u8::try_from(value)
                        .with_context(|| format!("plant {name}: growth row {row} {key} {value}"))
                };
                let (from, to) = (step("from")?, step("to")?);
                ensure!(
                    from < 2 && from + 1 == to,
                    "plant {name}: growth row {row} goes {from} → {to}; a transition is one stage step up"
                );
                ensure!(
                    slot.4.iter().all(|t: &Transition| (t.from, t.to) != (from, to)),
                    "plant {name}: two growth clips for {from} → {to}"
                );
                ensure!(
                    entry["loop"].as_bool() == Some(false),
                    "plant {name}: growth row {row} must be marked loop: false"
                );
                slot.4.push(Transition { from, to, clip: clip_at(row, seconds, false)? });
            }
            other => anyhow::bail!("plant {name}: unexpected stage {other} at row {row}"),
        }
    }
    close(&mut pending, &mut plants)?;
    Ok(plants)
}

/// Pack v3: `tall.png` is `plant_frames` tiles wide, one row per (tall plant, part) in
/// plant-major order with parts in `base`, `trunk`, `crown` order, whichever the plant has.
/// A tall plant being assembled from its rows: name, then base, trunk, crown.
type PendingTall = Option<(String, Option<Clip>, Option<Clip>, Option<Clip>)>;

fn load_tall(directory: &Path, meta: &serde_json::Value, plant_frames: usize) -> Result<Vec<TallPlant>> {
    ensure!(meta["tall_atlas"] == "tall.png", "unsupported tall atlas layout");
    let rows = meta["tall"].as_array().context("pack v3 needs a tall array")?;
    ensure!(!rows.is_empty(), "pack v3 has no tall plant rows");
    let (width, height, rgba) = read_rgba(&directory.join("tall.png"))?;
    ensure!(
        width == 16 * plant_frames && height == 16 * rows.len(),
        "tall atlas must be {}×{} RGBA8 for {} rows, got {width}×{height}",
        16 * plant_frames,
        16 * rows.len(),
        rows.len()
    );
    let clip_at = |row: usize, seconds: f64| -> Result<Clip> {
        ensure!(seconds.is_finite() && seconds > 0.0, "invalid tall clip duration");
        let frames = (0..plant_frames)
            .map(|column| tile(&rgba, width, column * 16, row * 16))
            .collect::<Result<Vec<_>>>()?;
        Ok(Clip { frames, seconds, looping: true })
    };
    let mut out: Vec<TallPlant> = Vec::new();
    let mut pending: PendingTall = None;
    let close = |pending: &mut PendingTall, out: &mut Vec<TallPlant>|
     -> Result<()> {
        if let Some((name, base, trunk, crown)) = pending.take() {
            let trunk = trunk.with_context(|| format!("tall plant {name} has no trunk"))?;
            ensure!(out.iter().all(|p| p.name != name), "duplicate tall plant {name}");
            let mut tail_row = 16;
            let cap = match &crown {
                Some(crown) => {
                    // The cap is cut from the crown by pairing same-index frames with the
                    // trunk's, which is only meaningful when the two clips run in step.
                    ensure!(
                        crown.frames.len() == trunk.frames.len() && crown.seconds == trunk.seconds,
                        "tall plant {name}: crown ({} frames, {} s) and trunk ({} frames, {} s) must share one sample count and duration",
                        crown.frames.len(),
                        crown.seconds,
                        trunk.frames.len(),
                        trunk.seconds
                    );
                    tail_row = tail_row_of(crown, &trunk);
                    ensure!(
                        tail_row >= CROWN_TAIL_MIN_ROW && tail_row < 16,
                        "tall plant {name}: crown tail starts at row {tail_row}; it must join the trunk from a row in {CROWN_TAIL_MIN_ROW}..16"
                    );
                    Some(Clip {
                        frames: crown
                            .frames
                            .iter()
                            .zip(&trunk.frames)
                            .map(|(c, t)| c.subtract(t, tail_row..16))
                            .collect(),
                        seconds: crown.seconds,
                        looping: crown.looping,
                    })
                }
                None => None,
            };
            out.push(TallPlant { name, base, trunk, crown, cap, tail_row });
        }
        Ok(())
    };
    for (row, entry) in rows.iter().enumerate() {
        ensure!(entry["row"].as_u64() == Some(row as u64), "tall rows must be sequential");
        let name = entry["name"].as_str().context("tall row needs a name")?;
        let seconds = entry["seconds"].as_f64().context("tall row needs seconds")?;
        ensure!(entry["frames"] == plant_frames as u64, "tall rows carry {plant_frames} frames");
        if pending.as_ref().is_none_or(|(n, ..)| n != name) {
            close(&mut pending, &mut out)?;
            pending = Some((name.to_string(), None, None, None));
        }
        let slot = pending.as_mut().unwrap();
        let clip = clip_at(row, seconds)?;
        // Parts arrive in base, trunk, crown order; each at most once.
        match entry["part"].as_str() {
            Some("base") if slot.1.is_none() && slot.2.is_none() && slot.3.is_none() => slot.1 = Some(clip),
            Some("trunk") if slot.2.is_none() && slot.3.is_none() => slot.2 = Some(clip),
            Some("crown") if slot.2.is_some() && slot.3.is_none() => slot.3 = Some(clip),
            other => anyhow::bail!("tall plant {name}: unexpected part {other:?} at row {row}"),
        }
    }
    close(&mut pending, &mut out)?;
    Ok(out)
}

/// Pack v3: `ground.png` is `GROUND_FRAMES` 8×8 tiles wide, one row per ground tile.
const GROUND_TILE: usize = 8;
const GROUND_FRAMES: usize = 4;

fn load_ground(directory: &Path, meta: &serde_json::Value) -> Result<Vec<GroundTile>> {
    ensure!(
        meta["ground_atlas"] == "ground.png"
            && meta["ground_tile"] == GROUND_TILE as u64
            && meta["ground_frames"] == GROUND_FRAMES as u64,
        "unsupported ground atlas layout"
    );
    let rows = meta["ground"].as_array().context("pack v3 needs a ground array")?;
    ensure!(!rows.is_empty(), "pack v3 has no ground rows");
    let (width, height, rgba) = read_rgba(&directory.join("ground.png"))?;
    ensure!(
        width == GROUND_TILE * GROUND_FRAMES && height == GROUND_TILE * rows.len(),
        "ground atlas must be {}×{} RGBA8 for {} rows, got {width}×{height}",
        GROUND_TILE * GROUND_FRAMES,
        GROUND_TILE * rows.len(),
        rows.len()
    );
    let mut out: Vec<GroundTile> = Vec::new();
    for (row, entry) in rows.iter().enumerate() {
        ensure!(entry["row"].as_u64() == Some(row as u64), "ground rows must be sequential");
        let name = entry["name"].as_str().context("ground row needs a name")?.to_string();
        let band = Band::parse(entry["band"].as_str().context("ground row needs a band")?)
            .with_context(|| format!("unknown band on ground tile {name}"))?;
        let seconds = entry["seconds"].as_f64().context("ground row needs seconds")?;
        ensure!(seconds.is_finite() && seconds > 0.0, "invalid ground clip duration");
        ensure!(entry["frames"] == GROUND_FRAMES as u64, "ground rows carry {GROUND_FRAMES} frames");
        ensure!(out.iter().all(|g| g.name != name), "duplicate ground tile {name}");
        ensure!(out.iter().all(|g| g.band != band), "two ground tiles for {}", band.name());
        let frames = (0..GROUND_FRAMES)
            .map(|column| {
                let (x, y) = (column * GROUND_TILE, row * GROUND_TILE);
                let mut px = Vec::with_capacity(GROUND_TILE * GROUND_TILE * 4);
                for r in y..y + GROUND_TILE {
                    px.extend_from_slice(&rgba[(r * width + x) * 4..(r * width + x + GROUND_TILE) * 4]);
                }
                Sprite::from_rgba(GROUND_TILE, GROUND_TILE, Vec2::new(4.0, 4.0), &px)
                    .map_err(anyhow::Error::msg)
            })
            .collect::<Result<Vec<_>>>()?;
        out.push(GroundTile { name, band, frames, seconds });
    }
    Ok(out)
}

fn tile(bytes: &[u8], width: usize, x: usize, y: usize) -> Result<Sprite> {
    let mut rgba = Vec::with_capacity(16 * 16 * 4);
    for row in y..y + 16 {
        rgba.extend_from_slice(&bytes[(row * width + x) * 4..(row * width + x + 16) * 4]);
    }
    Sprite::from_rgba(16, 16, Vec2::new(8.0, 8.0), &rgba).map_err(anyhow::Error::msg)
}

fn read_rgba(path: &Path) -> Result<(usize, usize, Vec<u8>)> {
    let file = std::io::BufReader::new(
        std::fs::File::open(path).with_context(|| format!("open {}", path.display()))?,
    );
    let decoder = png::Decoder::new(file);
    let mut reader = decoder.read_info()?;
    let info = reader.info();
    ensure!(
        info.width <= 512
            && info.height <= 4096
            && info.color_type == png::ColorType::Rgba
            && info.bit_depth == png::BitDepth::Eight,
        "atlas must be small RGBA8 PNG"
    );
    let mut bytes = vec![
        0;
        reader
            .output_buffer_size()
            .context("atlas allocation overflow")?
    ];
    let info = reader.next_frame(&mut bytes)?;
    bytes.truncate(info.buffer_size());
    Ok((info.width as usize, info.height as usize, bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn the_baked_art_loads_and_move_feed_and_bud_have_visible_animation() {
        let art =
            ArtPack::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/atelier"))
                .unwrap();
        assert_eq!(art.creature_count(), 4);
        assert_eq!(art.clips.len(), 4 * art.creature_count());
        for (form, name) in art.creature_names().iter().enumerate() {
            for (state, state_name) in STATES.iter().enumerate().skip(1) {
                let clip = &art.clips[form * 4 + state];
                let images = clip
                    .frames
                    .iter()
                    .map(|s| format!("{s:?}"))
                    .collect::<std::collections::HashSet<_>>();
                assert!(images.len() > 1, "{name} {state_name} is frozen");
            }
        }
        let bud = &art.clips[3];
        assert!(std::ptr::eq(bud.at(bud.seconds * 4.0), bud.frames.last().unwrap()));
        let movement = &art.clips[1];
        assert!(std::ptr::eq(
            movement.at(movement.seconds),
            &movement.frames[0]
        ));
    }

    #[test]
    fn creature_names_are_unique_match_the_clip_rows_and_every_tile_fits_the_budget() {
        let art =
            ArtPack::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/atelier"))
                .unwrap();
        let names = art.creature_names();
        assert_eq!(names, ["lantern", "sail", "mossback", "skimmer"]);
        let unique: std::collections::HashSet<_> = names.iter().collect();
        assert_eq!(unique.len(), names.len(), "creature names must be unique");
        // The pack.json rows carry the same names in the same creature-major order.
        let meta: serde_json::Value = serde_json::from_reader(
            std::fs::File::open(
                Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/atelier/pack.json"),
            )
            .unwrap(),
        )
        .unwrap();
        for (row, entry) in meta["clips"].as_array().unwrap().iter().enumerate() {
            assert_eq!(entry["name"], names[row / 4].as_str(), "row {row}");
            assert_eq!(entry["state"], STATES[row % 4], "row {row}");
        }
        assert!((MIN_FRAMES..=MAX_FRAMES).contains(&art.creature_frames()));
        for (i, clip) in art.clips.iter().enumerate() {
            assert_eq!(clip.frames.len(), art.creature_frames(), "clip {i}");
            for (f, frame) in clip.frames.iter().enumerate() {
                assert!(
                    frame.extent() > 0.0 && frame.extent() <= 9.0,
                    "{} {} frame {f}: extent {}",
                    names[i / 4],
                    STATES[i % 4],
                    frame.extent()
                );
            }
        }
        // The last rig's frames are reachable through the same accessor as the first's.
        let last = art.creature_count() - 1;
        assert!(std::ptr::eq(art.creature(last, 0, 0.0), &art.clips[last * 4].frames[0]));
    }

    fn distinct_frames(clip: &Clip) -> usize {
        clip.frames
            .iter()
            .map(|s| format!("{s:?}"))
            .collect::<std::collections::HashSet<_>>()
            .len()
    }

    #[test]
    fn the_baked_plants_have_three_growing_stages_a_band_and_fruit_where_promised() {
        let art =
            ArtPack::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/atelier"))
                .unwrap();
        // name, band, fruit clip, authored growth transitions (pack v5).
        let expected: [(&str, Band, bool, &[(u8, u8)]); 7] = [
            ("glowcap", Band::Soil, false, &[(0, 1), (1, 2)]),
            ("rootveil", Band::Soil, false, &[(0, 1), (1, 2)]),
            ("lanternstalk", Band::Foliage, true, &[(0, 1), (1, 2)]),
            ("tendrilfan", Band::Foliage, true, &[(0, 1), (1, 2)]),
            ("umbrellafrond", Band::Canopy, false, &[(0, 1), (1, 2)]),
            ("bloomcrown", Band::Canopy, true, &[(0, 1), (1, 2)]),
            ("reedspire", Band::Water, false, &[(0, 1), (1, 2)]),
        ];
        assert_eq!(art.plants.len(), expected.len());
        for (plant, (name, band, fruit, transitions)) in art.plants.iter().zip(expected) {
            assert_eq!(plant.name, name);
            assert_eq!(plant.band, band, "{name}");
            assert_eq!(plant.fruit.is_some(), fruit, "{name} fruit");
            assert_eq!(
                plant.transitions.iter().map(|t| (t.from, t.to)).collect::<Vec<_>>(),
                transitions,
                "{name} growth transitions"
            );
            for step in transitions {
                let clip = plant.transition(step.0, step.1).expect("declared transition");
                assert!(!clip.looping, "{name} {step:?} must not loop");
                assert_eq!(clip.frames.len(), art.plant_frames(), "{name} {step:?}");
                assert!(clip.seconds > 0.0, "{name} {step:?}");
                for frame in &clip.frames {
                    assert!(frame.extent() > 0.0 && frame.extent() <= 9.0, "{name} {step:?}");
                }
            }
            assert!(std::ptr::eq(art.plant(name).unwrap(), plant));
            let clips = plant.stages.iter().chain(plant.fruit.iter());
            for (i, clip) in clips.enumerate() {
                assert_eq!(clip.frames.len(), art.plant_frames(), "{name} clip {i}");
                assert!(clip.looping && clip.seconds > 0.0, "{name} clip {i} must loop");
                let need = if i == 0 { 2 } else { 4 };
                assert!(
                    distinct_frames(clip) >= need,
                    "{name} clip {i} has {} distinct frames, needs {need}",
                    distinct_frames(clip)
                );
                for frame in &clip.frames {
                    assert!(frame.extent() > 0.0 && frame.extent() <= 9.0, "{name} clip {i}");
                }
            }
        }
        assert!(art.plant("nope").is_none());
        // A fruit clip is the full-grown plant with something bright on it: it differs
        // from stage 2 in every frame.
        let bloom = art.plant("bloomcrown").unwrap();
        let fruit = bloom.fruit.as_ref().unwrap();
        for (a, b) in fruit.frames.iter().zip(&bloom.stages[2].frames) {
            assert_ne!(format!("{a:?}"), format!("{b:?}"));
        }
    }

    fn atelier() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/atelier")
    }

    /// Alpha of pixel (x, y) in an RGBA8 image.
    fn alpha(rgba: &[u8], width: usize, x: usize, y: usize) -> i32 {
        i32::from(rgba[(y * width + x) * 4 + 3])
    }

    /// A pixel with fully transparent texels normalized to zero, since the baker leaves
    /// two different RGB values under alpha 0 and neither is ever visible.
    fn pixel(rgba: &[u8], width: usize, x: usize, y: usize) -> [u8; 4] {
        let i = (y * width + x) * 4;
        if rgba[i + 3] == 0 {
            [0; 4]
        } else {
            [rgba[i], rgba[i + 1], rgba[i + 2], rgba[i + 3]]
        }
    }

    #[test]
    fn the_baked_tall_plants_have_periodic_trunks_that_their_caps_join() {
        let art = ArtPack::load(&atelier()).unwrap();
        let expected = [("spiretree", true, true), ("glasscane", true, true), ("vinecoil", false, false)];
        assert_eq!(art.tall.len(), expected.len());
        let meta: serde_json::Value =
            serde_json::from_reader(std::fs::File::open(atelier().join("pack.json")).unwrap()).unwrap();
        let rows = meta["tall"].as_array().unwrap();
        let (width, _, rgba) = read_rgba(&atelier().join("tall.png")).unwrap();
        for (plant, (name, base, crown)) in art.tall.iter().zip(expected) {
            assert_eq!(plant.name, name);
            assert_eq!(plant.base.is_some(), base, "{name} base");
            assert_eq!(plant.crown.is_some(), crown, "{name} crown");
            assert!(std::ptr::eq(art.tall_plant(name).unwrap(), plant));
            let row_of = |part: &str| {
                rows.iter().position(|r| r["name"] == name && r["part"] == part).unwrap()
            };
            let trunk_row = row_of("trunk");
            for clip in [Some(&plant.trunk), plant.base.as_ref(), plant.crown.as_ref()].into_iter().flatten() {
                assert_eq!(clip.frames.len(), art.plant_frames());
                assert!(clip.looping && clip.seconds > 0.0);
                assert!(distinct_frames(clip) >= 4, "{name}: a column part needs four distinct frames");
                for frame in &clip.frames {
                    assert!(frame.extent() > 0.0 && frame.extent() <= 9.0, "{name} extent {}", frame.extent());
                }
            }
            for frame in 0..art.plant_frames() {
                let x0 = frame * 16;
                // A trunk segment is periodic in y with period 4, so any two segments
                // stacked a whole number of cells apart draw identical overlapping pixels.
                for y in 0..12 {
                    for x in 0..16 {
                        assert_eq!(
                            pixel(&rgba, width, x0 + x, trunk_row * 16 + y),
                            pixel(&rgba, width, x0 + x, trunk_row * 16 + y + 4),
                            "{name} trunk frame {frame} is not 4-periodic at ({x},{y})"
                        );
                    }
                }
                // A cap is drawn over the trunk segment it overlaps (the crown 4 px above
                // the top segment, the base 4 px below the bottom one). The join is
                // seamless when, in the overlap rows, every texel the trunk paints is
                // painted identically by the cap; the cap's own art beside the trunk (a
                // crown's bulbs, a base's roots) is free to cover empty ground.
                let joins = |cap: &str, rows: std::ops::Range<usize>| {
                    let cap_row = row_of(cap);
                    for y in rows {
                        for x in 0..16 {
                            let trunk = pixel(&rgba, width, x0 + x, trunk_row * 16 + y);
                            if trunk[3] == 0 {
                                continue;
                            }
                            assert_eq!(
                                pixel(&rgba, width, x0 + x, cap_row * 16 + y),
                                trunk,
                                "{name} {cap} frame {frame} does not join the trunk at ({x},{y})"
                            );
                        }
                    }
                };
                if crown {
                    joins("crown", 9..16);
                }
                if base {
                    joins("base", 3..9);
                }
            }
        }
        assert!(art.tall_plant("nope").is_none());
    }

    #[test]
    fn the_lanternstalk_grows_from_its_sprout_into_the_neutral_stage_one_image() {
        let art = ArtPack::load(&atelier()).unwrap();
        let plant = art.plant("lanternstalk").unwrap();
        let clip = plant.transition(0, 1).expect("pack v5 carries the growth pilot");
        // 1 → 2 is authored too (the growth expansion); only this pilot carries the
        // stricter "endpoint is a loop sample" property asserted below.
        assert!(plant.transition(1, 2).is_some(), "the 1 → 2 step is authored as well");
        assert!(plant.transition(1, 0).is_none(), "a transition only runs upward");
        assert!(!clip.looping);
        assert_eq!(clip.frames.len(), art.plant_frames());
        assert!(distinct_frames(clip) >= 4, "the growth clip must move");
        // Sampled inclusively, so the endpoints are the poses the presenter blends into.
        let (first, last) = (clip.frames.first().unwrap(), clip.frames.last().unwrap());
        assert_ne!(format!("{first:?}"), format!("{last:?}"), "the plant must have grown");
        assert!(std::ptr::eq(clip.sample(0.0).first, first));
        assert!(std::ptr::eq(clip.sample(clip.seconds).first, last));
        // The neutral pose of a stage is rotation 0 with modulate 1, which for these clips
        // is the *half-period* sample: stage 1's phase-0 sample has the bulb dimmed to 0.7
        // and stage 0's has the whole sprout at 0.7, so neither endpoint may be compared
        // against frame 0. Both stage clips are 3 s with `plant_frames` even samples, so the
        // half- and quarter-period samples land exactly on 1.5 s and 0.75 s.
        let half = art.plant_frames() / 2;
        assert_eq!(
            format!("{last:?}"),
            format!("{:?}", plant.stages[1].frames[half]),
            "the last growth frame must be the neutral Stalk1 image"
        );
        assert_ne!(
            format!("{last:?}"),
            format!("{:?}", plant.stages[1].frames[0]),
            "stage 1's phase-0 sample is the dim end of its pulse, not the neutral image"
        );
        assert_eq!(
            format!("{first:?}"),
            format!("{:?}", plant.stages[0].frames[half / 2]),
            "the first growth frame must be the sprout at modulate 1"
        );
        // The root never moves: tile row 14 is the lowest painted row in every frame.
        let meta: serde_json::Value =
            serde_json::from_reader(std::fs::File::open(atelier().join("pack.json")).unwrap())
                .unwrap();
        let row = meta["plants"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["name"] == "lanternstalk" && r["stage"] == "grow")
            .and_then(|r| r["row"].as_u64())
            .unwrap() as usize;
        let (width, _, rgba) = read_rgba(&atelier().join("plants.png")).unwrap();
        for frame in 0..art.plant_frames() {
            let x0 = frame * 16;
            let painted = |y: usize| (0..16).any(|x| alpha(&rgba, width, x0 + x, row * 16 + y) > 0);
            assert!(painted(14), "growth frame {frame} lifts off the root row");
            assert!(!painted(15), "growth frame {frame} paints below the root");
        }
    }

    #[test]
    fn the_baked_ground_tiles_cover_the_three_bands_and_wrap_without_a_seam() {
        let art = ArtPack::load(&atelier()).unwrap();
        let expected = [("grit", Band::Soil), ("mossweave", Band::Foliage), ("frondmat", Band::Canopy)];
        assert_eq!(art.ground.len(), expected.len());
        let (width, _, rgba) = read_rgba(&atelier().join("ground.png")).unwrap();
        for (row, (tile, (name, band))) in art.ground.iter().zip(expected).enumerate() {
            assert_eq!(tile.name, name);
            assert_eq!(tile.band, band);
            assert!(std::ptr::eq(art.ground_for(band).unwrap(), tile));
            assert_eq!(tile.frames.len(), GROUND_FRAMES);
            assert!(tile.seconds > 0.0);
            let distinct = tile
                .frames
                .iter()
                .map(|s| format!("{s:?}"))
                .collect::<std::collections::HashSet<_>>()
                .len();
            assert!(distinct >= 2, "{name} does not breathe");
            for (frame, sprite) in tile.frames.iter().enumerate() {
                assert!(sprite.extent() > 0.0 && sprite.extent() <= 9.0, "{name} frame {frame}");
                let (x0, y0) = (frame * GROUND_TILE, row * GROUND_TILE);
                let a = |x: usize, y: usize| alpha(&rgba, width, x0 + x, y0 + y);
                // Seam test: the alpha step across the wrap (column 7 → column 0 of the
                // next copy, and row 7 → row 0) must be no larger than the pattern's own
                // interior steps, so a 4×4 tiling shows no join line.
                let n = GROUND_TILE as i32;
                let mut interior = 0i32;
                let mut interior_count = 0i32;
                let mut wrap = 0i32;
                for y in 0..GROUND_TILE {
                    for x in 0..GROUND_TILE - 1 {
                        interior += (a(x, y) - a(x + 1, y)).abs();
                        interior_count += 1;
                    }
                    wrap += (a(GROUND_TILE - 1, y) - a(0, y)).abs();
                }
                for x in 0..GROUND_TILE {
                    for y in 0..GROUND_TILE - 1 {
                        interior += (a(x, y) - a(x, y + 1)).abs();
                        interior_count += 1;
                    }
                    wrap += (a(x, GROUND_TILE - 1) - a(x, 0)).abs();
                }
                let interior_mean = interior as f64 / interior_count as f64;
                let wrap_mean = wrap as f64 / (2 * n) as f64;
                assert!(
                    wrap_mean <= interior_mean * 1.25 + 2.0,
                    "{name} frame {frame}: wrap step {wrap_mean:.1} vs interior {interior_mean:.1}"
                );
                let coverage: i32 = (0..GROUND_TILE).flat_map(|y| (0..GROUND_TILE).map(move |x| (x, y))).map(|(x, y)| a(x, y)).sum();
                assert!(coverage > 0, "{name} frame {frame} is empty");
            }
        }
    }
}
