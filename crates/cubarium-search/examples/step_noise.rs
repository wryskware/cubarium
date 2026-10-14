//! **How much of a training step is signal** (TRAIN-REVIEW, 2026-10-14): re-forms a twins
//! run's per-generation steps from its logged pair scores and the trainer's own
//! perturbation stream and gradient code, then measures:
//!
//! - `split`: the cosine between the steps of two random halves of a generation's pairs
//!   (each half ranked on its own, as the trainer would rank a generation of that size),
//!   averaged over `splits` random splits;
//! - `null`: the same cosine after shuffling the scores across pairs (noise floor);
//! - `next`: the cosine between consecutive generations' whole steps;
//! - a replay of the run's Adam from its starting centre, checked against its final centre
//!   (proves the perturbations regenerate exactly), with the net drift against the path.
//!
//! ```text
//! step_noise run=runs/lab/<job>/run/run start=<starting centre .json> \
//!     [train_seed=20260924] [sigma=0.05] [lr=0.01] [splits=16] [stage=top]
//! ```
//!
//! One JSON line per generation, then a summary line. Analysis only; it trains nothing.

use cubarium_search::es::optimizer::{self, Adam};
use cubarium_search::es::voxel::kin::config::BrainStage;
use cubarium_search::es::voxel::kin::stage::Perturber;
use cubarium_search::es::voxel::two_layer::{Layer, TwoLayerPolicyFile};
use std::io::BufRead;
use std::path::Path;

fn arg<'a>(args: &'a [String], key: &str) -> Option<&'a str> {
    args.iter().find_map(|a| a.strip_prefix(&format!("{key}=")))
}

fn cos(a: &[f64], b: &[f64]) -> f64 {
    let (mut ab, mut aa, mut bb) = (0.0, 0.0, 0.0);
    for (x, y) in a.iter().zip(b) {
        ab += x * y;
        aa += x * x;
        bb += y * y;
    }
    if aa == 0.0 || bb == 0.0 { 0.0 } else { ab / (aa * bb).sqrt() }
}

fn splitmix(x: &mut u64) -> u64 {
    *x = x.wrapping_add(0x9e37_79b9_7f4a_7c15);
    let mut z = *x;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

fn shuffle<T>(v: &mut [T], seed: u64) {
    let mut x = seed;
    for i in (1..v.len()).rev() {
        let j = (splitmix(&mut x) % (i as u64 + 1)) as usize;
        v.swap(i, j);
    }
}

/// The trainer's step over `pairs` (indices into the generation's pairs), with `d` the
/// pair differences: `ascend_pairs` enters pair `p` as `(d/2, −d/2)` into
/// [`optimizer::gradient`]. `eps[p]` is the top slice of pair `p`'s perturbation.
fn step(pairs: &[usize], d: &[f64], eps: &[Vec<f64>], sigma: f64) -> Vec<f64> {
    let plus: Vec<f64> = pairs.iter().map(|&p| d[p] / 2.0).collect();
    let minus: Vec<f64> = pairs.iter().map(|&p| -d[p] / 2.0).collect();
    let dim = eps[0].len();
    optimizer::gradient(&plus, &minus, dim, sigma, |i, out| out.copy_from_slice(&eps[pairs[i]]))
}

/// The raw (unranked) estimator `Σ d_p ε_p / |pairs|`, for the variance-share algebra.
fn raw(pairs: &[usize], d: &[f64], eps: &[Vec<f64>]) -> Vec<f64> {
    let mut g = vec![0.0; eps[0].len()];
    for &p in pairs {
        for (x, e) in g.iter_mut().zip(&eps[p]) {
            *x += d[p] * e;
        }
    }
    let n = pairs.len() as f64;
    g.iter_mut().for_each(|x| *x /= n);
    g
}

fn mean_se(v: &[f64]) -> (f64, f64) {
    let n = v.len() as f64;
    let m = v.iter().sum::<f64>() / n;
    let var = v.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (n - 1.0).max(1.0);
    (m, (var / n).sqrt())
}

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let run = Path::new(arg(&args, "run").ok_or("run=DIR")?);
    let start = TwoLayerPolicyFile::load(Path::new(arg(&args, "start").ok_or("start=CENTRE")?))?;
    let train_seed: u64 = arg(&args, "train_seed").unwrap_or("20260924").parse().map_err(|e| format!("{e}"))?;
    let sigma: f64 = arg(&args, "sigma").unwrap_or("0.05").parse().map_err(|e| format!("{e}"))?;
    let lr: f64 = arg(&args, "lr").unwrap_or("0.01").parse().map_err(|e| format!("{e}"))?;
    let splits: usize = arg(&args, "splits").unwrap_or("16").parse().map_err(|e| format!("{e}"))?;
    let stage = match arg(&args, "stage").unwrap_or("top") {
        "top" => BrainStage::Top,
        _ => BrainStage::Both,
    };
    let layout = start.layout();
    let perturber = Perturber::two_layer(&layout, stage);
    let top = if stage == BrainStage::Top { layout.range(Layer::Top) } else { 0..start.theta.len() };
    let dim = top.len();
    eprintln!("params {} perturbed {} ({}..{})", start.theta.len(), dim, top.start, top.end);

    let mut theta = start.theta.clone();
    let mut adam = Adam::new(theta.len());
    adam.learning_rate = lr;
    let mut prev: Option<Vec<f64>> = None;
    let mut path_len = 0.0;
    let (mut cs, mut cr, mut cn, mut cx, mut zero, mut rsq) = (vec![], vec![], vec![], vec![], vec![], vec![]);
    let file = std::fs::File::open(run.join("generations.jsonl")).map_err(|e| format!("{e}"))?;
    let mut full = vec![0.0; theta.len()];
    for line in std::io::BufReader::new(file).lines() {
        let line = line.map_err(|e| format!("{e}"))?;
        let r: serde_json::Value = serde_json::from_str(&line).map_err(|e| format!("{e}"))?;
        let generation = r["generation"].as_u64().ok_or("generation")? as u32;
        let l = &r["lineages"][0];
        let cand: Vec<f64> = l["candidate_scores"]
            .as_array()
            .ok_or("candidate_scores")?
            .iter()
            .map(|v| v.as_f64().unwrap_or(f64::NAN))
            .collect();
        let n = cand.len() / 2;
        // A pair's twin difference d = plus − minus = 2·(d/2); a missing pair is NaN.
        let d: Vec<f64> = (0..n).map(|p| cand[2 * p] - cand[2 * p + 1]).collect();
        let scored: Vec<usize> = (0..n).filter(|&p| d[p].is_finite()).collect();
        let eps: Vec<Vec<f64>> = (0..n)
            .map(|p| {
                perturber.eps(train_seed, generation, p, &mut full);
                full[top.clone()].to_vec()
            })
            .collect();
        // The generation's whole step, through Adam (the replay).
        let g = step(&scored, &d, &eps, sigma);
        let mut g_full = vec![0.0; theta.len()];
        g_full[top.clone()].copy_from_slice(&g);
        let before = theta.clone();
        let rms = adam.ascend(&mut theta, &g_full);
        path_len += theta.iter().zip(&before).map(|(a, b)| (a - b).powi(2)).sum::<f64>().sqrt();
        let logged_rms = l["update_rms"].as_f64().unwrap_or(f64::NAN);
        // Halves.
        let (mut c_split, mut c_raw, mut c_null) = (0.0, 0.0, 0.0);
        for s in 0..splits {
            let mut order = scored.clone();
            shuffle(&mut order, u64::from(generation) << 16 ^ s as u64 ^ 0x5eed);
            let (a, b) = order.split_at(order.len() / 2);
            c_split += cos(&step(a, &d, &eps, sigma), &step(b, &d, &eps, sigma));
            c_raw += cos(&raw(a, &d, &eps), &raw(b, &d, &eps));
            let mut dn = d.clone();
            let mut vals: Vec<f64> = scored.iter().map(|&p| d[p]).collect();
            shuffle(&mut vals, u64::from(generation) << 16 ^ s as u64 ^ 0xbad);
            for (&p, v) in scored.iter().zip(vals) {
                dn[p] = v;
            }
            c_null += cos(&step(a, &dn, &eps, sigma), &step(b, &dn, &eps, sigma));
        }
        let k = splits as f64;
        let (c_split, c_raw, c_null) = (c_split / k, c_raw / k, c_null / k);
        let c_next = prev.as_ref().map(|p| cos(p, &g));
        let zeros = scored.iter().filter(|&&p| d[p] == 0.0).count() as f64 / scored.len().max(1) as f64;
        // Share of a pair's difference its own variant explains, from the raw split cosine:
        // cos ≈ S/(S + D·E[d²]/h) ⇒ R² = S/E[d²] ≈ c·D/(h·(1−c)).
        let h = (scored.len() / 2) as f64;
        let r2 = c_raw * dim as f64 / (h * (1.0 - c_raw));
        println!(
            "{{\"generation\":{generation},\"pairs\":{},\"zero\":{zeros:.3},\"split\":{c_split:.4},\"split_raw\":{c_raw:.4},\"null\":{c_null:.4},\"next\":{},\"r2\":{r2:.4},\"rms\":{rms:.6},\"logged_rms\":{logged_rms:.6}}}",
            scored.len(),
            c_next.map_or("null".into(), |c| format!("{c:.4}")),
        );
        cs.push(c_split);
        cr.push(c_raw);
        cn.push(c_null);
        if let Some(c) = c_next {
            cx.push(c);
        }
        zero.push(zeros);
        rsq.push(r2);
        prev = Some(g);
    }
    // Replay against the run's final centre.
    let fin = std::fs::read_dir(run.join("centres"))
        .map_err(|e| format!("{e}"))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .find(|p| p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("final-")));
    let replay = match fin {
        Some(p) => {
            let f = TwoLayerPolicyFile::load(&p)?;
            let max = f.theta.iter().zip(&theta).map(|(a, b)| (a - b).abs()).fold(0.0, f64::max);
            format!("{max:e}")
        }
        None => "\"no final centre\"".into(),
    };
    let drift = theta.iter().zip(&start.theta).map(|(a, b)| (a - b).powi(2)).sum::<f64>().sqrt();
    let norm = |v: &[f64]| v[top.clone()].iter().map(|x| x * x).sum::<f64>().sqrt();
    let (norm0, norm1) = (norm(&start.theta), norm(&theta));
    let (ms, ss) = mean_se(&cs);
    let (mr, sr) = mean_se(&cr);
    let (mn, sn) = mean_se(&cn);
    let (mx, sx) = mean_se(&cx);
    let (mz, _) = mean_se(&zero);
    let (m2, s2) = mean_se(&rsq);
    println!(
        "{{\"summary\":\"{}\",\"generations\":{},\"perturbed\":{dim},\"split\":[{ms:.4},{ss:.4}],\"split_raw\":[{mr:.4},{sr:.4}],\"null\":[{mn:.4},{sn:.4}],\"next\":[{mx:.4},{sx:.4}],\"r2\":[{m2:.4},{s2:.4}],\"zero\":{mz:.3},\"replay_max_abs_diff\":{replay},\"drift\":{drift:.4},\"path\":{path_len:.4},\"top_norm\":[{norm0:.4},{norm1:.4}]}}",
        run.display(),
        cs.len()
    );
    Ok(())
}
