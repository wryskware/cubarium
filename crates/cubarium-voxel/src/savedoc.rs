//! **Configs as named-field documents in saves** (SAVE-EXTENSIBLE, Wrysk 2026-10-08: "we need
//! to do a better job about changing save formats, or making them extensible").
//!
//! A saved layer is a positional state (postcard, versioned by the layer's `SCHEMA`) and,
//! beside it, its config as a self-describing document with every field named. The config is
//! read back over the code defaults, the way the TOML loader reads a config file over them
//! ([`crate::registry::Registry::apply_table`]):
//!
//! - a knob the save lacks takes its default: the base's value at that path, so a lineage's
//!   or a species' own default, not a generic one;
//! - a knob the save has and this build does not know is dropped, and named once on stderr
//!   ([`note_ignored`]);
//! - so **adding a knob needs no schema bump**. Changing what a knob *means* still does.
//!
//! The overlay walks the two trees together: maps (structs) merge key by key, arrays of the
//! same length element by element, and anything else (a number, a string, an array of another
//! length, a `null`, an enum switched to another variant) is the saved value whole. A subtree
//! taken whole has no base to fill a missing field from; the struct's own `#[serde(default)]`
//! does it there.
//!
//! The document is CBOR (`ciborium`): self-describing like JSON, but it holds what a config
//! holds and JSON and TOML do not — an infinite float (a plant layer's open-ended
//! `height_m_max`), a `None` (TOML has no null, so it would come back as its default) and a
//! `u64` above `i64::MAX` (a drawn seed). It costs a few tens of kilobytes per save.

use anyhow::{Context, Result};
use ciborium::Value;
use serde::Serialize;
use serde::de::DeserializeOwned;

/// `cfg` as a named-field CBOR document.
pub fn to_doc<T: Serialize>(cfg: &T) -> Vec<u8> {
    let mut out = Vec::new();
    ciborium::into_writer(cfg, &mut out).expect("a config always serializes to CBOR");
    out
}

/// A config read from a saved document over `base` (the code defaults), and the paths of the
/// saved knobs this build does not know (dropped).
pub fn from_doc<T: Serialize + DeserializeOwned>(doc: &[u8], base: &T) -> Result<(T, Vec<String>)> {
    let saved: Value = ciborium::from_reader(doc).context("the saved config is not a CBOR document")?;
    let base = Value::serialized(base).context("serializing the default config")?;
    let mut ignored = Vec::new();
    let merged = overlay(base, saved, String::new(), &mut ignored);
    let cfg = merged
        .deserialized::<T>()
        .map_err(|e| anyhow::anyhow!("the saved config does not read as this build's: {e}"))?;
    Ok((cfg, ignored))
}

/// One field of a saved document read alone, when it is there: a number a layer needs to
/// choose its base by (a plant layer's cell size).
pub fn field_f64(doc: &[u8], key: &str) -> Option<f64> {
    let saved: Value = ciborium::from_reader(doc).ok()?;
    let Value::Map(m) = saved else { return None };
    m.into_iter().find_map(|(k, v)| match (k, v) {
        (Value::Text(k), Value::Float(x)) if k == key => Some(x),
        (Value::Text(k), Value::Integer(i)) if k == key => Some(i128::from(i) as f64),
        _ => None,
    })
}

/// One line on stderr naming the saved knobs this build dropped, if any.
pub fn note_ignored(layer: &str, ignored: &[String]) {
    if ignored.is_empty() {
        return;
    }
    const SHOWN: usize = 8;
    let mut names = ignored.iter().take(SHOWN).cloned().collect::<Vec<_>>().join(", ");
    if ignored.len() > SHOWN {
        names.push_str(&format!(", … ({} in all)", ignored.len()));
    }
    eprintln!("{layer} save: ignoring knob(s) this build does not know: {names}");
}

/// **Every knob a save could lack** (the test of "a knob the save lacks takes its default"):
/// for every struct in `cfg`'s tree, one of each shape, each field removed in turn from a
/// document of `cfg` and read back over `cfg`. A path listed did not come back as `cfg`'s value.
pub fn knobs_a_save_cannot_lack<T: Serialize + DeserializeOwned>(cfg: &T) -> Vec<String> {
    let tree = Value::serialized(cfg).expect("a config always serializes to CBOR");
    let mut nodes = Vec::new();
    collect_maps(&tree, &mut Vec::new(), String::new(), &mut nodes);
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for (steps, path, keys) in nodes {
        if !seen.insert(keys.clone()) {
            continue;
        }
        for k in &keys {
            let mut t = tree.clone();
            let Some(Value::Map(m)) = at_mut(&mut t, &steps) else { continue };
            m.retain(|(mk, _)| key_name(mk) != *k);
            let mut doc = Vec::new();
            ciborium::into_writer(&t, &mut doc).expect("a CBOR value always writes");
            let same = from_doc::<T>(&doc, cfg)
                .ok()
                .and_then(|(back, _)| Value::serialized(&back).ok())
                .is_some_and(|back| back == tree);
            if !same {
                out.push(if path.is_empty() { k.clone() } else { format!("{path}.{k}") });
            }
        }
    }
    out
}

/// A step into a tree: a map's key or an array's index.
#[derive(Clone)]
enum Step {
    Key(String),
    Index(usize),
}

fn collect_maps(v: &Value, steps: &mut Vec<Step>, path: String, out: &mut Vec<(Vec<Step>, String, Vec<String>)>) {
    match v {
        Value::Map(m) => {
            let keys: Vec<String> = m.iter().map(|(k, _)| key_name(k)).collect();
            out.push((steps.clone(), path.clone(), keys));
            for (k, v) in m {
                let name = key_name(k);
                steps.push(Step::Key(name.clone()));
                let at = if path.is_empty() { name } else { format!("{path}.{name}") };
                collect_maps(v, steps, at, out);
                steps.pop();
            }
        }
        Value::Array(a) => {
            for (i, v) in a.iter().enumerate() {
                steps.push(Step::Index(i));
                collect_maps(v, steps, format!("{path}[{i}]"), out);
                steps.pop();
            }
        }
        _ => {}
    }
}

fn at_mut<'a>(v: &'a mut Value, steps: &[Step]) -> Option<&'a mut Value> {
    let mut node = v;
    for s in steps {
        node = match (node, s) {
            (Value::Map(m), Step::Key(k)) => &mut m.iter_mut().find(|(mk, _)| key_name(mk) == *k)?.1,
            (Value::Array(a), Step::Index(i)) => a.get_mut(*i)?,
            _ => return None,
        };
    }
    Some(node)
}

fn key_name(k: &Value) -> String {
    match k {
        Value::Text(s) => s.clone(),
        other => format!("{other:?}"),
    }
}

fn overlay(base: Value, saved: Value, path: String, ignored: &mut Vec<String>) -> Value {
    overlay_in(base, saved, path, &[], ignored)
}

/// [`overlay`], with the elements of the array `base` sits in (`siblings`, empty outside an
/// array): a field whose base is `None` and whose saved value is a struct takes a sibling's
/// struct as its base (one lineage's `hunter` over another's), so its missing knobs get a real
/// default of the same type rather than none.
fn overlay_in(base: Value, saved: Value, path: String, siblings: &[Value], ignored: &mut Vec<String>) -> Value {
    match (base, saved) {
        (Value::Map(mut b), Value::Map(s)) => {
            // An externally tagged enum switched to another variant: `{"A": ..}` over
            // `{"B": ..}` is the saved variant whole, not a merge of two variants.
            if b.len() == 1 && s.len() == 1 && b[0].0 != s[0].0 {
                return Value::Map(s);
            }
            // Serde aliases alone cannot survive the unknown-key filter below. Resolve
            // the renamed light channel only where the base has its current field;
            // an explicit current key takes precedence over an old spelling.
            let aliases = [("uv", "nir"), ("lure_uv", "lure_nir"), ("uv_innate", "nir_innate")];
            let current_keys: Vec<Value> = s.iter().map(|(k, _)| k.clone()).collect();
            for (mut k, sv) in s {
                if let Value::Text(name) = &k {
                    if let Some((_, new)) = aliases.iter().find(|(old, _)| name == old) {
                        let current = Value::Text((*new).into());
                        if b.iter().any(|(bk, _)| *bk == current) {
                            if current_keys.contains(&current) {
                                continue;
                            }
                            k = current;
                        }
                    }
                }
                let at = if path.is_empty() {
                    key_name(&k)
                } else {
                    format!("{path}.{}", key_name(&k))
                };
                match b.iter_mut().find(|(bk, _)| *bk == k) {
                    Some((_, bv)) => {
                        let mut old = std::mem::replace(bv, Value::Null);
                        if old.is_null() && matches!(sv, Value::Map(_)) {
                            if let Some(borrowed) = sibling_field(siblings, &k) {
                                old = borrowed;
                            }
                        }
                        *bv = overlay_in(old, sv, at, &[], ignored);
                    }
                    None => ignored.push(at),
                }
            }
            Value::Map(b)
        }
        // Element by element; an array of another length (a species' profile bands) reads
        // each saved element over the base element at its index, or the last one.
        (Value::Array(b), Value::Array(s)) if !b.is_empty() => {
            let last = b.len() - 1;
            Value::Array(
                s.into_iter()
                    .enumerate()
                    .map(|(i, sv)| overlay_in(b[i.min(last)].clone(), sv, format!("{path}[{i}]"), &b, ignored))
                    .collect(),
            )
        }
        (_, saved) => saved,
    }
}

/// The first non-null value of field `key` among the struct elements `siblings`.
fn sibling_field(siblings: &[Value], key: &Value) -> Option<Value> {
    siblings.iter().find_map(|e| match e {
        Value::Map(m) => m.iter().find(|(k, v)| k == key && !v.is_null()).map(|(_, v)| v.clone()),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    /// A config as it stood when a save was written: no `added`, a `dropped` knob since
    /// removed, an enum, a `None`, an infinity and a seed above `i64::MAX`.
    #[derive(Serialize)]
    struct Old {
        rate: f64,
        dropped: u32,
        inner: OldInner,
        lineages: [OldInner; 2],
        mode: Mode,
        maybe: Option<f64>,
        cap: f64,
        seed: u64,
    }

    #[derive(Serialize)]
    struct OldInner {
        a: f64,
    }

    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default, deny_unknown_fields)]
    struct New {
        rate: f64,
        added: f64,
        inner: Inner,
        lineages: [Inner; 2],
        mode: Mode,
        maybe: Option<f64>,
        cap: f64,
        seed: u64,
    }

    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default, deny_unknown_fields)]
    struct Inner {
        a: f64,
        b: f64,
    }

    impl Default for Inner {
        fn default() -> Inner {
            Inner { a: 1.0, b: 2.0 }
        }
    }

    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    enum Mode {
        Off,
        Ring { width: f64 },
    }

    impl Default for New {
        fn default() -> New {
            New {
                rate: 0.5,
                added: 7.0,
                inner: Inner::default(),
                // Each element's own default differs from `Inner::default()`.
                lineages: [Inner { a: 10.0, b: 20.0 }, Inner { a: 30.0, b: 40.0 }],
                mode: Mode::Off,
                maybe: Some(3.0),
                cap: 1.0,
                seed: 1,
            }
        }
    }

    fn old() -> Old {
        Old {
            rate: 0.25,
            dropped: 9,
            inner: OldInner { a: -1.0 },
            lineages: [OldInner { a: 11.0 }, OldInner { a: 31.0 }],
            mode: Mode::Ring { width: 2.5 },
            maybe: None,
            cap: f64::INFINITY,
            seed: u64::MAX - 3,
        }
    }

    #[test]
    fn a_knob_the_save_lacks_takes_the_base_value_at_its_path() {
        let base = New::default();
        let (cfg, _) = from_doc::<New>(&to_doc(&old()), &base).expect("an older config reads");
        assert_eq!(cfg.added, base.added);
        assert_eq!(cfg.inner.b, base.inner.b);
        // Per element: each lineage's own default, not the struct's generic one.
        assert_eq!(cfg.lineages[0].b, base.lineages[0].b);
        assert_eq!(cfg.lineages[1].b, base.lineages[1].b);
        // And every knob the save has is the save's.
        assert_eq!(cfg.rate, 0.25);
        assert_eq!(cfg.inner.a, -1.0);
        assert_eq!(cfg.lineages[1].a, 31.0);
        assert_eq!(cfg.mode, Mode::Ring { width: 2.5 });
        assert_eq!(cfg.maybe, None, "a saved None stays None over a Some default");
        assert_eq!(cfg.cap, f64::INFINITY);
        assert_eq!(cfg.seed, u64::MAX - 3);
    }

    #[test]
    fn a_knob_this_build_does_not_know_is_dropped_and_named() {
        let (_, ignored) = from_doc::<New>(&to_doc(&old()), &New::default()).expect("reads");
        assert_eq!(ignored, vec!["dropped".to_string()]);
    }

    /// A lineage's `None` struct saved as `Some` borrows a sibling lineage's struct as its
    /// base, and an array of another length reads each element over the nearest base element:
    /// neither needs a `Default` on the struct.
    #[test]
    fn a_struct_the_defaults_lack_borrows_a_sibling_as_its_base() {
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        struct Hunt {
            x: f64,
            y: f64,
        }
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        struct Lineage {
            hunt: Option<Hunt>,
            bands: Vec<Hunt>,
        }
        #[derive(Serialize)]
        struct OldHunt {
            x: f64,
        }
        #[derive(Serialize)]
        struct OldLineage {
            hunt: Option<OldHunt>,
            bands: Vec<OldHunt>,
        }
        let base = vec![
            Lineage { hunt: Some(Hunt { x: 1.0, y: 2.0 }), bands: vec![Hunt { x: 0.0, y: 9.0 }] },
            Lineage { hunt: None, bands: vec![Hunt { x: 0.0, y: 8.0 }] },
        ];
        let saved = vec![
            OldLineage { hunt: None, bands: vec![] },
            OldLineage {
                hunt: Some(OldHunt { x: 5.0 }),
                bands: vec![OldHunt { x: 1.0 }, OldHunt { x: 2.0 }],
            },
        ];
        let (back, ignored) = from_doc::<Vec<Lineage>>(&to_doc(&saved), &base).expect("reads");
        assert!(ignored.is_empty());
        assert_eq!(back[0].hunt, None);
        assert_eq!(back[1].hunt, Some(Hunt { x: 5.0, y: 2.0 }));
        assert_eq!(back[1].bands, vec![Hunt { x: 1.0, y: 8.0 }, Hunt { x: 2.0, y: 8.0 }]);
    }

    #[test]
    fn every_knob_of_a_test_config_can_be_missing() {
        assert!(knobs_a_save_cannot_lack(&New::default()).is_empty());
    }

    #[test]
    fn a_round_trip_keeps_every_value() {
        let cfg = New {
            rate: 0.1 + 0.2,
            added: -0.0,
            inner: Inner { a: 1e-300, b: f64::NEG_INFINITY },
            lineages: [Inner { a: 5.0, b: 6.0 }, Inner { a: 7.0, b: 8.0 }],
            mode: Mode::Ring { width: std::f64::consts::PI },
            maybe: None,
            cap: f64::INFINITY,
            seed: u64::MAX,
        };
        let (back, ignored) = from_doc::<New>(&to_doc(&cfg), &New::default()).expect("reads");
        assert!(ignored.is_empty());
        assert_eq!(back, cfg);
        assert!(back.added.is_sign_negative());
    }
}
