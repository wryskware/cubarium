---
design_status: exploration
last_reviewed: 2026-10-10
decision_refs: []
---

# labd: one service for every sim, test and build job

Tooling record for `scripts/lab/` (stdlib Python 3.11+). The CLI is `scripts/lab/lab`; the
daemon is `scripts/lab/labd` (the same as `lab daemon`), run by the user unit that
`lab install` writes. Host inventory: `config/lab/hosts.toml`. Tests: `scripts/lab/test_lab.py`
and `scripts/lab/tests/test_remote.py` (localhost remote roots, no eidolon or AWS access).
Run with `timeout 270 env PYTHONPATH=scripts/lab/tests:scripts/lab python3 -m unittest test_lab test_remote`.

Why (Wrysk, 2026-10-10): multi-hour runs were lost to coordination mistakes. An A/B arm ran
on the wrong config, a shared `target/` mixed binaries, a build raced a commit in the main
checkout, and an OOM in tmpfs killed a session's jobs. Separate batches also landed on the
same cores. "launcher daemon ... can also manage things like load balancing, core
assignment, and ec2 instances. all instead of you or me coordinating by memory"; "keep the
daemon using a generic interface so that its easily extensible for all the different kinds
of jobs and tests we create and run."

## Using it

```sh
L=scripts/lab/lab
$L submit job.toml          # validates, pins commits now, spools; prints the job id
$L status                   # jobs, running tasks, hosts and group slots, EC2 requests
$L hosts                    # inventory and load
$L wait --any-new           # the one agent waiter: exits on any job end or approval request
$L wait --job ID            # exits when that job ends (0 done, 2 aborted/cancelled)
$L cancel ID
$L approve ec2 REQUEST --max-hours H --max-usd D [--type c6g.16xlarge]
$L events -n 50             # tail of events.jsonl
$L kinds                    # registered kinds
$L install                  # writes ~/.config/systemd/user/labd.service, enables, starts
```

Submission works while the daemon is down: `submit`, `cancel` and `approve` write JSON
files into `incoming/` and `control/` under the state dir, and the daemon applies them on
its next tick. A TOML or JSON spec dropped into `incoming/` by hand also works. Its
commits are then pinned when the daemon ingests it, not when the file was written.

A spec is a short table. `kind` picks the plugin. `name`, `priority` and `out` (default
`runs/lab/<job-id>/`) are common. Everything else belongs to the kind. Relative paths are
resolved from the submitter's working directory.

### Job stop conditions

Every kind can use `[[stop]]` rules. The core checks local census files at task end
and at the progress cadence, after remote outputs have been mirrored. For example,
an A/B boom guard checks the selected arm after 30 simulation minutes:

```toml
[[stop]]
when = "founders_littershredder > 300"
after_minute = 30
arms = ["branch"]
scope = "any"
action = "abort"
reason = "shredder boom"
```

`when` accepts census column names, numbers, `+ - * /`, comparisons (including
chained comparisons), `and`, `or`, `not`, and parentheses. It uses a small AST
interpreter with no `eval`. `delta(col, minutes)` subtracts the last sample at or
before that many simulation minutes ago from the latest sample, including prior
stages. For A/B jobs, `arm(name, col)` reads the latest value in another arm for
the same seed: `count - arm(main, count) > 10`. Arm and column arguments can be identifiers
or quoted strings. Census time is `sim_min` (`minute` is also supported). Missing
runs, peer outputs, or insufficient delta history do not match. Incomplete final
CSV lines are ignored until the writer completes them.

Defaults are `after_minute = 0`, `scope = "any"`, `action = "abort"`, and all arms.
`scope = "all"` requires every expected run in the selected arms to match,
including runs that have not started. `arms` is only available on A/B expression
rules. Unknown columns are rejected at submission when an explicit output or
resume directory already exposes a header; otherwise they produce a logged
`stop-error` and abort visibly on first evaluation. Referenced columns are checked
even when a boolean expression would short-circuit.

A question answered guard can keep its partial results and run the kind's summary:

```toml
[[stop]]
when = "seedporter_shredder_catches > 0"
after_minute = 45
scope = "any"
action = "finish"
reason = "question answered: seedporters catch shredders"
```

Use the actual counter name in the study's census header. For checks involving
several files or a custom analysis, use an executable local hook instead:

```toml
[[stop]]
hook = "scripts/studies/stops/answered.py"
every_min = 10
```

The hook path resolves from the submitter's working directory. It gets the job
output directory as `argv[1]` and prints exactly one JSON line, for example
`{"stop": true, "action": "finish", "reason": "question answered"}`. A Python
hook needs an executable bit and a Python shebang. Hooks run locally in a worker
thread, read mirrored outputs, and start at the first progress check, then at most
once per `every_min` wall minutes (default 10; timestamps persist across daemon
restarts). Nonzero exits, invalid replies, and the 60-second timeout are logged as
`stop-hook-error` and treated as continue. Hooks do not block the scheduler.

The first firing rule emits `stopped` with its action and reason. Both actions
cancel running tasks through the usual graceful cancellation path and skip
pending tasks. `abort` ends as aborted; `finish` ends as done and calls the kind's
`finish` on the retained outputs, after collecting stopped remote tasks. It never
spawns further stages. The reason goes into `DONE`, `summary.txt`, and the stored
summary. `lab status` and `lab top` show rules and the final reason. A/B's existing
identical-results tripwire remains in place.

## The kind interface

`labkit/core.py`. A kind is one module in `labkit/kinds/`, or in a directory on
`LAB_KIND_PATH` / `[lab] kind_paths`, that defines a `Kind` subclass with a unique `name`.
Discovery registers it. The scheduler and the CLI never change for a new kind.

| Method | When | Returns |
|---|---|---|
| `validate(spec, ctx)` | submit (CLI) and ingest | spec with defaults; commits pinned (`ctx.resolve_commit`) |
| `plan(job, ctx)` | once, after ingest | `[Task]` |
| `inputs(job, ctx)` | before preflight (optional) | artifact keys needed beyond the tasks' own `inputs` |
| `preflight(job, ctx)` | once every input exists, before any task starts; in a worker thread; seconds-long | `None` or `Abort(reason)` |
| `progress(job, task, ctx)` | each task end (`task` set), and every `progress_s` (`task` None) | `None`, `Abort(reason)` or `Spawn([Task])` |
| `census_runs(job, ctx)` | generic stop checks | expected `CensusRun(arm, seed, paths)` entries; paths are local, chronological stage files |
| `finish(job, ctx)` | no task pending or running | summary text; the core writes `summary.txt` and `DONE` |

By default `progress` aborts the job when a task does not end `done`, and `finish` counts
task states. `validate` takes `ctx`, unlike the brief's sketch, because pinning a commit
(a `git stash create` for a dirty tree) has to happen at submit time. `inputs` is an
addition: a preflight-only job such as `ab` can need binaries without having any tasks.
The default `census_runs` declares one `census.csv` per task. A/B groups stage files
by arm and seed; batch declares all runs from the mirrored `batch.json` manifest,
including queued runs. Kinds with another layout override only this declaration;
expression parsing, hooks, cancellation, and reporting stay in the core.

`job` gives `id`, `spec`, `out`, `state` (a kind-private dict, persisted after every hook),
`tasks` (name → view with `task`, `state`, `rc`, `reason`, `host`, `out`) and `inputs`
(key → path, set for preflight).

**Task** (a dataclass; every field but `name` and `argv` is optional):

| Field | Meaning |
|---|---|
| `name`, `argv` | identifier unique in the job; the command. Placeholders: `{out}`, `{job}`, `{input:KEY}`, `{python}`, `{lab}` (the tools dir on the execution host) |
| `threads`, `mem_gb` | placement charge |
| `l3` | must run inside one shared-L3 group; its affinity becomes that group (run-study semantics) |
| `arch`, `hosts`, `local_only` | constraints. `hosts=None` means non-EC2 hosts; EC2 costs money, so a job opts in by naming `"ec2"` |
| `inputs` | artifact keys, e.g. `binary:<commit>:<arch>:voxel_census` |
| `files` | local file/directory → host destination, with placeholders; copied on remote launch and its paths substituted in argv; destinations stay inside the remote job root |
| `remote_argv` | optional argv for remote execution, used by cargo-test's source-archive path |
| `outputs` | directory, default `<job>/<name>`; `stdout`/`stderr` default `{out}/stdout.log`, `{out}/stderr.log` |
| `done_marker` | relative to `outputs`; exit 0 without it counts as failure |
| `deps`, `priority` | ordering within and across jobs |
| `liveness` | `{stall_min, paths: [globs]}` (output growth) or `{stall_min, minute_file}` (census minute); a stalled task is stopped and reported |
| `timeout_h` | systemd `RuntimeMaxSec` |
| `est_h` | wall estimate for ETAs and the EC2 estimate |
| `retries`, `resume_argv` | after host loss (spot reclaim, a dead unit), requeue up to `retries` times with `resume_argv` |
| `produces` | `{key: path relative to outputs}`; on success the core copies the file into the artifact cache |
| `hidden` | wrap in `scripts/hidden.sh` (private Xvfb), local host only |
| `rlimit_as_gb`, `cwd`, `env`, `meta` | address-space limit (the training ceiling), working dir, extra env, kind-private data |

**ctx** (`Context`) offers:
- `resolve_commit(tree, ref)`: an exact sha. A dirty tree becomes a `git stash create` commit, protected by `refs/labd/snap/<sha>` and flagged `dirty`; untracked files are not included.
- `binary(tree, commit, name, ...)`: an artifact key that also records how to build it.
- `artifact(key)`, `path(spec, value)`, `event(name, ...)`.
- `probe_prefix(slot)`: run-study slot plus idle priority, for seconds-long probes.
- `l3_groups()`, `output_signature(globs)`, plus `repo`, `python`, `lab_dir` and `run_study`.

**Artifacts and providers.** When a job needs a key that is not cached, the core asks a
provider (`labkit/providers.py`, or a `PROVIDERS` dict in a kind module) for a job spec
that produces it. It submits that job once, and dedups across waiting jobs. `binary:`
becomes a `build` job and `source:` a `git archive` command job. A failed provider aborts
every job that waits on it.

### Writing a kind

```python
from labkit.core import Kind, Task, Abort, Spawn

class Sweep(Kind):
    name = "sweep"
    def validate(self, spec, ctx):
        spec.setdefault("values", [1, 2])
        spec["binary"] = ctx.resolve_commit(ctx.path(spec, "."))   # pinned now
        return spec
    def plan(self, job, ctx):
        key = ctx.binary(job.spec["binary"]["tree"], job.spec["binary"]["commit"], "voxel_census")
        return [Task(name=f"v{v}", argv=["{input:%s}" % key, "1", f"set=x={v}", "run={out}/run"],
                     inputs=[key], l3=True, liveness=dict(minute_file="run/status", stall_min=30))
                for v in job.spec["values"]]
```

The test file defines one such kind (`test-relay`, with a preflight and a `Spawn`) and runs it
end to end with no core edit.

## v1 kinds

- **command**: any headless argv with resources, the escape hatch. `binary = {commit,
  name}` adds a cached build as `{bin}`. A windowed `--gpu-target window` is refused unless
  `hidden = true`.
- **build**: `cargo build` of one target at one commit, cached as `binary:<commit>:<arch>:<target>`.
  It runs `labkit/snaprun.py` as a task. Other arches build natively on their host from a
  `source:<commit>` archive (EC2; untested against AWS).
- **cargo-test**: a suite (default `cargo nextest run --workspace --exclude cubarium-gpu`)
  in a snapshot tree, on desktop or eidolon. Remotes use the same `source:<commit>` archive
  provider as cross-arch builds, unpack into `<root>/trees/<sha16>`, and keep a warm
  `<root>/target` across commits/suites. The exit status is the verdict; logs are fetched.
  Same-arch binary builds still run locally and populate the shared artifact cache.
- **ab**: `scripts/studies/ab.py`'s guards, imported. Each arm is pinned at submit (tree,
  commit, optional `bin`, otherwise a cached build). Preflight is `ab.arm_inputs` in pinned
  mode, so configs come only from the arm's tree at its commit, followed by
  `ab.preflight`: the resolved `expand=`/`params=` diff, `expect`, and the identical-arm
  refusal. Config includes/path inputs are materialized from the same arm's pinned commit,
  preserving their relative layout. The early stage is one L3 task per arm and seed,
  eligible for desktop or eidolon. Remotes receive the cached binary (or supplied binary),
  that arm's config bundle, and only its own load checkpoint. `ab.early_read` is the
  identical-results tripwire. Continuations load only their own checkpoints, after
  `ab.unchanged`. `ab.py` stays a working CLI.
- **batch**: `scripts/studies/batch.py` as one L3 task, charged `parallel × threads`,
  on desktop or eidolon. Tools, binaries, config dependencies, saved starts and their
  founding census, required rosters, policies and genomes are staged through the same
  transport. A cached binary's config comes from its pinned tree. Its
  run-study slots wrap inside the CPU set labd gives it. It keeps policies, starts and
  `--resume`. A lost batch is retried once with `--resume`, and `resume = DIR` continues an
  old one. Outputs live in `batch-task/batch`, with wrapper logs in `batch-task`;
  saved manifest commands and input paths are remapped when resuming on another host.

## Placement

`labkit/hosts.py`. Per host, usable threads = `min(floor(cpus × max_cpu_fraction), cpus −
reserve_cpus)`, and each L3 group gets its share of those threads in proportion to its
size. Running tasks across **all** jobs are charged. An `l3` task goes to the first
preferred group with room, and its affinity is that group's CPUs. Other tasks float, and
their threads count against every group in proportion. Memory is checked against
`max_mem_gb`, and on the local host against live MemAvailable minus `reserve_mem_gb`.
With `external_load`, the load labd does not own (loadavg minus labd's threads) is
charged too: on the local host every tick, on a remote host at each probe. A task that
cannot fit on any host even when idle aborts its job at plan time. Otherwise it waits
(priority, then FIFO, with backfill).
Among eligible hosts that fit, prefer the most free threads: usable threads minus all
running labd tasks and external load. `hosts = ["desktop"]` or `["eidolon"]` pins run/test
tasks; without a pin every enabled non-EC2 host is eligible. EC2 remains opt-in and approved.

- Desktop: SCHED_IDLE plus nice 19 for every task (the effect of `chrt -i 0 nice -n 19`),
  group 1 preferred over the V-cache group 0, and 4 CPUs plus 12 GB reserved for Wrysk.
- Remote hosts report their own L3 groups and load, from their own sysfs. Desktop CPU
  numbers never go to eidolon: a task carries a group index, and `taskwrap.py` resolves it
  on the host.

## Execution, restart, reconcile

Every task runs `labkit/taskwrap.py LAUNCH.json`, which:
- removes `DISPLAY`, `WAYLAND_SOCKET` and `HYPRLAND_INSTANCE_SIGNATURE`;
- sets `WAYLAND_DISPLAY` to a socket that does not exist, because an unset value falls back
  to `wayland-0`;
- sets `CLAUDECODE=1`, so the cubarium binary refuses windows;
- applies affinity, idle priority and the rlimit, then runs the child;
- verifies its threads' affinity after two seconds;
- writes `rc.json` atomically when the child ends.

Launchers:
- **Local:** `systemd-run --user --collect --unit=lab-<job>-<task>`, with `MemoryMax` set
  to the host ceiling and `RuntimeMaxSec` from `timeout_h`. Restarting labd never touches
  a task, and an OOM stays in that one unit.
- **Eidolon:** probed 2026-10-10. `systemd-run --user` works there, but `Linger=no`, so the
  user manager and its units stop with the last session. `launcher = "auto"` therefore uses
  setsid plus pid files there, and the `host-up` event says so. Enabling linger on eidolon
  (`loginctl enable-linger`) is Wrysk's call; after that `auto` picks systemd.
- **Remote hosts:** scripts go through `ssh HOST 'bash -s'`, so fish login shells are fine.
  Tools and artifacts are copied under `root` (`~/cubarium-train/lab`), and outputs are
  incrementally rsynced back at each remote task probe, before progress/liveness hooks read
  local census, stderr, vitals and checkpoints. At task end a full fetch includes wrapper
  logs/status; a fetch failure leaves the task and remote copy available for retry.
  Only after a successful fetch and persisted task result are remote task outputs removed.
  Shared job inputs are removed when all tasks on that host have fetched their results.
  `taskwrap.py` resolves the L3 group from that host's sysfs and applies idle class/nice 19.

On start the daemon:
- resets jobs that were mid-preflight back to `inputs` (preflights are seconds-long);
- probes hosts;
- polls every running task: `rc.json` means ended, a live unit or process means still
  running, and neither means `lost` (requeued if retries remain).

`lab status` reads sqlite directly, so it works with the daemon down.

**Builds never touch a live checkout.** The snapshot is a detached worktree at the exact
commit under `~/.cache/labd/trees/<sha16>`. Its `target/<profile>` is seeded once by
`cp -a --reflink=always` from the newest candidate: other trees or the configured
`seed_targets`. If the reflink fails, the build runs cold; it never falls back to a full
copy. `/tmp` and tmpfs are refused for the state, cache, trees and outputs. A janitor keeps
the newest `keep_trees` trees. A commit made in the source checkout during a build cannot
reach it (tested).

## State model

`~/.local/state/labd/` (`$LABD_STATE`):

- `labd.sqlite` (WAL) with these tables:
  - `jobs`: `id`, `kind`, `state`, `spec`, `kstate`, `out`, `reason`, `summary`, times;
  - `tasks`: `job`, `name`, `state`, `spec`, `host`, `grp`, `unit`, `launcher`, `rc`, `reason`, `attempts`, `live`;
  - `artifacts`: `key`, `path`, `sha256`, `meta`;
  - `recipes` and `providers`;
  - `approvals`;
  - `kv`: EC2 state, host probes, the event sequence.
- `events.jsonl`, `incoming/`, `control/`, `rejected/` and `labd.lock` (one daemon per
  state dir).
- `~/.cache/labd/` (`$LABD_CACHE`) holds `artifacts/` and `trees/`.
- Job output goes to `runs/lab/<job-id>/` (`$LABD_RUNS`): the kind's files, `.lab/<task>/`
  (`launch.json`, `pid.json`, `rc.json`, `wrap.log`), `summary.txt` and `DONE` (the reason).

Job states: `submitted → inputs → preflight → running → done`, with `aborting →
aborted|cancelled` on an abort or cancel. A bad spec becomes `rejected`, and a kind error
turns into an abort with `lab-error.log`. Task states: `pending → running → done | failed |
cancelled | stalled | lost`, or `skipped`.

## Event schema

One JSON object per line, appended by the daemon only:
`{"seq": N, "ts": "2026-10-10T07:07:55", "t": epoch, "event": NAME, "job"?, "task"?, "host"?, "reason"?, ...}`.

| Event | Extra fields |
|---|---|
| `daemon-start` / `daemon-stop` | `pid`, `running_tasks` |
| `submitted` / `rejected` | `kind`, `name`, `parent` (a provider job) / `reason` |
| `planned`, `input-wait`, `artifact` | `tasks`; `key`, `provider`; `key`, `path` |
| `preflight-ok` / `preflight-abort` | `reason` |
| `started` | `task`, `host`, `group`, `unit`, `launcher` |
| `task-done` / `task-failed` / `task-lost` / `task-stalled` / `requeued` / `spawned` | `task`, `rc`, `reason` |
| `aborting`, then `done` / `aborted` / `cancelled` | `reason` (the DONE text), `out` |
| `host-up` / `host-down` | `host`, `launcher`, `detail`; for EC2 `instance`, `hours`, `est_usd` |
| `approval-needed` | `approval`, `jobs`, `instance_type`, `price_usd_h`, `local_eta_h`, `ec2_eta_h`, `est_usd`, `command` |
| `approval-granted` / `approval-withdrawn` | `instance_type`, `max_hours`, `max_usd` |
| `ec2-launching`, `ec2-limit`, `ec2-auth-expired`, `ec2-down-failed`, `ec2-error`, `ec2-orphan` | details |
| `control-error`, `tree-pruned` | |

`lab wait --any-new` wakes on the terminal job events, `approval-needed`,
`ec2-auth-expired`, `ec2-down-failed` and `ec2-limit`. It reads the file from its own
offset every `--interval` seconds (default 20). The standard library has no inotify, and a
sleep loop costs nothing.

## EC2 approval flow

`labkit/ec2.py`, driving `scripts/aws-graviton.sh up|status|down`.

1. **idle → approval-needed.** This happens when pending tasks may run on EC2 and either
   only EC2 fits (a job names `ec2`, or the arch is aarch64), or the local queue estimate
   exceeds `suggest_after_h` and the EC2 estimate is shorter. The estimate is
   `max(longest task, thread-hours / capacity)` plus `setup_h`. The cost is that estimate
   × the assumed spot price in `hosts.toml`. One request opens with
   `lab approve ec2 <id> --max-hours H --max-usd D`, and labd never calls `up` without
   it. The request is withdrawn if the eligible tasks go away.
2. **Approve → launching.** labd runs `up -n 1 -t TYPE --hours ceil(H)`, so the host's own
   shutdown timer backs up the limit. One approval buys one launch.
3. **launching → up.** Once `status` shows the instance running with an IP and the host
   is bootstrapped, the instance becomes the schedulable host `ec2` over ssh (key
   `~/.ssh/cubarium-aws.pem`, `python3.11`, setsid launcher).
4. **While up:**
   - When elapsed hours reach H, or hours × price reach D, labd emits `ec2-limit` and runs
     `down`. The tasks there become lost or requeued.
   - When the queue drains (nothing eligible pending, nothing running there), it runs
     `down`.
   - On spot loss (`status` no longer lists the instance), it emits `host-down`, requeues
     the lost tasks with `resume_argv` from fetched checkpoints, and opens a new request.
     A replacement needs its own go.
5. **AWS login problems are reported, never "fixed."** If `up` fails on an expired or
   missing login, labd emits `ec2-auth-expired` and moves to `blocked-auth`; a fresh
   approve is needed. If `down` fails, labd emits `ec2-down-failed` (the instance may be
   up) and retries every minute. labd never touches keys or credentials (the 2026-10-02
   incident truncated the key).
6. **At start**, labd resumes an instance it launched. It reports tagged instances it did
   not launch (`ec2-orphan`) and never kills them.

v1 is tested only against a mocked `aws-graviton.sh` and a local stand-in for the host. No
instance was launched. The first real launch should be watched end to end: build, task,
fetch, down.

## Later kinds

Each is one module; none needs a core change.

- **ES training** (`voxel-train`, `voxel-live-train`): one task with `threads ≤ 16` (the
  measured cap) and `rlimit_as_gb = 64`. It can float, because independent episode
  training is exempt from L3 confinement. Liveness comes from its checkpoint slot (a
  `paths` glob). `resume_argv` uses `--resume` with `retries ≥ 1`, so a lost host or spot
  reclaim continues. `progress` can read generation stats and `Abort` on a tripwire (for
  example, every lineage starving). The binary is `binary:<commit>:<arch>:cubarium-search`
  (recipe already in `build`). `hosts = ["ec2"]` turns on the approval flow, and the build
  then runs natively on the host.
- **eco_search map**: `plan` expands a generation into one L3 task per (candidate, seed),
  each with `est_h`. `progress` with `task=None` gathers finished censuses, scores a
  generation when its tasks are all done, and `Spawn`s the next generation. A staged
  liveness check after the first segment stops a broken map early. `eco_search --resume`
  maps to `resume_argv`.
- **library** (saved-start libraries): one founding task per seed. Each `produces`
  `start:<map>:<seed>:<commit>` keys, so other jobs name those keys as `inputs` and the
  library is built once. A provider in the kind module (`PROVIDERS = {"start": ...}`)
  makes a missing start trigger its founding job.
- **captures** (`--gpu-target headless`, `--gpu-capture DIR`, `voxel_specimens --gpu`):
  one `local_only` task, `hidden = true` only when a window is truly needed. `done_marker`
  points at the last frame or the MP4. `finish` returns the MP4's absolute path for the
  summary.

## Not in v1

These remain open:
- a real EC2 launch;
- mechanism-specific remote liveness beyond the existing minute/output-growth probes;
- inotify for `lab wait`.
