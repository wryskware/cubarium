//! **Did the centre move in a consistent direction?** (LJ-SIGNAL) Replays a two-layer
//! `voxel-live-train` run's updates from its start centre and its `generations.jsonl`
//! (`candidate_scores`), checks the replay lands on the run's final centre, and reads how
//! consistent the generations' gradients were with each other.
//!
//! ```text
//! live_replay start=CENTRE.json run=RUN_DIR lineage=lanternjaw [sigma=0.02] [lr=0.01]
//!             [train_seed=20260924] [stage=top] [shuffles=200]
//! ```
//!
//! - `cos(g_t, g_s)`: the mean cosine between different generations' gradients. With no
//!   signal each generation's gradient is an independent random mix of its own
//!   perturbations, so the mean is 0 within the shuffle null's spread.
//! - The shuffle null: each generation's candidate scores permuted at random (the same
//!   perturbations, no link between a variant and its score), repeated `shuffles` times.
//! - `straightness`: `‖θ_end − θ_start‖` over the summed step lengths (1 = every step the
//!   same way, about `1/√steps` for a random walk without momentum).
//!
//! Reads files only; runs no world.

use cubarium_search::es::optimizer::{self, Adam};
use cubarium_search::es::voxel::kin::stage::Perturber;
use cubarium_search::es::voxel::kin::BrainStage;
use cubarium_search::es::voxel::two_layer::TwoLayerPolicyFile;

fn arg<'a>(args: &'a [String], key: &str) -> Option<&'a str> {
    args.iter().find_map(|a| a.strip_prefix(&format!("{key}=")))
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

fn cos(a: &[f64], b: &[f64]) -> f64 {
    let d = (dot(a, a) * dot(b, b)).sqrt();
    if d > 0.0 {
        dot(a, b) / d
    } else {
        0.0
    }
}

/// Mean cosine over every pair of different generations.
fn mean_pair_cos(gs: &[Vec<f64>]) -> f64 {
    let (mut s, mut n) = (0.0, 0usize);
    for i in 0..gs.len() {
        for j in i + 1..gs.len() {
            s += cos(&gs[i], &gs[j]);
            n += 1;
        }
    }
    s / n.max(1) as f64
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let start = arg(&args, "start").expect("start=CENTRE.json");
    let run = std::path::PathBuf::from(arg(&args, "run").expect("run=RUN_DIR"));
    let lineage = arg(&args, "lineage").unwrap_or("lanternjaw");
    let sigma: f64 = arg(&args, "sigma").map_or(optimizer::SIGMA, |s| s.parse().expect("sigma"));
    let lr: f64 = arg(&args, "lr").map_or(optimizer::LEARNING_RATE, |s| s.parse().expect("lr"));
    let train_seed: u64 =
        arg(&args, "train_seed").map_or(20260924, |s| s.parse().expect("train_seed"));
    let stage = match arg(&args, "stage").unwrap_or("top") {
        "top" => BrainStage::Top,
        "both" => BrainStage::Both,
        s => panic!("stage {s}: top or both"),
    };
    let shuffles: usize = arg(&args, "shuffles").map_or(200, |s| s.parse().expect("shuffles"));

    // Unvalidated reads: the files may come from another build's manifest digest.
    let read = |p: &std::path::Path| -> TwoLayerPolicyFile {
        serde_json::from_slice(&std::fs::read(p).unwrap_or_else(|e| panic!("{}: {e}", p.display())))
            .unwrap_or_else(|e| panic!("{}: {e}", p.display()))
    };
    let file = read(std::path::Path::new(start));
    let final_path = run.join("centres").join(format!("final-{lineage}.json"));
    let fin = std::path::Path::new(&final_path)
        .exists()
        .then(|| read(&final_path));
    let layout = file.layout();
    let perturber = Perturber::two_layer(&layout, stage);
    let top = layout.range(cubarium_search::es::voxel::two_layer::Layer::Top);

    let text = std::fs::read_to_string(run.join("generations.jsonl")).expect("generations.jsonl");
    let mut gens: Vec<(u32, Vec<f64>)> = Vec::new();
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        let v: serde_json::Value = serde_json::from_str(line).expect("a generation record");
        let g = v["generation"].as_u64().expect("generation") as u32;
        let l = v["lineages"]
            .as_array()
            .expect("lineages")
            .iter()
            .find(|l| {
                l["lineage"]
                    .as_str()
                    .is_some_and(|n| n.eq_ignore_ascii_case(lineage))
            })
            .unwrap_or_else(|| panic!("generation {g}: no {lineage} lineage"));
        let scores: Vec<f64> = l["candidate_scores"]
            .as_array()
            .expect("candidate_scores")
            .iter()
            .map(|x| x.as_f64().unwrap_or(f64::NAN))
            .collect();
        gens.push((g, scores));
    }
    let pairs = gens[0].1.len() / 2;
    let dim = file.theta.len();

    // Each generation's perturbations, once (the top only at stage 1).
    let eps: Vec<Vec<Vec<f64>>> = gens
        .iter()
        .map(|(g, _)| {
            (0..pairs)
                .map(|p| {
                    let mut e = vec![0.0; dim];
                    perturber.eps(train_seed, *g, p, &mut e);
                    e[top.clone()].to_vec()
                })
                .collect()
        })
        .collect();
    let grad = |gi: usize, scores: &[f64]| -> Vec<f64> {
        let plus: Vec<f64> = (0..pairs).map(|p| scores[2 * p]).collect();
        let minus: Vec<f64> = (0..pairs).map(|p| scores[2 * p + 1]).collect();
        optimizer::gradient(&plus, &minus, top.len(), sigma, |p, out| {
            out.copy_from_slice(&eps[gi][p])
        })
    };

    // The replay.
    let mut theta = file.theta.clone();
    let mut adam = Adam::new(dim);
    adam.learning_rate = lr;
    let mut gs = Vec::new();
    let mut steps = Vec::new();
    let mut walked = 0.0;
    for (gi, (g, scores)) in gens.iter().enumerate() {
        let before = theta[top.clone()].to_vec();
        let mut full = vec![0.0; dim];
        let gt = grad(gi, scores);
        full[top.clone()].copy_from_slice(&gt);
        adam.ascend(&mut theta, &full);
        let step: Vec<f64> = theta[top.clone()]
            .iter()
            .zip(&before)
            .map(|(a, b)| a - b)
            .collect();
        walked += dot(&step, &step).sqrt();
        println!(
            "{{\"generation\":{g},\"gradient_norm\":{:.6},\"step_norm\":{:.6}}}",
            dot(&gt, &gt).sqrt(),
            dot(&step, &step).sqrt()
        );
        gs.push(gt);
        steps.push(step);
    }
    let disp: Vec<f64> = theta[top.clone()]
        .iter()
        .zip(&file.theta[top.clone()])
        .map(|(a, b)| a - b)
        .collect();
    let exact = fin.as_ref().map(|f| f.theta == theta);
    let max_err = fin.as_ref().map(|f| {
        f.theta
            .iter()
            .zip(&theta)
            .map(|(a, b)| (a - b).abs())
            .fold(0.0, f64::max)
    });
    let top_scale =
        (dot(&file.theta[top.clone()], &file.theta[top.clone()]) / top.len() as f64).sqrt();

    let observed = mean_pair_cos(&gs);
    let consecutive =
        gs.windows(2).map(|w| cos(&w[0], &w[1])).sum::<f64>() / (gs.len() - 1).max(1) as f64;
    let toward_end = gs.iter().map(|g| cos(g, &disp)).sum::<f64>() / gs.len() as f64;
    let first_half: Vec<f64> = {
        let mut v = vec![0.0; top.len()];
        for s in &steps[..steps.len() / 2] {
            for (a, b) in v.iter_mut().zip(s) {
                *a += b;
            }
        }
        v
    };
    let second_half: Vec<f64> = {
        let mut v = vec![0.0; top.len()];
        for s in &steps[steps.len() / 2..] {
            for (a, b) in v.iter_mut().zip(s) {
                *a += b;
            }
        }
        v
    };

    // The shuffle null (a small xorshift; reproducible, nothing to do with training draws).
    let mut x: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut rnd = |n: usize| {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        (x % n as u64) as usize
    };
    let mut null = Vec::with_capacity(shuffles);
    let mut null_end = Vec::with_capacity(shuffles);
    for _ in 0..shuffles {
        let mut th = file.theta.clone();
        let mut ad = Adam::new(dim);
        ad.learning_rate = lr;
        let mut ng = Vec::new();
        for (gi, (_, scores)) in gens.iter().enumerate() {
            let mut s = scores.clone();
            for i in (1..s.len()).rev() {
                let j = rnd(i + 1);
                s.swap(i, j);
            }
            let gt = grad(gi, &s);
            let mut full = vec![0.0; dim];
            full[top.clone()].copy_from_slice(&gt);
            ad.ascend(&mut th, &full);
            ng.push(gt);
        }
        null.push(mean_pair_cos(&ng));
        let d: Vec<f64> = th[top.clone()]
            .iter()
            .zip(&file.theta[top.clone()])
            .map(|(a, b)| a - b)
            .collect();
        null_end.push(dot(&d, &d).sqrt());
    }
    let stats = |v: &[f64]| {
        let m = v.iter().sum::<f64>() / v.len().max(1) as f64;
        let sd = (v.iter().map(|x| (x - m).powi(2)).sum::<f64>() / v.len().max(1) as f64).sqrt();
        (m, sd)
    };
    let (nm, nsd) = stats(&null);
    let (em, esd) = stats(&null_end);
    let above = null.iter().filter(|&&c| c >= observed).count();
    println!(
        "{{\"summary\":\"{lineage}\",\"generations\":{},\"pairs\":{pairs},\"top_params\":{},\"replay_matches_final\":{},\"replay_max_abs_err\":{},\
\"top_rms_start\":{top_scale:.6},\"displacement_norm\":{:.6},\"displacement_rms\":{:.6},\"walked\":{walked:.6},\"straightness\":{:.4},\
\"mean_pair_cos\":{observed:.5},\"consecutive_cos\":{consecutive:.5},\"mean_cos_to_end\":{toward_end:.5},\"half_vs_half_cos\":{:.4},\
\"null_mean_pair_cos\":{nm:.5},\"null_sd\":{nsd:.5},\"null_at_or_above\":{above},\"shuffles\":{shuffles},\
\"null_displacement_norm\":{em:.6},\"null_displacement_sd\":{esd:.6}}}",
        gens.len(),
        top.len(),
        exact.map_or("null".into(), |b| b.to_string()),
        max_err.map_or("null".into(), |e| format!("{e:e}")),
        dot(&disp, &disp).sqrt(),
        (dot(&disp, &disp) / top.len() as f64).sqrt(),
        dot(&disp, &disp).sqrt() / walked.max(1e-300),
        cos(&first_half, &second_half),
    );
}
