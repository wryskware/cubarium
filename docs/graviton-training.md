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

## Running today's study tools on Graviton

Prepared 2026-10-08; this package made **no AWS calls and launched no hosts**.
`graviton-studies.py` uses one independent named job per host. `batch` and `eco`
run their census children through host-local `run-study.py` slots; wrapping the
batch dispatcher would incorrectly hide the other L3 groups. `live` wraps the
whole trainer in slot 0, with at most 16 threads. Every job is detached, nice 19,
has a 64 GiB address-space ceiling and a wall deadline. Censuses use one thread,
at most 57 workers on 64 vCPUs, and batch.py further limits concurrency to 80% of
MemAvailable / `--mem-per-run-gb` (default 3 GiB). Measure mature-world peak memory
and throughput with vitals/checkpoints before filling the queue; 57 is a ceiling.

Prerequisites: Python 3.11+ (including tomllib), rsync, SSH key, ARM binaries from
this build, and validated inputs. AL2023 bootstrap installs Python/rsync/util-linux.
AWS login is needed only for `up`, `status`, `down`; `plan` and `kit` are offline.
Remote commands use `python3.11`, not AL2023's potentially older `python3` default.
The existing recipe was **native compilation via `aws-graviton.sh ship`**, not a
local cross recipe. This preparation cross-builds locally with Zig 0.13's C/linker
and `rustup target add aarch64-unknown-linux-gnu`, `cargo build --locked --release
--target aarch64-unknown-linux-gnu -p cubarium-search`, then `-p cubarium --example
voxel_census`. The compiler wrapper drops cc-rs's redundant `--target=` and Zig's
unsupported `-Wl,--fix-cortex-a53-843419`; Rust's target is unchanged. The binaries
land in the normal repository cache at `target/aarch64-graviton2/`.

The JSON examples are job specifications, **not chosen ecological experiments**.
The eco example is 16 candidates × 8 seeds plus 8 baseline worlds: 136 evaluations,
3 CPU waves at 57 workers, plus holdouts. `screen-2h` requires **mature saved starts**:
prepare `runs/graviton-starts/s1` … `s16` as actual checkpoint directories (each
has minute/world.bin/flora.bin/fauna.bin), retaining founding census/roster evidence
where needed. Put them in the kit with `--input runs/graviton-starts`; run `plan`
with that same option. The paired-batch example uses fresh configs and identical
base/candidate arms as a launcher smoke template: supply the intended candidate's
`set=` arguments before a real study. Its two hosts each evaluate 2 arms × 8 seeds.

```sh
G=scripts/graviton-studies.py
S=config/training/graviton-eco.json # or graviton-batches.json
python3 "$G" plan --spec "$S" --price-per-host-hour 0.75 --hours 12 --worlds-per-worker-hour 2.4 --input runs/graviton-starts
python3 "$G" kit --bin-dir /home/wrysk/wryskware/cubarium/target/aarch64-graviton2 --input runs/graviton-starts
# Only AFTER Wrysk's go for this specific launch:
python3 "$G" up --spec "$S" --hours 12 --go
scripts/aws-graviton.sh status # obtain IPs; no automated sleep/poll loop
python3 "$G" ship --spec "$S" --host ec2-user@IP1 # add --host ec2-user@IP2 for two jobs
python3 "$G" run --spec "$S" --host ec2-user@IP1 --hours 12
python3 "$G" fetch --spec "$S" --host ec2-user@IP1 --dest runs/graviton-fetch
python3 "$G" down # ALWAYS, even after a failed run or fetch
```

For batches omit the mature-start `--input`. For custom jobs write a JSON `jobs`
list with `name`, `kind` (`batch`, `eco`, `live`) and an `args` string array. The
launcher owns output, binary, threads, concurrency, detach, resume and wall limits;
batch's `--hours` is the simulated horizon, separate from host `--hours`.
A live spec supplies `--centre LINEAGE=config/...json`, `--world config/...toml`,
`--worlds`, `--pairs`, `--generations` and any other training options. Additional
policies/starts outside config must be kit `--input` paths inside this checkout.
Configs and tools are copied and hashed along with both ARM ELF binaries.
Never ship a changed kit over a running job. On-host log/PID: `runs/NAME.log`,
`runs/NAME.pid`; output `runs/NAME/`, with `HOST_EXIT` recording the process exit.
Inspect STATE.tsv/status/FAILED: a batch DONE only says its children ended.
After launch inspect every thread's `/proc/PID/task/*/status` Cpus_allowed_list:
batch.py verifies its census children; live trainer threads must stay in slot 0's
reported L3 group. Check existing host load before starting another campaign.

**Interruption recovery: fetch → relaunch elsewhere → resume.** Fetch periodically
by explicitly invoking `fetch`, before terminating hosts; recovery uses the last
successful off-host fetch, without a notice or final save. Fetch stages a copy and
checks its committed checkpoint hashes before replacing the previous local copy;
if a checkpoint rolls during transfer, it fails and retains that copy. Fetch again
at a quiet boundary. Only the latest complete checkpoint per census and live
trainer's committed slot are retained; temporary/older checkpoints are excluded.
A run lost before its first checkpoint is restarted; completed and failed censuses
are retained. `eco_search --resume` now continues its interrupted batch instead of
deleting it. Live resume restores optimizer, worlds and familiarity together.

After obtaining Wrysk's go for the replacement launch, `up`, obtain its new IP,
ship the **same kit and inputs**, then:
```sh
python3 "$G" restore --spec "$S" --host ec2-user@NEW_IP --dest runs/graviton-fetch
python3 "$G" run --spec "$S" --host ec2-user@NEW_IP --resume --hours 12
python3 "$G" down # ALWAYS on every launched/replacement host
```
Use the same host order in spec/fetch/restore and the fixed remote root
`/home/ec2-user/cubarium-kit`: manifests contain absolute paths. No path relocation
or cross-version migration is implied. A failed liveness check stays failed for
inspection. A hard kill may lose work since the last checkpoint/fetch.

Offline cost: **hosts × wall hours × user-supplied price/host-hour**, plus storage
and transfer. At an explicitly assumed 2.4 two-hour worlds/worker-hour, 57 workers
mean ≤136.8 worlds/hour and ~75 min per 136-world generation (3 waves), before
setup/holdouts. The two 16-world batch jobs have only 32 occupied worker slots:
≤76.8 worlds/hour total under that same assumption. These are planning assumptions,
not current measurements or price quotes. One host ×12 h ×$0.75 = $9; two = $18.
Wrysk's go is required **per launch**. `up --dry-run` is not an offline plan: the
old helper performs key/security-group setup. Always `down`; it now includes
stopped/stopping as well as pending/running tagged instances and terminates them.
The shutdown timer and delete-on-termination volumes remain secondary guards.
Legacy `aws-graviton.sh run` is retired because it bypassed resource policy.

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
