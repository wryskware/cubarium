//! The parameter registry's dump (package R1, `design/ecology-parameter-registry-2026-09-28.md`):
//! every path with its value, unit and range, after the layers a run would apply.
//!
//! ```text
//! cargo run -p cubarium-voxel-sim --example params                       # every leaf
//! cargo run -p cubarium-voxel-sim --example params -- --config config/desktop/terrarium.toml \
//!     --set fauna.founders.frondgrazer.core.maintenance_per_s=0.002
//! cargo run -p cubarium-voxel-sim --example params -- --markdown         # design/backlog.md §1
//! cargo run -p cubarium-voxel-sim --example params -- --toml             # what a run writes
//! cargo run -p cubarium-voxel-sim --example params -- --backlog design/backlog.md
//! ```
//!
//! `--backlog FILE` rewrites the block between the `registry dump` markers in `FILE`
//! (`design/backlog.md` §1) with the markdown dump of the defaults.

use cubarium_voxel_sim::params::{Layers, Params};

const BEGIN: &str = "<!-- registry dump: begin (cargo run -p cubarium-voxel-sim --example params -- --backlog design/backlog.md) -->";
const END: &str = "<!-- registry dump: end -->";

fn main() {
    let mut args = std::env::args().skip(1);
    let mut layers = Layers::default();
    let mut mode = "list";
    let mut backlog: Option<String> = None;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--config" => {
                let path = args.next().expect("--config FILE");
                let text = std::fs::read_to_string(&path)
                    .unwrap_or_else(|e| panic!("cannot read {path}: {e}"));
                layers.file = Some(text.parse().unwrap_or_else(|e| panic!("{path}: {e}")));
            }
            "--set" => layers.sets.push(args.next().expect("--set path=value")),
            "--markdown" => mode = "markdown",
            "--toml" => mode = "toml",
            "--backlog" => {
                mode = "backlog";
                backlog = Some(args.next().expect("--backlog FILE"));
            }
            other => panic!("unknown argument {other}"),
        }
    }
    let params = Params::resolve(&layers).unwrap_or_else(|e| {
        eprintln!("params: {e}");
        std::process::exit(2)
    });
    match mode {
        "markdown" => print!("{}", params.markdown().expect("the markdown")),
        "toml" => print!("{}", params.to_toml().expect("the TOML")),
        "backlog" => {
            let path = backlog.expect("a file");
            let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
            let (begin, end) = (BEGIN, END);
            let i = text.find(begin).unwrap_or_else(|| panic!("{path} has no `{begin}`"));
            let j = text.find(end).unwrap_or_else(|| panic!("{path} has no `{end}`"));
            let block = params.markdown().expect("the markdown");
            let out = format!("{}{begin}\n{block}\n{}", &text[..i], &text[j..]);
            std::fs::write(&path, out).unwrap_or_else(|e| panic!("{path}: {e}"));
            eprintln!("rewrote the registry dump in {path}");
        }
        _ => {
            for line in params.dump().expect("the dump") {
                println!(
                    "{} = {}\t# {} {}",
                    line.path, line.value, line.row.unit, line.row.range
                );
            }
        }
    }
}
