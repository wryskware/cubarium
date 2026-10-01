//! **The search genome** (R3: `design/ecology-parameter-registry-2026-09-28.md` §3–§6):
//! a search is a file of genes over registry paths, and one generic decode replaces the
//! hand-written routing of every searched number.
//!
//! ```toml
//! # config/training/search/<name>.toml
//! retrain = false          # true lets a gene move a `digest` row (a trained brain's senses)
//! free = ["intake_follows_upkeep"]   # ties (by name) or derived fields (by path) set free
//!
//! [[gene]]
//! name  = "upkeep"
//! path  = "fauna.founders.*.core.maintenance_per_s"   # a glob, or several in an array
//! tie   = "each"           # one gene per value of the first `*`/`@group` segment; "all" (default)
//! range = [0.1, 4.0]       # the multiplier's box; the rows' search intervals otherwise
//! # inverse = [...]        # paths divided by the multiplier instead (a duration under a clock)
//! # mode = "value"         # set the value instead of multiplying it
//! # value = 1.3            # a genome file carries each gene's value; x1 when absent
//! ```
//!
//! - **A path** is a registry path. `*` is any one segment, `@group` as a segment any member
//!   of that segment group, and `@group` alone every glob of a path group
//!   ([`crate::registry::Group`]). A gene path that matches no leaf, an unknown group and an
//!   unknown key are errors.
//! - **Each gene is a multiplier** (`mode = "scale"`): ×1 is the config it decodes over.
//!   Several genes on one leaf multiply.
//! - **Derived fields** ([`crate::registry::Tie`]) follow their bases after the genes, unless
//!   freed. One gene on both a derived field and its base would count twice and is refused.
//! - **Digest rows** — what a trained controller's sense contract hashes — are refused
//!   unless the file says `retrain = true`.
//! - Any other table (a search's `[screen]`) is kept as [`Genome::rest`] for its reader.

use std::collections::{HashMap, HashSet};

use toml::Value;

use crate::registry::{GroupKind, Registry};

/// How a gene's value acts on the leaves it matches.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Multiply (default): ×1 is the decoded-over config.
    Scale,
    /// Replace.
    Value,
}

/// One searched number.
#[derive(Clone, Debug, PartialEq)]
pub struct Gene {
    pub name: String,
    /// Globs the value multiplies (or sets).
    pub paths: Vec<String>,
    /// Globs the value divides: durations under a rate gene.
    pub inverse: Vec<String>,
    pub mode: Mode,
    /// The box: a multiplier's for `Scale`, the value's for `Value`.
    pub range: (f64, f64),
    pub value: f64,
    /// One line on what it is.
    pub what: String,
}

/// A search file, expanded over the configs it decodes: one entry per searched number.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Genome {
    pub retrain: bool,
    pub free: Vec<String>,
    pub genes: Vec<Gene>,
    /// Every other table of the file (`[screen]`), for its own reader.
    pub rest: toml::Table,
}

/// One gene as the file writes it, before `tie = "each"` is expanded.
#[derive(Clone, Debug)]
struct Spec {
    name: String,
    paths: Vec<String>,
    inverse: Vec<String>,
    mode: Mode,
    each: bool,
    range: Option<(f64, f64)>,
    value: Option<f64>,
    what: String,
}

/// A search file, parsed.
#[derive(Clone, Debug)]
pub struct SearchFile {
    retrain: bool,
    free: Vec<String>,
    specs: Vec<Spec>,
    rest: toml::Table,
}

/// One crate's table and its serialized config.
pub type Part<'a> = (&'a Registry, &'a Value);

fn strings(v: &Value, what: &str) -> Result<Vec<String>, String> {
    match v {
        Value::String(s) => Ok(vec![s.clone()]),
        Value::Array(a) => a
            .iter()
            .map(|x| {
                x.as_str()
                    .map(str::to_string)
                    .ok_or_else(|| format!("{what}: {x} is not a path"))
            })
            .collect(),
        other => Err(format!("{what}: {other} is not a path or a list of paths")),
    }
}

fn number(v: &Value) -> Option<f64> {
    v.as_float().or_else(|| v.as_integer().map(|i| i as f64))
}

impl SearchFile {
    /// Parse a search (or genome) file. Unknown keys are errors; any other table is kept.
    pub fn parse(text: &str) -> Result<SearchFile, String> {
        let doc: toml::Table = text
            .parse()
            .map_err(|e| format!("the search file is not TOML: {e}"))?;
        let mut file = SearchFile {
            retrain: false,
            free: Vec::new(),
            specs: Vec::new(),
            rest: toml::Table::new(),
        };
        for (key, v) in doc {
            match key.as_str() {
                "retrain" => {
                    file.retrain = v.as_bool().ok_or("retrain is true or false")?;
                }
                "free" => file.free = strings(&v, "free")?,
                "gene" => {
                    let genes = v.as_array().ok_or("gene is a [[gene]] array")?;
                    for g in genes {
                        file.specs.push(Spec::parse(g)?);
                    }
                }
                _ if v.is_table() => {
                    file.rest.insert(key, v);
                }
                _ => return Err(format!("unknown key `{key}` in the search file")),
            }
        }
        Ok(file)
    }
}

impl Spec {
    fn parse(v: &Value) -> Result<Spec, String> {
        let t = v.as_table().ok_or("a [[gene]] is a table")?;
        let mut s = Spec {
            name: String::new(),
            paths: Vec::new(),
            inverse: Vec::new(),
            mode: Mode::Scale,
            each: false,
            range: None,
            value: None,
            what: String::new(),
        };
        for (k, x) in t {
            match k.as_str() {
                "name" => s.name = x.as_str().ok_or("a gene's name is a string")?.to_string(),
                "path" => s.paths = strings(x, "path")?,
                "inverse" => s.inverse = strings(x, "inverse")?,
                "mode" => {
                    s.mode = match x.as_str() {
                        Some("scale") => Mode::Scale,
                        Some("value") => Mode::Value,
                        _ => return Err(format!("mode is \"scale\" or \"value\", not {x}")),
                    }
                }
                "tie" => {
                    s.each = match x.as_str() {
                        Some("all") => false,
                        Some("each") => true,
                        _ => return Err(format!("tie is \"all\" or \"each\", not {x}")),
                    }
                }
                "range" => {
                    let a = x.as_array().filter(|a| a.len() == 2);
                    let (lo, hi) = a
                        .and_then(|a| Some((number(&a[0])?, number(&a[1])?)))
                        .ok_or_else(|| format!("range is [lo, hi], not {x}"))?;
                    if !(lo.is_finite() && hi.is_finite() && lo <= hi) {
                        return Err(format!("range [{lo}, {hi}] is not an interval"));
                    }
                    s.range = Some((lo, hi));
                }
                "value" => s.value = Some(number(x).ok_or("a gene's value is a number")?),
                "what" => s.what = x.as_str().unwrap_or_default().to_string(),
                other => return Err(format!("unknown key `{other}` in a [[gene]]")),
            }
        }
        if s.paths.is_empty() && s.inverse.is_empty() {
            return Err(format!("gene `{}` names no path", s.name));
        }
        if s.name.is_empty() {
            s.name = s.paths.first().cloned().unwrap_or_default();
        }
        Ok(s)
    }
}

/// A glob split into segments, matched against a concrete path's segments; the captured
/// `*`/`@group` segments in order, or `None`.
fn captures(glob: &[&str], path: &[&str], parts: &[Part<'_>]) -> Option<Vec<String>> {
    if glob.len() != path.len() {
        return None;
    }
    let mut caps = Vec::new();
    for (g, p) in glob.iter().zip(path) {
        if *g == "*" {
            caps.push(p.to_string());
        } else if let Some(name) = g.strip_prefix('@') {
            if !segment_group(name, parts)?.iter().any(|m| m == p) {
                return None;
            }
            caps.push(p.to_string());
        } else if g != p {
            return None;
        }
    }
    Some(caps)
}

fn segment_group(name: &str, parts: &[Part<'_>]) -> Option<Vec<String>> {
    let mut found = None;
    for (reg, _) in parts {
        for g in reg.groups() {
            if let (true, GroupKind::Segment(m)) = (g.name == name, &g.kind) {
                found.get_or_insert_with(Vec::new).extend(m.iter().cloned());
            }
        }
    }
    found
}

/// A gene path as globs: a path group's members, or the path itself. Every `@group`
/// segment must name a segment group.
fn globs(path: &str, parts: &[Part<'_>]) -> Result<Vec<String>, String> {
    let out = if let Some(name) = path.strip_prefix('@').filter(|n| !n.contains('.')) {
        let mut members = Vec::new();
        let mut known = false;
        for (reg, _) in parts {
            for g in reg.groups() {
                if let (true, GroupKind::Paths(m)) = (g.name == name, &g.kind) {
                    known = true;
                    members.extend(m.iter().cloned());
                }
            }
        }
        if !known {
            return Err(format!("unknown group `@{name}` in `{path}`"));
        }
        members
    } else {
        vec![path.to_string()]
    };
    for g in &out {
        for seg in g.split('.') {
            if let Some(name) = seg.strip_prefix('@')
                && segment_group(name, parts).is_none()
            {
                return Err(format!("unknown group `@{name}` in `{path}`"));
            }
        }
    }
    Ok(out)
}

fn glob_match(glob: &str, path: &str, parts: &[Part<'_>]) -> bool {
    let g: Vec<&str> = glob.split('.').collect();
    let p: Vec<&str> = path.split('.').collect();
    captures(&g, &p, parts).is_some()
}

/// Every leaf of every part, `(part, path, value, integer)`.
fn all_leaves(parts: &[Part<'_>]) -> Vec<Vec<(String, f64, bool)>> {
    parts.iter().map(|(reg, v)| reg.typed_leaves(v)).collect()
}

/// `glob` with its first `*` or `@group` segment replaced by `seg`; `None` without one.
fn substitute_first(glob: &str, seg: &str) -> Option<String> {
    let mut parts: Vec<String> = glob.split('.').map(str::to_string).collect();
    let k = parts.iter().position(|s| s == "*" || s.starts_with('@'))?;
    parts[k] = seg.to_string();
    Some(parts.join("."))
}

impl Genome {
    /// Expand `file` over `parts` (each crate's table and its config before the genome):
    /// `tie = "each"` split, ranges filled, and the whole genome checked by a decode.
    pub fn expand(file: SearchFile, parts: &[Part<'_>]) -> Result<Genome, String> {
        let leaves = all_leaves(parts);
        let mut genes = Vec::new();
        for spec in file.specs {
            let mut paths = Vec::new();
            for p in &spec.paths {
                paths.extend(globs(p, parts)?);
            }
            let mut inverse = Vec::new();
            for p in &spec.inverse {
                inverse.extend(globs(p, parts)?);
            }
            let split: Vec<(String, Vec<String>, Vec<String>)> = if spec.each {
                let first = paths
                    .iter()
                    .chain(&inverse)
                    .find(|g| g.split('.').any(|s| s == "*" || s.starts_with('@')))
                    .ok_or_else(|| {
                        format!(
                            "gene `{}`: tie = \"each\" needs a `*` or `@group`",
                            spec.name
                        )
                    })?
                    .clone();
                let k = first
                    .split('.')
                    .position(|s| s == "*" || s.starts_with('@'))
                    .expect("found above");
                let mut values: Vec<String> = Vec::new();
                for part in &leaves {
                    for (path, _, _) in part {
                        if glob_match(&first, path, parts) {
                            let seg = path.split('.').nth(k).expect("same length").to_string();
                            if !values.contains(&seg) {
                                values.push(seg);
                            }
                        }
                    }
                }
                if values.is_empty() {
                    return Err(format!(
                        "gene `{}`: `{first}` matches no parameter",
                        spec.name
                    ));
                }
                let sub = |gs: &[String], v: &str| -> Vec<String> {
                    gs.iter()
                        .map(|g| substitute_first(g, v).unwrap_or_else(|| g.clone()))
                        .collect()
                };
                values
                    .iter()
                    .map(|v| {
                        (
                            format!("{}@{v}", spec.name),
                            sub(&paths, v),
                            sub(&inverse, v),
                        )
                    })
                    .collect()
            } else {
                vec![(spec.name.clone(), paths, inverse)]
            };
            for (name, paths, inverse) in split {
                let range = match spec.range {
                    Some(r) => r,
                    None => default_range(&name, &paths, spec.mode, parts, &leaves)?,
                };
                let value = match (spec.value, spec.mode) {
                    (Some(v), _) => v,
                    (None, Mode::Scale) => 1.0,
                    (None, Mode::Value) => {
                        return Err(format!("gene `{name}` (mode = \"value\") needs a value"));
                    }
                };
                if !(range.0..=range.1).contains(&value) {
                    return Err(format!(
                        "gene `{name}` = {value} is outside its range [{}, {}]",
                        range.0, range.1
                    ));
                }
                genes.push(Gene {
                    name,
                    paths,
                    inverse,
                    mode: spec.mode,
                    range,
                    value,
                    what: spec.what.clone(),
                });
            }
        }
        let mut seen = HashSet::new();
        for g in &genes {
            if !seen.insert(g.name.clone()) {
                return Err(format!("two genes are called `{}`", g.name));
            }
        }
        let genome = Genome {
            retrain: file.retrain,
            free: file.free,
            genes,
            rest: file.rest,
        };
        genome.resolve(parts)?;
        Ok(genome)
    }

    /// The edits this genome makes to each part's config, in `parts` order: `(path,
    /// value)` for every leaf whose value changes. Checks every rule of the module docs.
    pub fn resolve(&self, parts: &[Part<'_>]) -> Result<Vec<Vec<(String, Value)>>, String> {
        let leaves = all_leaves(parts);
        let mut matched = vec![false; self.genes.len()];
        let mut out = Vec::new();
        let mut free_used = vec![false; self.free.len()];
        for (pi, (reg, _)) in parts.iter().enumerate() {
            let base: HashMap<&str, (f64, bool)> = leaves[pi]
                .iter()
                .map(|(p, v, i)| (p.as_str(), (*v, *i)))
                .collect();
            // The genes' factor and set value per leaf.
            let mut factor: HashMap<&str, f64> = HashMap::new();
            let mut set: HashMap<&str, f64> = HashMap::new();
            let mut by_gene: Vec<Vec<&str>> = vec![Vec::new(); self.genes.len()];
            for (gi, g) in self.genes.iter().enumerate() {
                for (globs, sign) in [(&g.paths, 1.0), (&g.inverse, -1.0)] {
                    for glob in globs {
                        for (path, _, _) in &leaves[pi] {
                            if !glob_match(glob, path, parts) {
                                continue;
                            }
                            matched[gi] = true;
                            by_gene[gi].push(path.as_str());
                            if !self.retrain && reg.row_for(path).is_some_and(|r| r.digest) {
                                return Err(format!(
                                    "gene `{}` moves `{path}`, a digest row (a trained brain's \
                                     senses read it): set `retrain = true` to retrain",
                                    g.name
                                ));
                            }
                            match g.mode {
                                Mode::Scale => {
                                    *factor.entry(path.as_str()).or_insert(1.0) *=
                                        g.value.powf(sign);
                                }
                                Mode::Value => {
                                    if set.insert(path.as_str(), g.value).is_some() {
                                        return Err(format!("two value genes set `{path}`"));
                                    }
                                }
                            }
                        }
                    }
                }
            }
            // The ties in force: (derived leaf, its base leaves with exponents).
            let mut derived: HashMap<&str, Vec<(String, f64)>> = HashMap::new();
            for tie in reg.ties() {
                let fg: Vec<&str> = tie.field.split('.').collect();
                for (path, _, _) in &leaves[pi] {
                    let segs: Vec<&str> = path.split('.').collect();
                    let Some(caps) = captures(&fg, &segs, parts) else {
                        continue;
                    };
                    let mut freed = false;
                    for (k, f) in self.free.iter().enumerate() {
                        if f == tie.name || glob_match(f, path, parts) {
                            free_used[k] = true;
                            freed = true;
                        }
                    }
                    if freed {
                        continue;
                    }
                    for (bglob, e) in &tie.bases {
                        let mut it = caps.iter();
                        let b: Vec<String> = bglob
                            .split('.')
                            .map(|s| {
                                if s == "*" {
                                    it.next().cloned().unwrap_or_else(|| s.to_string())
                                } else {
                                    s.to_string()
                                }
                            })
                            .collect();
                        let b = b.join(".");
                        if !base.contains_key(b.as_str()) {
                            return Err(format!(
                                "tie `{}`: `{path}` follows `{b}`, which is not a parameter",
                                tie.name
                            ));
                        }
                        for (gi, g) in self.genes.iter().enumerate() {
                            if by_gene[gi].contains(&path.as_str())
                                && by_gene[gi].contains(&b.as_str())
                            {
                                return Err(format!(
                                    "gene `{}` moves `{path}` and its base `{b}` (tie `{}`): it \
                                     would count twice; free the tie or narrow the gene",
                                    g.name, tie.name
                                ));
                            }
                        }
                        derived.entry(path.as_str()).or_default().push((b, *e));
                    }
                }
            }
            // Resolve every leaf, bases before what follows them.
            let mut memo: HashMap<String, f64> = HashMap::new();
            fn value_of(
                path: &str,
                base: &HashMap<&str, (f64, bool)>,
                factor: &HashMap<&str, f64>,
                set: &HashMap<&str, f64>,
                derived: &HashMap<&str, Vec<(String, f64)>>,
                memo: &mut HashMap<String, f64>,
                stack: &mut Vec<String>,
            ) -> Result<f64, String> {
                if let Some(v) = memo.get(path) {
                    return Ok(*v);
                }
                if stack.iter().any(|s| s == path) {
                    return Err(format!("the ties around `{path}` form a cycle"));
                }
                stack.push(path.to_string());
                let b0 = base.get(path).map_or(0.0, |x| x.0);
                let mut v =
                    set.get(path).copied().unwrap_or(b0) * factor.get(path).copied().unwrap_or(1.0);
                if let Some(bases) = derived.get(path) {
                    for (b, e) in bases {
                        let was = base.get(b.as_str()).map_or(0.0, |x| x.0);
                        let now = value_of(b, base, factor, set, derived, memo, stack)?;
                        if was != 0.0 && now != was {
                            v *= (now / was).powf(*e);
                        }
                    }
                }
                stack.pop();
                memo.insert(path.to_string(), v);
                Ok(v)
            }
            let mut edits = Vec::new();
            for (path, b0, int) in &leaves[pi] {
                let mut stack = Vec::new();
                let v = value_of(path, &base, &factor, &set, &derived, &mut memo, &mut stack)?;
                if v.to_bits() == b0.to_bits() {
                    continue;
                }
                if !v.is_finite() {
                    return Err(format!("the genome makes `{path}` {v}"));
                }
                edits.push((
                    path.clone(),
                    if *int {
                        Value::Integer(v.round() as i64)
                    } else {
                        Value::Float(v)
                    },
                ));
            }
            out.push(edits);
        }
        for (gi, g) in self.genes.iter().enumerate() {
            if !matched[gi] {
                let paths: Vec<&str> = g
                    .paths
                    .iter()
                    .chain(&g.inverse)
                    .map(String::as_str)
                    .collect();
                return Err(format!(
                    "gene `{}`: unknown parameter path {}: it matches nothing",
                    g.name,
                    paths.join(", ")
                ));
            }
        }
        for (k, used) in free_used.iter().enumerate() {
            if !used {
                return Err(format!(
                    "free `{}` names no tie and no derived field",
                    self.free[k]
                ));
            }
        }
        Ok(out)
    }

    /// The gene names, in order: the search's dimensions.
    pub fn names(&self) -> Vec<String> {
        self.genes.iter().map(|g| g.name.clone()).collect()
    }

    pub fn get(&self, name: &str) -> Option<f64> {
        self.genes.iter().find(|g| g.name == name).map(|g| g.value)
    }

    /// Set one gene, refusing an unknown name or a value outside its range.
    pub fn set(&mut self, name: &str, value: f64) -> Result<(), String> {
        let g = self
            .genes
            .iter_mut()
            .find(|g| g.name == name)
            .ok_or_else(|| format!("no gene is called `{name}`"))?;
        if !(value.is_finite() && (g.range.0..=g.range.1).contains(&value)) {
            return Err(format!(
                "{name} = {value} is outside [{}, {}]",
                g.range.0, g.range.1
            ));
        }
        g.value = value;
        Ok(())
    }

    /// The search box per gene: natural-log multipliers for `Scale`, the value for `Value`.
    pub fn search_bounds(&self) -> Vec<(f64, f64)> {
        self.genes
            .iter()
            .map(|g| match g.mode {
                Mode::Scale => (g.range.0.ln(), g.range.1.ln()),
                Mode::Value => g.range,
            })
            .collect()
    }

    /// The genes as search coordinates ([`Genome::search_bounds`]'s space).
    pub fn to_search(&self) -> Vec<f64> {
        self.genes
            .iter()
            .map(|g| match g.mode {
                Mode::Scale => g.value.ln(),
                Mode::Value => g.value,
            })
            .collect()
    }

    /// This genome with the genes at search coordinates `x`, each clamped into its box.
    pub fn from_search(&self, x: &[f64]) -> Genome {
        assert_eq!(x.len(), self.genes.len(), "one coordinate per gene");
        let mut g = self.clone();
        for (gene, &v) in g.genes.iter_mut().zip(x) {
            gene.value = match gene.mode {
                Mode::Scale => v.exp().clamp(gene.range.0, gene.range.1),
                Mode::Value => v.clamp(gene.range.0, gene.range.1),
            };
        }
        g
    }

    /// Every gene at x1 (a value gene keeps its value).
    pub fn neutral(&self) -> Genome {
        let mut g = self.clone();
        for gene in g.genes.iter_mut().filter(|g| g.mode == Mode::Scale) {
            gene.value = 1.0;
        }
        g
    }

    /// `name=value` pairs, for a log line.
    pub fn describe(&self) -> String {
        self.genes
            .iter()
            .map(|g| format!("{}={:.3}", g.name, g.value))
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// The genome as a file, explicit: every gene expanded, with its value; parsed back by
    /// [`SearchFile::parse`] it is this genome.
    pub fn to_toml(&self, note: &str) -> String {
        let mut doc = toml::Table::new();
        if self.retrain {
            doc.insert("retrain".into(), Value::Boolean(true));
        }
        if !self.free.is_empty() {
            doc.insert(
                "free".into(),
                Value::Array(self.free.iter().cloned().map(Value::String).collect()),
            );
        }
        for (k, v) in &self.rest {
            doc.insert(k.clone(), v.clone());
        }
        let genes: Vec<Value> = self
            .genes
            .iter()
            .map(|g| {
                let mut t = toml::Table::new();
                t.insert("name".into(), Value::String(g.name.clone()));
                if !g.paths.is_empty() {
                    t.insert(
                        "path".into(),
                        Value::Array(g.paths.iter().cloned().map(Value::String).collect()),
                    );
                }
                if !g.inverse.is_empty() {
                    t.insert(
                        "inverse".into(),
                        Value::Array(g.inverse.iter().cloned().map(Value::String).collect()),
                    );
                }
                if g.mode == Mode::Value {
                    t.insert("mode".into(), Value::String("value".into()));
                }
                t.insert(
                    "range".into(),
                    Value::Array(vec![Value::Float(g.range.0), Value::Float(g.range.1)]),
                );
                t.insert("value".into(), Value::Float(g.value));
                if !g.what.is_empty() {
                    t.insert("what".into(), Value::String(g.what.clone()));
                }
                Value::Table(t)
            })
            .collect();
        doc.insert("gene".into(), Value::Array(genes));
        let mut text = String::new();
        for line in note.lines() {
            text.push_str(&format!("# {line}\n"));
        }
        text.push_str(&toml::to_string(&doc).unwrap_or_default());
        text
    }
}

/// A gene with no `range`: the intersection over its leaves of each row's search interval
/// over the leaf's value (multipliers for `Scale`).
fn default_range(
    name: &str,
    paths: &[String],
    mode: Mode,
    parts: &[Part<'_>],
    leaves: &[Vec<(String, f64, bool)>],
) -> Result<(f64, f64), String> {
    let mut range: Option<(f64, f64)> = None;
    for (pi, (reg, _)) in parts.iter().enumerate() {
        for (path, v, _) in &leaves[pi] {
            if !paths.iter().any(|g| glob_match(g, path, parts)) {
                continue;
            }
            let Some((lo, hi)) = reg.row_for(path).and_then(|r| r.search) else {
                continue;
            };
            let r = match mode {
                Mode::Scale if *v > 0.0 => (lo / v, hi / v),
                Mode::Scale => continue,
                Mode::Value => (lo, hi),
            };
            range = Some(match range {
                None => r,
                Some((a, b)) => (a.max(r.0), b.min(r.1)),
            });
        }
    }
    match range {
        Some((lo, hi)) if lo <= hi && lo > 0.0 || mode == Mode::Value && lo <= hi => Ok((lo, hi)),
        _ => Err(format!(
            "gene `{name}` needs a range: its rows name no common search interval"
        )),
    }
}
