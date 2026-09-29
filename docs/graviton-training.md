# Graviton offload for training

Status 2026-09-28: **mode A is set up and was checked end to end** (up, ship, run, fetch,
down on one c7g.16xlarge, about $0.04). No training job has been run on AWS yet.
Wrysk's choices: mode A (the whole trainer runs on the AWS host), `c7g.16xlarge` on spot,
`c6g.16xlarge` as the fallback.

## What "spot" means

AWS sells its spare capacity at a steep discount (here about 70 % off: c7g.16xlarge is
about $0.70/h against $2.32/h on-demand). The price is the same kind of machine; the catch is
that AWS can take it back with two minutes' warning when it needs the capacity. A reclaimed
host is terminated and everything on it is lost. So: spot for runs that checkpoint (or that
you can afford to rerun), on-demand only if a run must not be interrupted. The account may run
two spot 64-core hosts at once (128 vCPU quota) or one on-demand one (64 vCPU).

## Runbook (next training session)

Prerequisites: `aws` logged in (it is), region us-east-1. Everything is `scripts/aws-graviton.sh`.

```sh
scripts/aws-graviton.sh up                 # c7g.16xlarge spot; falls back to c6g.16xlarge
scripts/aws-graviton.sh status             # wait for "running"
scripts/aws-graviton.sh ship               # native build on host 1 (~1.5 min), saves binary locally
scripts/aws-graviton.sh put runs/x/centre.json config/foo.toml   # inputs untracked by git
scripts/aws-graviton.sh run 1 mytrain -- voxel-train ...         # nohup, all cores, log on host
scripts/aws-graviton.sh logs 1 mytrain 40
scripts/aws-graviton.sh fetch 1            # pull ~/cubarium/runs/ into runs/aws-<ip>/
scripts/aws-graviton.sh down               # ALWAYS, when done or the run has died
```

- `up -n 2` launches two hosts (a second host is a second independent run: mode A does not
  split one run across hosts). `up -t c6g.16xlarge` forces a type; `--ondemand` skips spot;
  `--hours H` sets the self-terminate timer (default 12; the host powers off and is deleted).
- `ship` sends the tracked files as they are on disk, uncommitted edits included, so the AWS
  build matches your working tree. The host builds natively; the stripped binary is saved to
  `target/aarch64-graviton2/cubarium-search` and lands on the host at
  `~/cubarium-train/cubarium-search` (and `~/cubarium/target/release/cubarium-search`).
- `run` starts `cubarium-search ARGS` from `~/cubarium` (so repo-relative `config/...` paths
  work), with `CUBARIUM_WORKER_RESERVE=0`, so all 64 cores are workers. Untracked inputs
  (warm-start centres under `runs/`) must be `put` first. Write outputs under `runs/` so
  `fetch` finds them.
- Cost guards: the self-terminate timer, spot by default, ssh from your current IP only, no
  IAM role on the hosts, EBS deleted on terminate. A forgotten host costs about $17/day on
  spot, $52/day on-demand: run `status` at the end of a session.
- **Interruptions:** a reclaimed spot host kills the run. Start runs with checkpoints
  (`--save-every` on `voxel-live-train`; check `voxel-train`'s option) and `fetch` periodically;
  otherwise rerun.
- Leftover resources by design: key pair `cubarium-aws` (private key `~/.ssh/cubarium-aws.pem`)
  and security group `cubarium-train-ssh`. Both are free and reused.

Code change made for this (uncommitted at the time of writing, `task.rs`):
`MAX_EPISODE_WORKERS` 32 → 64, and `CUBARIUM_WORKER_RESERVE=N` replaces the ten-percent CPU
reserve with "leave N free". The desktop and the Tachyon leave it unset, so nothing changes
there (28 workers on the 32-thread desktop).

## Checked in the end-to-end run (c7g.16xlarge, spot, us-east-1)

- `up` picked c7g first; a c7g.16xlarge was available.
- Native build on Graviton3: **48 s** (Graviton2 was 64 s). `ship` end to end: 1 m 26 s.
- A default-worker run used **64 workers** with the reserve at 0.
- New and existing worker-limit unit tests pass on aarch64.
- A 4-episode arena smoke gave 197k ticks/s on one worker against 140k on Graviton2: only a
  hint (tiny, noisy), not a benchmark; c7g landscape throughput is still unmeasured.
- The fallback path (c7g unavailable → c6g) was not exercised: c7g had capacity.

Not yet done: mode B (desktop coordinates, AWS workers via `--remote`); it needs a cross-arch
connect check and a fixture-hash test across x86/aarch64 (see the matrix below).

---

## Measurements and options (Graviton2 benchmark, 2026-09-28)

Date 2026-09-28. Region us-east-1. Prices are AWS list prices fetched that day; spot is the
7-day range per zone. Nothing here is committed to a choice.

## What was measured

One spot `c6g.16xlarge` (64 Neoverse-N1 cores, 1 thread per core, 123 GB) in us-east-1b,
eight minutes of life, about $0.10. Terminated, key pair and security group deleted
afterwards (the `up --dry-run` recreated an empty key pair and security group; both free).

| Measurement | Result |
|---|---|
| `cubarium-search` builds on aarch64 with no source change | yes, rustc 1.98.1 |
| Cold build incl. rustup, 64 cores | **1 m 04 s** wall (release, 2.0 GB peak RSS) |
| Stripped binary | 8.3 MB (94 MB with debuginfo); saved at `target/aarch64-graviton2/cubarium-search` |
| Landscape episode, 1 worker (browser, 4,800 ticks) | small 11.7k, default 14.5k, wide 13.5k ticks/s |
| Same at 32 workers, per worker | 10.8k / 13.8k / 12.6k ticks/s (scaling 92–96 %) |
| Aggregate at 32 workers | **345k / 442k / 404k ticks/s** |
| Arena episodes (much lighter) at 32 workers | 3.0–4.0 M ticks/s |
| Process RSS, 32 workers, 48 landscapes founded | 2.3 GB (memory is not the constraint) |
| Full 4,800-tick landscape episode | 0.33–0.41 s of one core |

Command: `cubarium-search voxel-landscapes --founder browser --ticks 4800 --episodes 4 --workers 32 --pin off`.

**Against the desktop:** not measured side by side (the desktop is busy with `lt2`). Recorded
figure from the 2026-09-24 cache study: 305k ticks/s at 32 threads on the P5-C pool, about
9.5k per thread. So a Graviton2 core is roughly equal to or a little better than a 9950X3D
hardware thread on this workload, and **32 Graviton2 cores are worth about the whole
desktop**. Treat that as ±25 %.

## Workload shape (why this offloads well)

Training is embarrassingly parallel: one ES unit is an antithetic pair on one fixture, about
0.3–3 s of one core, and the payload is a few KB of weights each way. Compute-bound, L3 misses
about 0.01 per 1k instructions (cache study), so a 32 MB shared L3 does not hurt. No SIMD or
x86-specific code in the workspace; `libm` transcendentals are the only cross-arch float risk.

## Cost matrix: hardware and market

`$/core-h` = hourly price / vCPUs (Graviton2 vCPU = physical core).

| Option | vCPU / RAM | On-demand $/h | Spot $/h (7 d) | On-demand $/core-h | Spot $/core-h | Notes |
|---|---|---|---|---|---|---|
| **c6g.16xlarge** | 64 / 128 GB | 2.176 | 0.63–0.94 (typ. 0.71) | 0.034 | ~0.011 | Baseline. 1f had **no capacity** at launch time; 1b worked |
| c6g.4xlarge ×4 | 16 / 32 GB each | 0.544 each | 0.17–0.23 each | 0.034 | 0.011–0.014 | Finer grain, more hosts to connect, each founds fixtures itself |
| c6g.metal | 64 / 128 GB | 2.176 | n/a | 0.034 | n/a | Same price as 16xlarge; no reason for us |
| m6g.16xlarge | 64 / 256 GB | 2.464 | 0.61–0.76 | 0.039 | ~0.010 | Spot ≈ c6g spot, double RAM; RAM unneeded (2.3 GB used) |
| c6gd.16xlarge | 64 / 128 GB + NVMe | 2.458 | 0.39–0.63 | 0.038 | 0.007–0.010 | Cheapest spot seen; local disk unused; check capacity |
| r6g.16xlarge | 64 / 512 GB | 3.226 | 0.95–1.98 | 0.050 | 0.015–0.031 | No: memory not needed |
| c6gn.16xlarge | 64 / 128 GB, 100 Gb | 2.765 | not sampled | 0.043 | n/a | No: network not needed |
| t4g.xlarge | 4 / 16 GB | 0.134 | n/a | 0.034 | n/a | No: burstable, throttles under sustained load |
| *Ref:* c7g.16xlarge (Graviton3) | 64 / 128 GB | 2.320 | 0.64–0.75 | 0.036 | ~0.011 | **Same spot price as c6g**, faster core (vendor claim, unmeasured) |
| *Ref:* c8g.16xlarge (Graviton4) | 64 / 128 GB | 2.552 | 0.71–0.91 | 0.040 | ~0.012 | Unmeasured |
| *Ref:* c7a.16xlarge (x86) | 64 / 128 GB | 3.284 | not sampled | 0.051 | n/a | For scale only |
| eidolon.local (owned) | 12 threads | 0 | 0 | 0 | 0 | Unreachable from here at the time of writing |

Ticks per dollar at about 12k ticks/s/core (landscapes): **on-demand 1.3 G ticks/$, spot 3.9 G ticks/$**.

## Cost of a run (estimates; assumes a Graviton2 core ≈ a desktop thread)

From the P5-C figures in `design/handoffs/voxel-training-speed-2026-09-23.md`
(512 updates; browser 28 s/gen on 7 workers, shredder 87 s/gen on 16 workers):

| Run | Core-hours | Wall on 64 cores | On-demand | Spot |
|---|---|---|---|---|
| P5-C browser | ~28 | ~0.4 h (was 4.5 h) | ~$0.95 | ~$0.31 |
| P5-C shredder | ~198 | ~3.1 h (was 12.3 h) | ~$6.70 | ~$2.20 |

Idle time is the real cost risk: a forgotten 64-core on-demand host is $52/day (spot ~$17/day).
`aws-graviton.sh` sets a self-terminate timer (default 12 h).

`voxel-live-train` (the run on the desktop now) was **not benchmarked**; its per-world cost
differs. Same order of magnitude is a guess.

## Quotas and limits (us-east-1)

- Running on-demand standard vCPUs: **64**, so one c6g.16xlarge on-demand.
- All standard spot vCPUs: **128**, so two c6g.16xlarge on spot.
- Increases are free to request (Service Quotas), usually approved within a day.
- No existing instances, volumes or spot requests in the account.

## Matrix: how the work gets there

| Mode | Code change needed | Desktop cores used | Spot-interrupt behaviour | Effort | Notes |
|---|---|---|---|---|---|
| **A. Whole trainer on the AWS host** (`voxel-train`, `voxel-live-train` run there; pull results) | Raise `MAX_EPISODE_WORKERS` (32) in `task.rs` so 64 cores are used, otherwise half idle | 0 | Run dies; resume only if the run saves checkpoints (`--save-every`) | Smallest | No WAN in the loop. Held-out eval runs there too. Best first step |
| **B. Desktop coordinates, AWS hosts as `--remote` workers** | `remote.rs` connect check refuses a worker whose build flags differ (`target=` and features are in the string), so an aarch64 worker is refused by an x86 coordinator. Needs a cross-arch mode. Fixture summaries are "hashed exactly": untested whether libm gives identical founding on aarch64. Same 32-thread cap | Coordinator + held-out only | Dropped remote's units are re-queued locally, generation completes (already implemented) | Medium | Pairs stay whole on one machine, so the gradient tolerates float differences. Needs a fixture-hash test across arch before trusting |
| C. Managed batch (AWS Batch / Fargate Graviton) | Container image, job queue, S3 handoff | 0 | Retries are built in | Large | Overkill at this scale |

## Other considerations

- **Data transfer:** ingress free, egress $0.09/GB. Results and centres are KB to MB. EBS 40 GB gp3 is deleted on terminate.
- **Security:** ssh only, from this machine's current public IP; no IAM role on the hosts; key at `~/.ssh/cubarium-aws.pem`. Note the IP changes if home IP does; `up` re-authorizes the current one.
- **Working policy:** launching hosts spends money; an agent does it only when Wrysk says a run is go (his "no runs until I say go" rule), and always ends with `down`.
- **Build tuning:** the saved binary is a plain release build. A `-C target-cpu=neoverse-n1` build was not tried.
- **Reproducibility:** a Graviton run is a different platform from x86 runs; the project rule is no bit-identical requirement, conservation and qualitative reproducibility only.
