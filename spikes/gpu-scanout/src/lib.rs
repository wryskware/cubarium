//! Shared bits for the GPU-scanout spike: the KMS output and a tiny stats helper.
pub mod kms;

/// Print min/p50/p95/max/mean for a millisecond sample.
pub fn stat(name: &str, v: &[f64]) {
    if v.is_empty() {
        println!("{name}: no samples");
        return;
    }
    let mut s = v.to_vec();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mean = s.iter().sum::<f64>() / s.len() as f64;
    println!(
        "{name}: n={} min={:.2} p50={:.2} p95={:.2} max={:.2} mean={:.2} ms",
        s.len(),
        s[0],
        s[s.len() / 2],
        s[(s.len() * 95 / 100).min(s.len() - 1)],
        s[s.len() - 1],
        mean
    );
}
