//! The registry's generic spec (package R1, `design/ecology-parameter-registry-2026-09-28.md`),
//! shared by every crate that owns a table: each crate's `tests/registry.rs` includes this
//! file by path and runs it over its own default config.
//!
//! The struct's own values are read through **serde_json**, not through the registry's
//! TOML path, so "get equals the struct value" is checked against an independent
//! serialization.
#![allow(dead_code)]

use cubarium_voxel::registry::{Range, Registry};
use serde::Serialize;
use serde::de::DeserializeOwned;

/// Every numeric leaf of `cfg` as serde_json writes it: `root.a.0.b` (indices, not names).
pub fn json_leaves<T: Serialize>(root: &str, cfg: &T) -> Vec<(String, f64)> {
    fn walk(path: String, v: &serde_json::Value, out: &mut Vec<(String, f64)>) {
        match v {
            serde_json::Value::Number(n) => out.push((path, n.as_f64().expect("a number"))),
            serde_json::Value::Array(a) => {
                for (i, x) in a.iter().enumerate() {
                    walk(format!("{path}.{i}"), x, out);
                }
            }
            serde_json::Value::Object(o) => {
                for (k, x) in o {
                    walk(format!("{path}.{k}"), x, out);
                }
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    walk(root.to_string(), &serde_json::to_value(cfg).expect("json"), &mut out);
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// The numeric leaves outside the registry's opaque subtrees.
fn registered_leaves<T: Serialize>(reg: &Registry, cfg: &T) -> Vec<(String, f64)> {
    json_leaves(reg.root(), cfg)
        .into_iter()
        .filter(|(p, _)| !reg.is_opaque(p))
        .collect()
}

/// **Test 1, coverage**: every numeric leaf of every config in `cfgs` (recursively, into
/// the per-lineage and per-species tables) has a row, and every row that is not marked
/// optional matches a leaf of at least one of them.
pub fn coverage<T: Serialize>(reg: &Registry, cfgs: &[&T]) {
    let mut failures = Vec::new();
    let mut leaves = Vec::new();
    for cfg in cfgs {
        leaves.extend(registered_leaves(reg, *cfg));
    }
    for (path, _) in &leaves {
        if reg.row_for(path).is_none() {
            failures.push(format!("no row for `{}`", reg.display(path)));
        }
    }
    for row in reg.rows() {
        let hit = leaves
            .iter()
            .any(|(p, _)| row.matches(p) || row.matches(&reg.display(p)));
        if !hit && !row.optional {
            failures.push(format!("row `{}` matches no field", row.path));
        }
        assert!(
            row.path.starts_with(&format!("{}.", reg.root())),
            "row `{}` is not under `{}`",
            row.path,
            reg.root()
        );
    }
    failures.sort();
    failures.dedup();
    assert!(
        failures.is_empty(),
        "{} coverage failures:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// A value inside `range`, different from `v`, whole if `whole`.
fn another_inside(range: &Range, v: f64, whole: bool) -> Option<f64> {
    let mut tries = vec![v + 1.0, v - 1.0, v * 0.5, v * 1.5, v * 0.99, v * 1.01];
    if range.lo.is_finite() && range.hi.is_finite() {
        tries.push((range.lo + range.hi) / 2.0);
        tries.push(range.lo + (range.hi - range.lo) * 0.25);
    }
    if range.lo.is_finite() {
        tries.push(range.lo + 1.0);
        tries.push(range.lo + 1e-3);
    }
    tries
        .into_iter()
        .map(|x| if whole { x.round() } else { x })
        .find(|x| *x != v && range.contains(*x))
}

/// **Test 2, round trip**: for every leaf, `get` on the defaults equals the struct's value
/// (read through serde_json); `set` to another valid value, then `get`, returns it, and
/// no other leaf moved.
pub fn round_trip<T: Serialize + DeserializeOwned + Clone>(reg: &Registry, cfg: &T) {
    let before = registered_leaves(reg, cfg);
    let mut failures = Vec::new();
    for (path, v) in &before {
        let shown = reg.display(path);
        match reg.get(cfg, &shown) {
            Ok(got) => {
                let got = got.as_float().or(got.as_integer().map(|i| i as f64));
                if got != Some(*v) {
                    failures.push(format!("get `{shown}` = {got:?}, the struct holds {v}"));
                }
            }
            Err(e) => {
                failures.push(format!("get `{shown}`: {e}"));
                continue;
            }
        }
        let Some(row) = reg.row_for(path) else {
            failures.push(format!("no row for `{shown}`"));
            continue;
        };
        let whole = matches!(reg.get(cfg, &shown), Ok(toml::Value::Integer(_)));
        let Some(next) = another_inside(&row.range, *v, whole) else {
            failures.push(format!("`{shown}`: no other value inside {}", row.range));
            continue;
        };
        let value = if whole {
            toml::Value::Integer(next as i64)
        } else {
            toml::Value::Float(next)
        };
        let mut moved = cfg.clone();
        if let Err(e) = reg.set(&mut moved, &shown, value) {
            failures.push(format!("set `{shown}` = {next}: {e}"));
            continue;
        }
        let got = reg.get(&moved, &shown).expect("a path just set");
        let got = got.as_float().or(got.as_integer().map(|i| i as f64));
        if got != Some(next) {
            failures.push(format!("set `{shown}` = {next}, then get = {got:?}"));
        }
        let after = registered_leaves(reg, &moved);
        let changed: Vec<&String> = before
            .iter()
            .zip(&after)
            .filter(|(a, b)| a != b)
            .map(|(a, _)| &a.0)
            .collect();
        if before.len() != after.len() || changed != vec![path] {
            failures.push(format!("set `{shown}` changed {changed:?}"));
        }
    }
    assert!(
        failures.is_empty(),
        "{} round-trip failures:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// A value just outside `range` on each finite side, NaN, and infinity unless the range
/// closes on it.
fn outside(range: &Range, whole: bool) -> Vec<f64> {
    let mut out = Vec::new();
    if range.lo.is_finite() {
        out.push(if range.lo_open {
            range.lo
        } else if whole {
            range.lo - 1.0
        } else {
            range.lo - range.lo.abs().max(1.0) * 1e-3
        });
    }
    if range.hi.is_finite() {
        out.push(if range.hi_open {
            range.hi
        } else if whole {
            range.hi + 1.0
        } else {
            range.hi + range.hi.abs().max(1.0) * 1e-3
        });
    }
    if !whole {
        out.push(f64::NAN);
        if !range.contains(f64::INFINITY) {
            out.push(f64::INFINITY);
        }
    }
    out
}

/// **Test 5, range**: for every leaf, a value outside its row's range is refused by
/// `set`, and the message names the path and the range.
pub fn range_refusals<T: Serialize + DeserializeOwned + Clone>(reg: &Registry, cfg: &T) {
    let mut failures = Vec::new();
    for (path, _) in registered_leaves(reg, cfg) {
        let shown = reg.display(&path);
        let Some(row) = reg.row_for(&path) else {
            failures.push(format!("no row for `{shown}`"));
            continue;
        };
        let whole = matches!(reg.get(cfg, &shown), Ok(toml::Value::Integer(_)));
        for bad in outside(&row.range, whole) {
            let value = if whole {
                toml::Value::Integer(bad as i64)
            } else {
                toml::Value::Float(bad)
            };
            let mut moved = cfg.clone();
            match reg.set(&mut moved, &shown, value) {
                Ok(()) => failures.push(format!("`{shown}` = {bad} accepted outside {}", row.range)),
                Err(e) => {
                    let range = row.range.to_string();
                    if !(e.contains(&shown) && e.contains(&range)) {
                        failures.push(format!(
                            "`{shown}` = {bad}: the refusal does not name the path and {range}: {e}"
                        ));
                    }
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} range failures:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// An unknown path is refused by `set`, and the message names it.
pub fn unknown_paths_refused<T: Serialize + DeserializeOwned + Clone>(reg: &Registry, cfg: &T) {
    let root = reg.root();
    for path in [format!("{root}.no_such_field"), format!("{root}.no.such.path")] {
        let mut moved = cfg.clone();
        let e = reg
            .set(&mut moved, &path, toml::Value::Float(1.0))
            .expect_err("an unknown path was accepted");
        assert!(e.contains(&path), "{e}");
    }
}
