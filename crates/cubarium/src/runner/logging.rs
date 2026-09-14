use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use cubarium_core::view::FieldDump;
use cubarium_core::{LifeEvent, Telemetry};

pub(super) struct TelemetryLog {
    path: PathBuf,
    file: std::fs::File,
    pub(super) samples: u64,
}

impl TelemetryLog {
    pub(super) fn open(path: &Path) -> Result<TelemetryLog> {
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent).with_context(|| {
                format!("creating the telemetry directory {}", parent.display())
            })?;
        }
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .with_context(|| format!("opening the telemetry file {}", path.display()))?;
        Ok(TelemetryLog {
            path: path.to_path_buf(),
            file,
            samples: 0,
        })
    }

    /// A telemetry write failure is an observer problem, never a reason to stop the world.
    pub(super) fn write(&mut self, sample: &Telemetry) {
        let line = match serde_json::to_string(sample) {
            Ok(line) => line,
            Err(e) => {
                eprintln!("cubarium: cannot encode telemetry: {e}");
                return;
            }
        };
        if let Err(e) = writeln!(self.file, "{line}").and_then(|()| self.file.flush()) {
            eprintln!("cubarium: cannot append to {}: {e}", self.path.display());
            return;
        }
        self.samples += 1;
    }
}

/// Four decimals, the precision `design/m2-world-spec.md` "Observer" asks field dumps
/// for. Non-finite values become JSON `null` rather than a lie a reader cannot detect.
pub(super) fn rounded(values: &[f64]) -> Vec<serde_json::Value> {
    values
        .iter()
        .map(|x| {
            serde_json::Number::from_f64((x * 1e4).round() / 1e4)
                .map_or(serde_json::Value::Null, serde_json::Value::Number)
        })
        .collect()
}

/// One field array as a JSON array of numbers. Written by hand rather than through a
/// map so the keys keep the spec's order (`tick, n, p, d, de, organisms`); `serde_json`
/// maps would sort them and put `tick` last, where nobody tailing the file expects it.
fn field_array(values: &[f64]) -> String {
    serde_json::Value::Array(rounded(values)).to_string()
}

/// `fields.jsonl`: a header naming each cell's graph neighbors, then one line per dump
/// on the telemetry cadence rule. Append-only like the telemetry log, and equally
/// non-fatal: a dump that cannot be written is reported and the world goes on.
pub(super) struct FieldLog {
    path: PathBuf,
    file: std::fs::File,
    /// True when this process created (or found empty) the file, so it owes a header.
    pub(super) fresh: bool,
    lines: u64,
}

impl FieldLog {
    pub(super) fn open(path: &Path) -> Result<FieldLog> {
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent).with_context(|| {
                format!("creating the field dump directory {}", parent.display())
            })?;
        }
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .with_context(|| format!("opening the field dump file {}", path.display()))?;
        // An existing file already carries its header; only a new or empty one is owed
        // one, because the neighbor graph never changes within a world.
        let fresh = file.metadata().map(|m| m.len() == 0).unwrap_or(true);
        Ok(FieldLog {
            path: path.to_path_buf(),
            file,
            fresh,
            lines: 0,
        })
    }

    fn append(&mut self, line: &str) {
        if let Err(e) = writeln!(self.file, "{line}").and_then(|()| self.file.flush()) {
            eprintln!("cubarium: cannot append to {}: {e}", self.path.display());
            return;
        }
        self.lines += 1;
    }

    /// `{"cells": [[n0, n1, n2, n3], …]}` in `Edge` order, `null` at the rim.
    pub(super) fn write_header(&mut self, neighbors: &[[Option<u16>; 4]]) {
        let cells: Vec<serde_json::Value> = neighbors
            .iter()
            .map(|cell| {
                serde_json::Value::Array(
                    cell.iter()
                        .map(|n| n.map_or(serde_json::Value::Null, serde_json::Value::from))
                        .collect(),
                )
            })
            .collect();
        self.append(&serde_json::json!({ "cells": cells }).to_string());
    }

    pub(super) fn write(&mut self, dump: &FieldDump) {
        let organisms = serde_json::Value::from(dump.organisms.as_slice()).to_string();
        self.append(&format!(
            r#"{{"tick":{},"n":{},"p":{},"d":{},"de":{},"f":{},"w":{},"organisms":{organisms}}}"#,
            dump.tick,
            field_array(&dump.n),
            field_array(&dump.p),
            field_array(&dump.d),
            field_array(&dump.de),
            field_array(&dump.f),
            field_array(&dump.w),
        ));
    }
}

/// The key order `design/m2-world-spec.md` "Observer" prints a birth record in. Keys the
/// world adds later are not dropped: they are appended after these.
const BIRTH_KEYS: [&str; 8] = [
    "kind",
    "tick",
    "id",
    "parent",
    "parent_age_ticks",
    "parent_births",
    "genome",
    "origin",
];
/// The same for a death record.
const DEATH_KEYS: [&str; 7] = [
    "kind",
    "tick",
    "id",
    "age_ticks",
    "cause",
    "births",
    "genome",
];

/// `slot:generation` for an `OrganismId` the world serialized as `{slot, generation}`.
/// Anything else (a string the world already renders itself, a missing field) is left
/// exactly as it came, so this can never invent an id.
fn organism_id(value: &serde_json::Value) -> Option<serde_json::Value> {
    let (slot, generation) = (value.get("slot")?, value.get("generation")?);
    Some(serde_json::Value::from(format!("{slot}:{generation}")))
}

/// One life event as the spec's line: ids as `slot:generation`, `origin` and `cause`
/// lowercase, and the spec's key order so a reader sees `kind` and `tick` first.
fn event_line(event: &LifeEvent) -> Result<String> {
    let value = serde_json::to_value(event).context("encoding a life event")?;
    let mut fields = match value {
        serde_json::Value::Object(map) => map,
        other => anyhow::bail!("a life event is not a JSON object: {other}"),
    };
    for key in ["id", "parent"] {
        if let Some(rendered) = fields.get(key).and_then(organism_id) {
            fields.insert(key.to_string(), rendered);
        }
    }
    for key in ["origin", "cause"] {
        if let Some(name) = fields
            .get(key)
            .and_then(|v| v.as_str())
            .map(str::to_lowercase)
        {
            fields.insert(key.to_string(), serde_json::Value::from(name));
        }
    }
    let spec_order: &[&str] = match fields.get("kind").and_then(|v| v.as_str()) {
        Some("death") => &DEATH_KEYS,
        _ => &BIRTH_KEYS,
    };
    // Assembled as text: a `serde_json` map is sorted, which would bury `kind` and
    // `tick` in the middle of the line.
    let pair =
        |key: &str, value: &serde_json::Value| format!("{}:{value}", serde_json::Value::from(key));
    let mut parts = Vec::with_capacity(fields.len());
    for key in spec_order {
        if let Some(value) = fields.remove(*key) {
            parts.push(pair(key, &value));
        }
    }
    // Whatever the world grew since this list was written goes on the end, in key order,
    // rather than being silently lost.
    parts.extend(fields.iter().map(|(key, value)| pair(key, value)));
    Ok(format!("{{{}}}", parts.join(",")))
}

/// `events.jsonl`: one line per birth and death, appended in the order the world
/// reported them. Non-fatal like the other observer files.
pub(super) struct EventLog {
    path: PathBuf,
    file: std::fs::File,
    lines: u64,
}

impl EventLog {
    pub(super) fn open(path: &Path) -> Result<EventLog> {
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent).with_context(|| {
                format!("creating the event log directory {}", parent.display())
            })?;
        }
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .with_context(|| format!("opening the event log {}", path.display()))?;
        Ok(EventLog {
            path: path.to_path_buf(),
            file,
            lines: 0,
        })
    }

    /// Write one tick's events and flush once. A tick with no events touches no disk.
    pub(super) fn write_tick(&mut self, events: &[LifeEvent]) {
        if events.is_empty() {
            return;
        }
        let mut written = 0u64;
        for event in events {
            let line = match event_line(event) {
                Ok(line) => line,
                Err(e) => {
                    eprintln!("cubarium: cannot encode a life event: {e:#}");
                    continue;
                }
            };
            if let Err(e) = writeln!(self.file, "{line}") {
                eprintln!("cubarium: cannot append to {}: {e}", self.path.display());
                return;
            }
            written += 1;
        }
        if let Err(e) = self.file.flush() {
            eprintln!("cubarium: cannot flush {}: {e}", self.path.display());
            return;
        }
        self.lines += written;
    }
}

/// The one-line stderr digest headless runs get for every sample, so a `--sink none` run
/// is observable without opening the telemetry file.
pub(super) fn headless_line(sample: &Telemetry) -> String {
    // Forms are listed up to the last one alive, so a four-kind world prints four numbers
    // and a v1 world (all hue terciles) three, without eight trailing zeros.
    let last_form = sample
        .population_by_form
        .iter()
        .rposition(|&n| n > 0)
        .map_or(0, |i| i + 1);
    let forms = sample.population_by_form[..last_form]
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join("/");
    format!(
        "cubarium: tick {} pop {} births {} deaths {} forms {} fruit {:.2} water {:.1} residual {:.3e} hash {:016x}",
        sample.tick,
        sample.population,
        sample.births,
        sample.deaths_starvation + sample.deaths_age + sample.deaths_collapse,
        if forms.is_empty() {
            "-".to_string()
        } else {
            forms
        },
        sample.fruit,
        sample.water,
        sample.mass_residual,
        sample.state_hash,
    )
}
