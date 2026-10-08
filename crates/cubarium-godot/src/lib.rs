//! **The Godot extension**: one class, `CubariumSession`, over [`cubarium_session::Session`].
//!
//! Thin on purpose: every call is one session call, and every value crosses as a generic
//! conversion — serde values become Dictionaries and Arrays, the view feed's columns become
//! packed arrays. A new species, stat or query needs no change here; only a new *kind* of
//! visible state adds a field to the feed (and one line below).
//!
//! Nothing here blocks on a tick: founding, loading and saving run off Godot's thread and
//! report through `status()`, `candidates()` and `take_notices()`.
//!
//! Paths are filesystem paths: GDScript passes `ProjectSettings.globalize_path("user://…")`.
//!
//! **Seeds are unsigned 64-bit and Godot's integers are signed**, so a seed crosses as a
//! decimal String both ways: every `seed` (and `placement_seed`) this class returns is a
//! String, and a spec's `seed` may be a String (any value up to 2^64 − 1) or an int (0 or
//! more; a negative int is read as the unsigned value with the same bits, which is what
//! `String.num_uint64` shows for it).

#![deny(unsafe_code)]

use std::path::PathBuf;

pub mod mesher;
pub mod models;
pub mod palette;
pub mod renderer;

use cubarium_observatory::Command;
use cubarium_session::{Session, SessionOptions, View, WorldSpec};
use godot::prelude::*;
use serde_json::Value;

// gdextension strips attributes from its impl, including a local lint allow.
// Keep the exception around only that impl and gdext's generated entry points.
#[allow(unsafe_code)]
mod extension {
    use godot::prelude::*;

    struct CubariumExtension;

    #[gdextension]
    unsafe impl ExtensionLibrary for CubariumExtension {
        // Safe code only: the one call below, defined outside this module.
        fn on_stage_deinit(_stage: godot::init::InitStage) {
            super::teardown::stop_all();
        }
    }
}

/// **Every thread this library starts is joined before Godot unloads it.** At exit Godot
/// unmaps the library; a thread still running its code then faults (SIGSEGV after "SHELL
/// OK", 2026-10-08). Each session and renderer stops and joins its threads when it is
/// dropped; this is the backstop for one still alive when the library is being torn down
/// (a reference GDScript never released, or released only after the extension's
/// deinitialisation): Godot calls [`stop_all`] at every deinit stage, the main loop's first.
mod teardown {
    use std::sync::Mutex;

    use cubarium::sink::gpu::voxel::hosted::HostedStop;
    use cubarium_session::SessionStop;

    static SESSIONS: Mutex<Vec<SessionStop>> = Mutex::new(Vec::new());
    static RENDERERS: Mutex<Vec<HostedStop>> = Mutex::new(Vec::new());

    /// Remember a session to stop at teardown (only weakly: it is dropped as usual).
    pub(crate) fn session(s: SessionStop) {
        let mut all = SESSIONS.lock().unwrap_or_else(|e| e.into_inner());
        all.retain(SessionStop::alive);
        all.push(s);
    }

    /// Remember a renderer to stop at teardown (only weakly).
    pub(crate) fn renderer(h: HostedStop) {
        let mut all = RENDERERS.lock().unwrap_or_else(|e| e.into_inner());
        all.retain(HostedStop::alive);
        all.push(h);
    }

    /// Stop and join the threads of every session, then every renderer, still alive.
    /// Sessions first: a session's loop runs its renderer's hook.
    pub(crate) fn stop_all() {
        let sessions = std::mem::take(&mut *SESSIONS.lock().unwrap_or_else(|e| e.into_inner()));
        let renderers = std::mem::take(&mut *RENDERERS.lock().unwrap_or_else(|e| e.into_inner()));
        let s = sessions.iter().filter(|s| s.shutdown()).count();
        let r = renderers.iter().filter(|h| h.shutdown()).count();
        if s + r > 0 {
            eprintln!("cubarium: stopped {s} session(s) and {r} renderer(s) still running at unload");
        }
    }
}

/// A running Cubarium world. `CubariumSession.new()` starts an empty session; make a world
/// with `generate_candidates` + `promote`, or `load`.
#[derive(GodotClass)]
#[class(base = RefCounted)]
pub struct CubariumSession {
    base: Base<RefCounted>,
    session: Session,
}

#[godot_api]
impl IRefCounted for CubariumSession {
    fn init(base: Base<RefCounted>) -> Self {
        let session = Session::new(SessionOptions::default());
        teardown::session(session.stopper());
        CubariumSession { base, session }
    }
}

#[godot_api]
impl CubariumSession {
    // --- Lifecycle ----------------------------------------------------------------------

    /// Found `count` candidate worlds in parallel from a spec Dictionary — `config` (a
    /// config file path), `size` ([w, h, d]), `terrain`, `climate`, `seed`, `sets`,
    /// `empty`; unknown keys are refused. Returns "" or the reason it was refused.
    #[func]
    fn generate_candidates(&self, spec: Variant, count: i64) -> GString {
        let result = world_spec(&spec).and_then(|s| self.session.generate_candidates(&s, count.max(0) as usize));
        error_string(result)
    }

    /// Draw one terrain candidate per entry of `seeds`, from `spec`: a seed (String or int)
    /// brings that terrain back — what the picker's water slider does to show the same
    /// landforms at another level — and `null` draws one. "" or the reason it was refused.
    #[func]
    fn regenerate_candidates(&self, spec: Variant, seeds: VarArray) -> GString {
        let seeds: Result<Vec<Option<u64>>, String> = seeds
            .iter_shared()
            .map(|v| if v.is_nil() { Ok(None) } else { seed_of(&to_json(&v)).map(Some) })
            .collect();
        let result = seeds.and_then(|seeds| {
            world_spec(&spec).and_then(|s| self.session.generate_candidates_seeded(&s, &seeds))
        });
        error_string(result)
    }

    /// The picked candidate's founding: `state` (idle, founding, done, failed, cancelled),
    /// `candidate`, `seed`, `terrain`, `phase`, `phase_label`, `progress` (0–1 through the
    /// current try), `attempt` of `attempts`, `placement_seed`, `elapsed_s`, `error`,
    /// `label`.
    #[func]
    fn founding(&self) -> Variant {
        to_variant(&serde_json::to_value(self.session.founding()).unwrap_or(Value::Null))
    }

    /// Stop the candidates still being drawn and the founding in flight.
    #[func]
    fn cancel(&self) {
        self.session.cancel();
    }

    /// Candidate and founding threads still running.
    #[func]
    fn workers(&self) -> i64 {
        self.session.workers() as i64
    }

    /// Every candidate — a bare terrain until it is picked: `index`, `state` (generating,
    /// ready, failed, cancelled), `seed`, `label`, `error`, `summary`, `populations`,
    /// `terrain` (its hash), `feasible` (whether the founders fit on the bare terrain),
    /// `notes`.
    #[func]
    fn candidates(&self) -> Variant {
        to_variant(&serde_json::to_value(self.session.candidates()).unwrap_or(Value::Null))
    }

    /// A ready candidate's whole view (the `take_view` layout, every chunk), or an empty
    /// Dictionary while it is being founded. It also names where it came from, `source` (this
    /// session) and `candidate` (the index), so a world view can render the candidate's world
    /// itself (`WorldRenderer.show_candidate`).
    #[func]
    fn candidate_view(&self, index: i64) -> VarDictionary {
        let mut d = usize::try_from(index)
            .ok()
            .and_then(|i| self.session.candidate_view(i))
            .map_or_else(VarDictionary::new, |v| view_dict(&v));
        if !d.is_empty() {
            put(&mut d, "source", self.to_gd());
            put(&mut d, "candidate", index);
        }
        d
    }

    /// What a world view drawn by the renderer needs of a ready candidate: `source` (this
    /// session) and `candidate` (the index), for `WorldView.show_view`; an empty Dictionary
    /// while it is being drawn. Unlike `candidate_view` it converts nothing of the world, so
    /// it costs Godot's thread nothing on the widest world.
    #[func]
    fn candidate_ref(&self, index: i64) -> VarDictionary {
        let mut d = VarDictionary::new();
        if usize::try_from(index).is_ok_and(|i| self.session.candidate_ready(i)) {
            put(&mut d, "source", self.to_gd());
            put(&mut d, "candidate", index);
        }
        d
    }

    /// Found candidate `index` on its own terrain, and run it once founded (the others are
    /// dropped then); follow it through `founding()`. "" or the reason it can't start.
    #[func]
    fn promote(&self, index: i64) -> GString {
        let result = usize::try_from(index)
            .map_err(|_| format!("no candidate {index}"))
            .and_then(|i| self.session.promote(i));
        error_string(result)
    }

    /// Load the save directory at `path` in place of the running world. Returns at once:
    /// `status().generation` moves when it has loaded, and a refusal (a save from another
    /// version, say) comes as a `load_failed` notice.
    #[func]
    fn load(&self, path: GString) {
        self.session.load(&PathBuf::from(path.to_string()));
    }

    /// Save the whole world into the directory `path`, with its `meta.json`. Returns at
    /// once; the outcome comes as a `saved` or `save_failed` notice.
    #[func]
    fn save(&self, path: GString) {
        self.session.save(&PathBuf::from(path.to_string()));
    }

    /// Stop and drop the running world.
    #[func]
    fn close(&self) {
        self.session.close();
    }

    // --- Run control --------------------------------------------------------------------

    /// A console line (the stdin console's commands: `p`, `s N`, `speed X`, `r`, `i X Y Z`,
    /// …). Its reply comes as a `console` notice.
    #[func]
    fn command(&self, line: GString) {
        self.session.console(&line.to_string());
    }

    #[func]
    fn pause(&self) {
        self.session.command(Command::Pause);
    }

    #[func]
    fn resume(&self) {
        self.session.command(Command::Resume);
    }

    #[func]
    fn set_speed(&self, speed: f64) {
        self.session.command(Command::SetSpeed(speed));
    }

    /// Pause, and advance exactly `n` ticks.
    #[func]
    fn step(&self, n: i64) {
        self.session.command(Command::Step(n.clamp(1, i64::from(u32::MAX)) as u32));
    }

    // --- Reading ------------------------------------------------------------------------

    /// `running`, `paused`, `speed`, `tick`, `generation`, `loading`, `saving`, `label`,
    /// `last_error`, and the loop's own costs.
    #[func]
    fn status(&self) -> Variant {
        to_variant(&serde_json::to_value(self.session.status()).unwrap_or(Value::Null))
    }

    /// A query over the latest published snapshot (`summary`, `clock`, `environment`,
    /// `populations`, `run`, `animals`, `animal` {id}, `history` {since}, `species`,
    /// `status`, `candidates`, `saves` {dir}). On a refusal: `{"error": reason}`.
    #[func]
    fn query(&self, name: GString, args: Variant) -> Variant {
        let mut args = to_json(&args);
        if let Some(seed) = args.get("seed").cloned()
            && let Ok(n) = seed_of(&seed)
        {
            args["seed"] = Value::from(n);
        }
        match self.session.query(&name.to_string(), &args) {
            Ok(v) => to_variant(&v),
            Err(e) => to_variant(&serde_json::json!({ "error": e })),
        }
    }

    /// What changed since the last take (every chunk the first time after a world is
    /// installed) and the newest organisms and sky. See `game/README.md` for the layout.
    #[func]
    fn take_view(&self) -> VarDictionary {
        view_dict(&self.session.take_view())
    }

    /// Life events since the last take: Dictionaries with `tick`, `animal`, `kind` (born,
    /// meal, kill, injured, died, …) and the kind's own fields.
    #[func]
    fn take_events(&self) -> Variant {
        to_variant(&serde_json::to_value(self.session.take_events()).unwrap_or(Value::Null))
    }

    /// Console replies and save and load outcomes since the last take.
    #[func]
    fn take_notices(&self) -> Variant {
        to_variant(&serde_json::to_value(self.session.take_notices()).unwrap_or(Value::Null))
    }
}

/// `d[key] = v`, whatever `v` passes as.
fn put(d: &mut VarDictionary, key: &str, v: impl ToGodot) {
    d.set(&key.to_variant(), &v.to_variant());
}

fn error_string(r: Result<(), String>) -> GString {
    match r {
        Ok(()) => GString::new(),
        Err(e) => GString::from(e.as_str()),
    }
}

/// A view as a Dictionary: the world's shape, the chunks as one packed byte array, the
/// organisms as packed columns and the sky as a small Dictionary.
fn view_dict(v: &View) -> VarDictionary {
    let mut d = VarDictionary::new();
    let ints = |a: [u32; 3]| Vector3i::new(a[0] as i32, a[1] as i32, a[2] as i32);
    put(&mut d, "generation", v.generation as i64);
    put(&mut d, "dims", ints(v.dims));
    put(&mut d, "chunk_dims", ints(v.chunk_dims));
    put(&mut d, "chunk_size", cubarium_session::feed::CHUNK as i64);
    put(&mut d, "voxel_m", v.voxel_m);
    put(&mut d, "full", v.full);
    let mut coords: Vec<i32> = Vec::with_capacity(3 * v.chunks.len());
    let mut bytes: Vec<u8> = Vec::with_capacity(cubarium_session::feed::CHUNK_BYTES * v.chunks.len());
    for c in &v.chunks {
        coords.extend(c.at.map(|x| x as i32));
        bytes.extend_from_slice(&c.bytes);
    }
    put(&mut d, "chunk_coords", PackedInt32Array::from(coords.as_slice()));
    put(&mut d, "voxels", PackedByteArray::from(bytes.as_slice()));
    let mut marks = VarDictionary::new();
    let mut offsets = vec![0i32];
    let mut site: Vec<i32> = Vec::new();
    let mut kind: Vec<u8> = Vec::new();
    let mut species: Vec<i32> = Vec::new();
    let mut stock: Vec<f32> = Vec::new();
    let mut strength: Vec<f32> = Vec::new();
    for c in &v.chunks {
        for r in &c.marks {
            site.extend(r.site);
            kind.push(r.kind);
            species.push(r.species);
            stock.push(r.stock);
            strength.push(r.strength);
        }
        offsets.push(kind.len() as i32);
    }
    put(&mut marks, "offsets", PackedInt32Array::from(offsets.as_slice()));
    put(&mut marks, "site", PackedInt32Array::from(site.as_slice()));
    put(&mut marks, "kind", PackedByteArray::from(kind.as_slice()));
    put(&mut marks, "species", PackedInt32Array::from(species.as_slice()));
    put(&mut marks, "stock", PackedFloat32Array::from(stock.as_slice()));
    put(&mut marks, "strength", PackedFloat32Array::from(strength.as_slice()));
    put(&mut d, "marks", marks);
    let mut vines = VarDictionary::new();
    let mut offsets = vec![0i32];
    let mut site: Vec<i32> = Vec::new();
    let mut direction: Vec<u8> = Vec::new();
    let mut owner: Vec<i64> = Vec::new();
    let mut density: Vec<u8> = Vec::new();
    let mut accent: Vec<u8> = Vec::new();
    let mut flags: Vec<u8> = Vec::new();
    let mut tile: Vec<i32> = Vec::new();
    for c in &v.chunks {
        for r in &c.vines {
            site.extend(r.site);
            direction.push(r.direction);
            owner.push(r.owner);
            density.push(r.density);
            accent.push(r.accent);
            flags.push(r.flags);
            tile.push(r.tile);
        }
        offsets.push(direction.len() as i32);
    }
    put(&mut vines, "offsets", PackedInt32Array::from(offsets.as_slice()));
    put(&mut vines, "site", PackedInt32Array::from(site.as_slice()));
    put(&mut vines, "direction", PackedByteArray::from(direction.as_slice()));
    put(&mut vines, "owner", PackedInt64Array::from(owner.as_slice()));
    put(&mut vines, "density", PackedByteArray::from(density.as_slice()));
    put(&mut vines, "accent", PackedByteArray::from(accent.as_slice()));
    put(&mut vines, "flags", PackedByteArray::from(flags.as_slice()));
    put(&mut vines, "tile", PackedInt32Array::from(tile.as_slice()));
    put(&mut d, "vines", vines);
    if let Some(f) = &v.frame {
        put(&mut d, "tick", f.tick as i64);
        let a = &f.animals;
        let mut animals = VarDictionary::new();
        put(&mut animals, "ids", PackedInt64Array::from(a.ids.as_slice()));
        put(&mut animals, "species", PackedInt32Array::from(a.species.as_slice()));
        put(&mut animals, "pos", PackedFloat32Array::from(a.pos.as_slice()));
        put(&mut animals, "heading", PackedFloat32Array::from(a.heading.as_slice()));
        put(&mut animals, "state", PackedByteArray::from(a.state.as_slice()));
        put(&mut animals, "mobility", PackedByteArray::from(a.mobility.as_slice()));
        put(&mut animals, "stage", PackedByteArray::from(a.stage.as_slice()));
        put(&mut animals, "flags", PackedByteArray::from(a.flags.as_slice()));
        put(&mut animals, "size", PackedFloat32Array::from(a.size.as_slice()));
        put(&mut animals, "riding_host", PackedInt64Array::from(a.riding_host.as_slice()));
        put(&mut animals, "wall_toward", PackedInt32Array::from(a.wall_toward.as_slice()));
        put(&mut d, "animals", animals);
        let p = &f.plants;
        let mut plants = VarDictionary::new();
        put(&mut plants, "ids", PackedInt64Array::from(p.ids.as_slice()));
        put(&mut plants, "species", PackedInt32Array::from(p.species.as_slice()));
        put(&mut plants, "site", PackedInt32Array::from(p.site.as_slice()));
        put(&mut plants, "growth", PackedFloat32Array::from(p.growth.as_slice()));
        put(&mut plants, "foliage", PackedFloat32Array::from(p.foliage.as_slice()));
        put(&mut plants, "moisture", PackedFloat32Array::from(p.moisture.as_slice()));
        put(&mut plants, "wilt", PackedByteArray::from(p.wilt.as_slice()));
        put(&mut plants, "ripeness", PackedFloat64Array::from(p.ripeness.as_slice()));
        put(&mut plants, "parcel_fullness", PackedFloat64Array::from(p.parcel_fullness.as_slice()));
        put(&mut plants, "foliage_offsets", PackedInt32Array::from(p.foliage_offsets.as_slice()));
        put(&mut plants, "foliage_stock", PackedFloat64Array::from(p.foliage_stock.as_slice()));
        put(&mut plants, "foliage_capacity", PackedFloat64Array::from(p.foliage_capacity.as_slice()));
        put(&mut d, "plants", plants);
        let c = &f.carcasses;
        let mut carcasses = VarDictionary::new();
        put(&mut carcasses, "ids", PackedInt64Array::from(c.ids.as_slice()));
        put(&mut carcasses, "species", PackedInt32Array::from(c.species.as_slice()));
        put(&mut carcasses, "pos", PackedFloat32Array::from(c.pos.as_slice()));
        put(&mut carcasses, "heading", PackedFloat32Array::from(c.heading.as_slice()));
        put(&mut carcasses, "state", PackedByteArray::from(c.state.as_slice()));
        put(&mut carcasses, "remaining", PackedFloat32Array::from(c.remaining.as_slice()));
        put(&mut carcasses, "organic", PackedFloat32Array::from(c.organic.as_slice()));
        put(&mut d, "carcasses", carcasses);
        put(&mut d, "sky", to_variant(&serde_json::to_value(&f.sky).unwrap_or(Value::Null)));
    }
    d
}

/// A spec Dictionary as a [`WorldSpec`], its `seed` read by [`seed_of`].
fn world_spec(spec: &Variant) -> Result<WorldSpec, String> {
    let mut json = to_json(spec);
    if let Some(seed) = json.get("seed").cloned()
        && !seed.is_null()
    {
        json["seed"] = Value::from(seed_of(&seed)?);
    }
    serde_json::from_value(json).map_err(|e| format!("the world spec: {e}"))
}

/// A seed from Godot: a decimal String (up to 2^64 − 1), or an int — 0 or more, and a
/// negative int as the unsigned value with the same bits (`String.num_uint64` of it).
fn seed_of(v: &Value) -> Result<u64, String> {
    match v {
        Value::String(s) => s
            .trim()
            .parse::<u64>()
            .map_err(|_| format!("a seed is a whole number from 0 to {}, not \"{s}\"", u64::MAX)),
        Value::Number(n) => n
            .as_u64()
            .or_else(|| n.as_i64().map(|i| i as u64))
            .ok_or_else(|| format!("a seed is a whole number, not {n}")),
        other => Err(format!("a seed is a whole number, not {other}")),
    }
}

/// Keys whose values are seeds, sent to Godot as decimal Strings (see the module notes).
const SEED_KEYS: [&str; 2] = ["seed", "placement_seed"];

/// A serde value as a Variant: objects become Dictionaries, arrays Arrays. A seed
/// ([`SEED_KEYS`]) becomes a decimal String.
fn to_variant(v: &Value) -> Variant {
    match v {
        Value::Null => Variant::nil(),
        Value::Bool(b) => b.to_variant(),
        Value::Number(n) => match (n.as_i64(), n.as_u64()) {
            (Some(i), _) => i.to_variant(),
            (None, Some(u)) => (u as i64).to_variant(),
            _ => n.as_f64().unwrap_or(0.0).to_variant(),
        },
        Value::String(s) => GString::from(s.as_str()).to_variant(),
        Value::Array(items) => {
            let mut a = VarArray::new();
            for x in items {
                a.push(&to_variant(x));
            }
            a.to_variant()
        }
        Value::Object(map) => {
            let mut d = VarDictionary::new();
            for (k, x) in map {
                let value = match x {
                    Value::Number(n) if SEED_KEYS.contains(&k.as_str()) => match n.as_u64() {
                        Some(u) => GString::from(u.to_string().as_str()).to_variant(),
                        None => to_variant(x),
                    },
                    _ => to_variant(x),
                };
                d.set(&GString::from(k.as_str()).to_variant(), &value);
            }
            d.to_variant()
        }
    }
}

/// A Variant as a serde value: Dictionaries become objects (keys as strings), Arrays and
/// packed arrays arrays.
fn to_json(v: &Variant) -> Value {
    match v.get_type() {
        VariantType::NIL => Value::Null,
        VariantType::BOOL => Value::Bool(v.to::<bool>()),
        VariantType::INT => Value::from(v.to::<i64>()),
        VariantType::FLOAT => Value::from(v.to::<f64>()),
        VariantType::STRING | VariantType::STRING_NAME | VariantType::NODE_PATH => Value::String(v.to_string()),
        VariantType::ARRAY => Value::Array(v.to::<VarArray>().iter_shared().map(|x| to_json(&x)).collect()),
        VariantType::DICTIONARY => Value::Object(
            v.to::<VarDictionary>()
                .iter_shared()
                .map(|(k, x)| (k.to_string(), to_json(&x)))
                .collect(),
        ),
        VariantType::PACKED_INT32_ARRAY => Value::from(v.to::<PackedInt32Array>().to_vec()),
        VariantType::PACKED_INT64_ARRAY => Value::from(v.to::<PackedInt64Array>().to_vec()),
        VariantType::PACKED_FLOAT32_ARRAY => Value::from(v.to::<PackedFloat32Array>().to_vec()),
        VariantType::PACKED_FLOAT64_ARRAY => Value::from(v.to::<PackedFloat64Array>().to_vec()),
        VariantType::PACKED_STRING_ARRAY => Value::from(
            v.to::<PackedStringArray>().to_vec().iter().map(|s| s.to_string()).collect::<Vec<_>>(),
        ),
        VariantType::VECTOR3I => {
            let p = v.to::<Vector3i>();
            Value::from(vec![p.x, p.y, p.z])
        }
        _ => Value::String(v.to_string()),
    }
}
