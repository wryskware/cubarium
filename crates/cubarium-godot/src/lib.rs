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

use std::path::PathBuf;

use cubarium_observatory::Command;
use cubarium_session::{Session, SessionOptions, View, WorldSpec};
use godot::prelude::*;
use serde_json::Value;

struct CubariumExtension;

#[gdextension]
unsafe impl ExtensionLibrary for CubariumExtension {}

/// A running Cubarium world. `CubariumSession.new()` starts an empty session; make a world
/// with `generate_candidates` + `promote`, or `load`.
#[derive(GodotClass)]
#[class(base = RefCounted)]
pub struct CubariumSession {
    session: Session,
}

#[godot_api]
impl IRefCounted for CubariumSession {
    fn init(_base: Base<RefCounted>) -> Self {
        CubariumSession { session: Session::new(SessionOptions::default()) }
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
        let spec: Result<WorldSpec, String> =
            serde_json::from_value(to_json(&spec)).map_err(|e| format!("the world spec: {e}"));
        let result = spec.and_then(|s| self.session.generate_candidates(&s, count.max(0) as usize));
        error_string(result)
    }

    /// Every candidate: `index`, `state` (generating, ready, failed), `seed`, `label`,
    /// `error`, `summary`, `populations`.
    #[func]
    fn candidates(&self) -> Variant {
        to_variant(&serde_json::to_value(self.session.candidates()).unwrap_or(Value::Null))
    }

    /// A ready candidate's whole view (the `take_view` layout, every chunk), or an empty
    /// Dictionary while it is being founded.
    #[func]
    fn candidate_view(&self, index: i64) -> VarDictionary {
        usize::try_from(index)
            .ok()
            .and_then(|i| self.session.candidate_view(i))
            .map_or_else(VarDictionary::new, |v| view_dict(&v))
    }

    /// Run candidate `index` and drop the others. "" or the reason.
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
        match self.session.query(&name.to_string(), &to_json(&args)) {
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
        put(&mut d, "animals", animals);
        let p = &f.plants;
        let mut plants = VarDictionary::new();
        put(&mut plants, "ids", PackedInt64Array::from(p.ids.as_slice()));
        put(&mut plants, "species", PackedInt32Array::from(p.species.as_slice()));
        put(&mut plants, "site", PackedInt32Array::from(p.site.as_slice()));
        put(&mut plants, "growth", PackedFloat32Array::from(p.growth.as_slice()));
        put(&mut plants, "foliage", PackedFloat32Array::from(p.foliage.as_slice()));
        put(&mut d, "plants", plants);
        put(&mut d, "sky", to_variant(&serde_json::to_value(&f.sky).unwrap_or(Value::Null)));
    }
    d
}

/// A serde value as a Variant: objects become Dictionaries, arrays Arrays.
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
                d.set(&GString::from(k.as_str()).to_variant(), &to_variant(x));
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
