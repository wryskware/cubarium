//! **The parameter registry** (`design/ecology-parameter-registry-2026-09-28.md` §1–§2,
//! package R1): every number a config struct carries, declared once as a [`Row`] keyed
//! by its path, and generic code over those rows instead of hand-written code per field.
//!
//! - **Paths** are the serialized config's own keys joined by dots under a root —
//!   `world.climate.dry_gap_min_s`, `flora.springturf.hop`,
//!   `fauna.founders.frondgrazer.core.maintenance_per_s`. An array element is its index,
//!   or a name where the registry gives the array's elements names (the founders by
//!   lineage, the yields by food). A row's path may use `*` for any one segment, so one
//!   row covers a field wherever its struct is mounted.
//! - **Get and set** go through the config's serialized TOML value: serialize, read or
//!   replace the leaf, deserialize. Config time only; nothing here is on a tick.
//! - **Validation** is each row's [`Range`] over every numeric leaf, plus the few
//!   cross-field rules each crate keeps by hand.
//! - **Rescale** to a voxel size multiplies each leaf by its row's [`Scale`] tag.
//! - **The dump** lists every leaf with its value, unit and range.
//!
//! Each crate owns its table: `cubarium_voxel::registry::world()`,
//! `cubarium_voxel_flora::registry()`, `cubarium_voxel_fauna::registry()`.

use std::fmt;

use serde::Serialize;
use serde::de::DeserializeOwned;
use toml::Value;

/// How a field follows the world's voxel size ([`Registry::rescale`]). The presets are
/// authored on the reference 0.25 m voxel; `ratio` below is `0.25 / voxel_m`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scale {
    /// Not stored in voxel units: rates, fractions, metres, seconds. Untouched.
    None,
    /// A **distance counted in voxels** (a root box, a hop): × `ratio`. A whole-number
    /// field is rounded, and a nonzero one never falls below 1.
    Length,
    /// An **amount per voxel face** (the latticevine's per-face mass and rates):
    /// × `1 / ratio²`, the face's area against the reference face's.
    Area,
    /// An **amount per voxel**: × `1 / ratio³`.
    Volume,
}

impl Scale {
    pub fn name(self) -> &'static str {
        match self {
            Scale::None => "none",
            Scale::Length => "length",
            Scale::Area => "area",
            Scale::Volume => "volume",
        }
    }
}

/// A row's valid interval. Every value must also be finite.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Range {
    pub lo: f64,
    pub hi: f64,
    /// Whether `lo` itself is excluded.
    pub lo_open: bool,
    /// Whether `hi` itself is excluded.
    pub hi_open: bool,
}

impl Range {
    /// `[lo, hi]`.
    pub const fn closed(lo: f64, hi: f64) -> Range {
        Range { lo, hi, lo_open: false, hi_open: false }
    }
    /// `[0, ∞)`: finite and nonnegative, the common floor.
    pub const fn nonneg() -> Range {
        Range { lo: 0.0, hi: f64::INFINITY, lo_open: false, hi_open: true }
    }
    /// `(0, ∞)`: finite and positive.
    pub const fn positive() -> Range {
        Range { lo: 0.0, hi: f64::INFINITY, lo_open: true, hi_open: true }
    }
    /// `[0, 1]`.
    pub const fn fraction() -> Range {
        Range::closed(0.0, 1.0)
    }
    /// `(-∞, ∞)`: any finite number.
    pub const fn any() -> Range {
        Range { lo: f64::NEG_INFINITY, hi: f64::INFINITY, lo_open: true, hi_open: true }
    }
    /// `[lo, ∞)`.
    pub const fn at_least(lo: f64) -> Range {
        Range { lo, hi: f64::INFINITY, lo_open: false, hi_open: true }
    }
    /// The same interval with `lo` excluded.
    pub const fn open_lo(self) -> Range {
        Range { lo_open: true, ..self }
    }
    /// The same interval with `hi` excluded.
    pub const fn open_hi(self) -> Range {
        Range { hi_open: true, ..self }
    }

    /// Whether `v` is finite and inside.
    pub fn contains(&self, v: f64) -> bool {
        v.is_finite()
            && (if self.lo_open { v > self.lo } else { v >= self.lo })
            && (if self.hi_open { v < self.hi } else { v <= self.hi })
    }
}

impl fmt::Display for Range {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let end = |v: f64| {
            if v == f64::INFINITY {
                "inf".to_string()
            } else if v == f64::NEG_INFINITY {
                "-inf".to_string()
            } else {
                format!("{v}")
            }
        };
        write!(
            f,
            "{}{}, {}{}",
            if self.lo_open { '(' } else { '[' },
            end(self.lo),
            end(self.hi),
            if self.hi_open { ')' } else { ']' }
        )
    }
}

/// One declared parameter.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    /// The full path, root first; `*` stands for any one segment.
    pub path: String,
    /// Its unit, as `parameters.tsv` gives it where it has a row.
    pub unit: &'static str,
    /// The values a config may hold: refused outside it ([`Registry::check`]).
    pub range: Range,
    /// The interval a search may move it over (the audit's `evolve` column), where one
    /// was named. Not a validity bound: today's value may sit outside it.
    pub search: Option<(f64, f64)>,
    /// Whether a search moves it on a log scale.
    pub log: bool,
    /// How it follows the voxel size.
    pub scale: Scale,
    /// Whether the audit marks it for evolution.
    pub evolve: bool,
    /// Whether moving it moves a trained controller's digest (§6).
    pub digest: bool,
    /// Whether the default config may lack it (an `Option` that is `None`, a variant not
    /// taken): such a row matches no default field without being stale.
    pub optional: bool,
    /// The audit's grade, `A`–`D`, or `-`.
    pub grade: &'static str,
    /// Where the value came from: the audit's provenance, or `placeholder`.
    pub provenance: &'static str,
    /// One line on what it is.
    pub what: &'static str,
}

impl Row {
    /// A row with no search interval, no scale, not evolved, not in a digest, grade `-`,
    /// provenance `placeholder`.
    pub fn new(path: impl Into<String>, unit: &'static str, range: Range, what: &'static str) -> Row {
        Row {
            path: path.into(),
            unit,
            range,
            search: None,
            log: false,
            scale: Scale::None,
            evolve: false,
            digest: false,
            optional: false,
            grade: "-",
            provenance: "placeholder",
            what,
        }
    }
    /// Marked for evolution over `[lo, hi]`.
    pub fn search(mut self, lo: f64, hi: f64) -> Row {
        self.search = Some((lo, hi));
        self.evolve = true;
        self
    }
    /// Marked for evolution, interval left to the search file.
    pub fn evolve(mut self) -> Row {
        self.evolve = true;
        self
    }
    pub fn log(mut self) -> Row {
        self.log = true;
        self
    }
    pub fn scale(mut self, scale: Scale) -> Row {
        self.scale = scale;
        self
    }
    pub fn digest(mut self) -> Row {
        self.digest = true;
        self
    }
    pub fn optional(mut self) -> Row {
        self.optional = true;
        self
    }
    pub fn grade(mut self, grade: &'static str) -> Row {
        self.grade = grade;
        self
    }
    pub fn prov(mut self, provenance: &'static str) -> Row {
        self.provenance = provenance;
        self
    }
    /// The same row under `prefix` (`"fauna.founders.*.core"` + `"maintenance_per_s"`).
    pub fn under(mut self, prefix: &str) -> Row {
        self.path = format!("{prefix}.{}", self.path);
        self
    }

    /// Whether a concrete path (root first, elements by index or name) is this row's.
    pub fn matches(&self, path: &str) -> bool {
        let mut a = self.path.split('.');
        let mut b = path.split('.');
        loop {
            match (a.next(), b.next()) {
                (None, None) => return true,
                (Some(p), Some(s)) if p == "*" || p == s => continue,
                _ => return false,
            }
        }
    }

    fn wildcards(&self) -> usize {
        self.path.split('.').filter(|s| *s == "*").count()
    }
}

/// Names for an array's elements: `pattern` (root first, `*` allowed) is the array's
/// path, and element `i` may be written `names[i]`.
#[derive(Clone, Debug, PartialEq)]
pub struct Alias {
    pub pattern: String,
    pub names: Vec<&'static str>,
}

/// One leaf of a [`Registry::dump`].
#[derive(Clone, Debug, PartialEq)]
pub struct DumpLine {
    /// The concrete path, elements by name where they have one.
    pub path: String,
    /// The value as TOML writes it.
    pub value: String,
    /// The row it falls under.
    pub row: Row,
}

/// One crate's table.
#[derive(Clone, Debug)]
pub struct Registry {
    root: &'static str,
    rows: Vec<Row>,
    aliases: Vec<Alias>,
    opaque: Vec<String>,
}

impl Registry {
    /// A table under `root`. `opaque` lists subtrees whose numbers are not registered
    /// here (the terrain recipe): they are set and replaced whole and validated by their
    /// own struct, and no row is asked of them.
    pub fn new(root: &'static str, rows: Vec<Row>, aliases: Vec<Alias>, opaque: Vec<String>) -> Registry {
        Registry { root, rows, aliases, opaque }
    }

    pub fn root(&self) -> &'static str {
        self.root
    }

    pub fn rows(&self) -> &[Row] {
        &self.rows
    }

    /// The row a concrete path falls under: the most specific match (fewest `*`).
    pub fn row_for(&self, path: &str) -> Option<&Row> {
        let path = self.display(path);
        self.rows
            .iter()
            .filter(|r| r.matches(&path) || r.matches(&self.canonical(&path)))
            .min_by_key(|r| r.wildcards())
    }

    /// Whether `path` lies in an opaque subtree.
    pub fn is_opaque(&self, path: &str) -> bool {
        let path = self.canonical(path);
        self.opaque
            .iter()
            .any(|o| path == *o || path.starts_with(&format!("{o}.")))
    }

    /// `path` with every named element replaced by its index.
    pub fn canonical(&self, path: &str) -> String {
        self.rename(path, |alias, seg| {
            alias.names.iter().position(|n| *n == seg).map(|i| i.to_string())
        })
    }

    /// `path` with every indexed element that has a name written by it.
    pub fn display(&self, path: &str) -> String {
        self.rename(path, |alias, seg| {
            seg.parse::<usize>()
                .ok()
                .and_then(|i| alias.names.get(i))
                .map(|n| n.to_string())
        })
    }

    fn rename(&self, path: &str, f: impl Fn(&Alias, &str) -> Option<String>) -> String {
        let segs: Vec<&str> = path.split('.').collect();
        let mut out: Vec<String> = Vec::with_capacity(segs.len());
        for seg in segs {
            let parent = out.join(".");
            let renamed = self
                .aliases
                .iter()
                .filter(|a| {
                    let row = Row::new(a.pattern.clone(), "", Range::any(), "");
                    row.matches(&parent)
                })
                .find_map(|a| f(a, seg));
            out.push(renamed.unwrap_or_else(|| seg.to_string()));
        }
        out.join(".")
    }

    /// Every numeric leaf of a serialized config (the value of `[root]`), root first,
    /// elements by name; opaque subtrees left out.
    pub fn numeric_leaves(&self, value: &Value) -> Vec<(String, f64)> {
        let mut out = Vec::new();
        self.walk(self.root.to_string(), value, &mut out);
        out
    }

    fn walk(&self, path: String, value: &Value, out: &mut Vec<(String, f64)>) {
        if self.is_opaque(&path) {
            return;
        }
        match value {
            Value::Integer(i) => out.push((self.display(&path), *i as f64)),
            Value::Float(x) => out.push((self.display(&path), *x)),
            Value::Array(items) => {
                for (i, v) in items.iter().enumerate() {
                    self.walk(format!("{path}.{i}"), v, out);
                }
            }
            Value::Table(t) => {
                for (k, v) in t {
                    self.walk(format!("{path}.{k}"), v, out);
                }
            }
            _ => {}
        }
    }

    /// Every numeric leaf of `value` has a row and lies in its range.
    pub fn check_value(&self, value: &Value) -> Result<(), String> {
        for (path, v) in self.numeric_leaves(value) {
            let row = self
                .row_for(&path)
                .ok_or_else(|| format!("`{path}` has no row in the parameter registry"))?;
            if !row.range.contains(v) {
                return Err(out_of_range(&path, v, row));
            }
        }
        Ok(())
    }

    /// [`Registry::check_value`] over `cfg`'s serialized value.
    pub fn check<T: Serialize>(&self, cfg: &T) -> Result<(), String> {
        self.check_value(&to_value(cfg)?)
    }

    /// The value at `path` (root first).
    pub fn get<T: Serialize>(&self, cfg: &T, path: &str) -> Result<Value, String> {
        let value = to_value(cfg)?;
        let rel = self.relative(path)?;
        lookup(&value, &rel)
            .cloned()
            .ok_or_else(|| unknown(path))
    }

    /// Replace the leaf (or subtree) at `path` with `value` and deserialize; every
    /// numeric leaf it put in place is range-checked. An unknown path is an error naming
    /// it.
    pub fn set<T: Serialize + DeserializeOwned>(
        &self,
        cfg: &mut T,
        path: &str,
        value: Value,
    ) -> Result<(), String> {
        self.apply(cfg, &[(path.to_string(), value)])
    }

    /// [`Registry::set`] for several paths, in order, in one serialize/deserialize pass.
    pub fn apply<T: Serialize + DeserializeOwned>(
        &self,
        cfg: &mut T,
        sets: &[(String, Value)],
    ) -> Result<(), String> {
        if sets.is_empty() {
            return Ok(());
        }
        let mut tree = to_value(cfg)?;
        for (path, value) in sets {
            self.put(&mut tree, path, value.clone())?;
        }
        let next: T = from_value(tree.clone())?;
        // Range-check what was set, under the paths it was set at.
        for (path, _) in sets {
            let rel = self.relative(path)?;
            if let Some(v) = lookup(&tree, &rel) {
                let mut leaves = Vec::new();
                self.walk(self.canonical(path), v, &mut leaves);
                for (leaf, x) in leaves {
                    let row = self.row_for(&leaf).ok_or_else(|| {
                        format!("`{leaf}` has no row in the parameter registry")
                    })?;
                    if !row.range.contains(x) {
                        return Err(out_of_range(&leaf, x, row));
                    }
                }
            }
        }
        *cfg = next;
        Ok(())
    }

    /// Apply a TOML table written under `[root]` — keys are path segments, a nested table
    /// goes one segment deeper, anything else (a number, a string, an array, an opaque
    /// subtree) is the value set at that path.
    pub fn apply_table<T: Serialize + DeserializeOwned>(
        &self,
        cfg: &mut T,
        table: &toml::Table,
    ) -> Result<(), String> {
        let sets = self.flatten(table);
        self.apply(cfg, &sets)
    }

    /// A `[root]` table as `(path, value)` pairs ([`Registry::apply_table`]).
    pub fn flatten(&self, table: &toml::Table) -> Vec<(String, Value)> {
        let mut out = Vec::new();
        self.flatten_into(self.root.to_string(), table, &mut out);
        out
    }

    fn flatten_into(&self, prefix: String, table: &toml::Table, out: &mut Vec<(String, Value)>) {
        for (k, v) in table {
            let path = format!("{prefix}.{k}");
            match v {
                Value::Table(t) if !self.is_opaque(&path) => self.flatten_into(path, t, out),
                other => out.push((path, other.clone())),
            }
        }
    }

    /// Multiply every leaf whose row has a [`Scale`] tag by that tag's factor for
    /// `ratio` (`0.25 / voxel_m`).
    pub fn rescale<T: Serialize + DeserializeOwned>(&self, cfg: &mut T, ratio: f64) -> Result<(), String> {
        let mut tree = to_value(cfg)?;
        let leaves = self.numeric_leaves(&tree);
        for (path, _) in leaves {
            let Some(row) = self.row_for(&path) else { continue };
            let rel = self.relative(&path)?;
            let Some(leaf) = lookup_mut(&mut tree, &rel) else { continue };
            match (row.scale, &*leaf) {
                (Scale::None, _) => {}
                (Scale::Length, Value::Integer(i)) => {
                    *leaf = Value::Integer(scaled_voxel_distance(*i, ratio));
                }
                (Scale::Length, Value::Float(x)) => *leaf = Value::Float(x * ratio),
                (Scale::Area, Value::Float(x)) => *leaf = Value::Float(x * (1.0 / (ratio * ratio))),
                (Scale::Volume, Value::Float(x)) => {
                    *leaf = Value::Float(x * (1.0 / (ratio * ratio * ratio)))
                }
                (scale, v) => {
                    return Err(format!(
                        "`{path}` is tagged {} but holds {v}, which that tag cannot scale",
                        scale.name()
                    ));
                }
            }
        }
        *cfg = from_value(tree)?;
        Ok(())
    }

    /// Every numeric leaf of `cfg`, with its value and row, in serialized order.
    pub fn dump<T: Serialize>(&self, cfg: &T) -> Result<Vec<DumpLine>, String> {
        let tree = to_value(cfg)?;
        let mut out = Vec::new();
        for (path, _) in self.numeric_leaves(&tree) {
            let row = self
                .row_for(&path)
                .ok_or_else(|| format!("`{path}` has no row in the parameter registry"))?
                .clone();
            let rel = self.relative(&path)?;
            let value = lookup(&tree, &rel).map(|v| v.to_string()).unwrap_or_default();
            out.push(DumpLine { path, value, row });
        }
        Ok(out)
    }

    /// `path` without the root, elements by index.
    fn relative(&self, path: &str) -> Result<Vec<String>, String> {
        let canon = self.canonical(path);
        let mut segs = canon.split('.');
        if segs.next() != Some(self.root) {
            return Err(format!("`{path}` is not under `{}`", self.root));
        }
        Ok(segs.map(str::to_string).collect())
    }

    /// Put `value` at `path` in `tree`: the path must exist, or end in a key an optional
    /// row (or an opaque subtree's existing parent) allows.
    fn put(&self, tree: &mut Value, path: &str, value: Value) -> Result<(), String> {
        let rel = self.relative(path)?;
        let Some((last, parents)) = rel.split_last() else {
            return Err(unknown(path));
        };
        let mut node = tree;
        for seg in parents {
            node = match node {
                Value::Table(t) => t.get_mut(seg.as_str()),
                Value::Array(a) => seg.parse::<usize>().ok().and_then(|i| a.get_mut(i)),
                _ => None,
            }
            .ok_or_else(|| unknown(path))?;
        }
        match node {
            Value::Table(t) => {
                let new = match t.get(last.as_str()) {
                    Some(old) => coerce(path, old, value)?,
                    None => {
                        let allowed = self.is_opaque(path)
                            || self
                                .row_for(path)
                                .is_some_and(|r| r.optional);
                        if !allowed {
                            return Err(unknown(path));
                        }
                        value
                    }
                };
                t.insert(last.clone(), new);
            }
            Value::Array(a) => {
                let i: usize = last.parse().map_err(|_| unknown(path))?;
                let old = a.get(i).ok_or_else(|| unknown(path))?;
                let new = coerce(path, old, value)?;
                a[i] = new;
            }
            _ => return Err(unknown(path)),
        }
        Ok(())
    }
}

/// `path=value`, the value read as TOML (a bare word is a string).
pub fn parse_set(s: &str) -> Result<(String, Value), String> {
    let (path, raw) = s
        .split_once('=')
        .ok_or_else(|| format!("`{s}` is not path=value"))?;
    let path = path.trim();
    let raw = raw.trim();
    if path.is_empty() {
        return Err(format!("`{s}` names no path"));
    }
    let value = format!("v = {raw}")
        .parse::<toml::Table>()
        .ok()
        .and_then(|mut t| t.remove("v"))
        .unwrap_or_else(|| Value::String(raw.to_string()));
    Ok((path.to_string(), value))
}

/// `cfg` as a TOML value.
pub fn to_value<T: Serialize>(cfg: &T) -> Result<Value, String> {
    Value::try_from(cfg).map_err(|e| format!("cannot serialize the config: {e}"))
}

/// A TOML value as a config.
pub fn from_value<T: DeserializeOwned>(value: Value) -> Result<T, String> {
    value.try_into().map_err(|e: toml::de::Error| e.to_string())
}

fn lookup<'a>(tree: &'a Value, rel: &[String]) -> Option<&'a Value> {
    let mut node = tree;
    for seg in rel {
        node = match node {
            Value::Table(t) => t.get(seg.as_str())?,
            Value::Array(a) => a.get(seg.parse::<usize>().ok()?)?,
            _ => return None,
        };
    }
    Some(node)
}

fn lookup_mut<'a>(tree: &'a mut Value, rel: &[String]) -> Option<&'a mut Value> {
    let mut node = tree;
    for seg in rel {
        node = match node {
            Value::Table(t) => t.get_mut(seg.as_str())?,
            Value::Array(a) => a.get_mut(seg.parse::<usize>().ok()?)?,
            _ => return None,
        };
    }
    Some(node)
}

/// A float field set from an integer is a float; an integer field takes only a whole
/// number.
fn coerce(path: &str, old: &Value, new: Value) -> Result<Value, String> {
    Ok(match (old, new) {
        (Value::Float(_), Value::Integer(i)) => Value::Float(i as f64),
        (Value::Integer(_), Value::Float(x)) => {
            if x.is_finite() && x.fract() == 0.0 && x.abs() < 9.0e15 {
                Value::Integer(x as i64)
            } else {
                return Err(format!("`{path}` takes a whole number, not {x}"));
            }
        }
        (_, new) => new,
    })
}

fn unknown(path: &str) -> String {
    format!("unknown parameter `{path}`: no such field in the config")
}

fn out_of_range(path: &str, v: f64, row: &Row) -> String {
    format!(
        "`{path}` = {v} is outside its range {} ({})",
        row.range, row.unit
    )
}

/// A voxel distance on a finer grid: rounded, and never below one voxel if it was
/// nonzero.
fn scaled_voxel_distance(distance: i64, ratio: f64) -> i64 {
    if distance == 0 {
        return 0;
    }
    (distance as f64 * ratio).round().clamp(1.0, f64::from(u32::MAX)) as i64
}

pub use toml;

/// The world's table (`world.*`): `cubarium_voxel::Config`, its day clock and climate.
pub fn world() -> &'static Registry {
    static TABLE: std::sync::OnceLock<Registry> = std::sync::OnceLock::new();
    TABLE.get_or_init(|| Registry::new("world", Vec::new(), Vec::new(), vec!["world.landform".into()]))
}
