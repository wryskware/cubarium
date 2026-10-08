//! `voxel_resave ROOT...`: rewrite every saved layer under each root that this build reads
//! but does not write (the one-version legacy reader's schemas) in the current format
//! ([`cubarium_voxel_sim::saved::resave`]). Run it over a save library before the legacy
//! reader is retired.

fn main() {
    let roots: Vec<String> = std::env::args().skip(1).collect();
    if roots.is_empty() {
        eprintln!("usage: voxel_resave ROOT...");
        std::process::exit(2);
    }
    for root in roots {
        match cubarium_voxel_sim::saved::resave(std::path::Path::new(&root)) {
            Ok(done) => println!("{root}: {} layer file(s) rewritten", done.len()),
            Err(e) => {
                eprintln!("{root}: {e}");
                std::process::exit(1);
            }
        }
    }
}
