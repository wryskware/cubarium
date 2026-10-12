---
status: open
date: 2026-10-12
---

# Worker rules for the behaviour-AI packages (2026-10-12)

Every behaviour-AI brief dated 2026-10-12 points here. `AGENTS.md` and `WORKING_POLICY.md`
(repo root) are the full rules. `design/handoffs/worker-rules-2026-10-07.md` still holds,
except where this file supersedes it (jobs now go through labd).

- **The design you build from** is `design/behaviour-ai-2026-10-11.md` (`design_status:
  proposal`; Wrysk gave the go to implement it on 2026-10-12). §0 holds his calls in his words;
  don't reverse one. §11 holds the contracts. Where the doc leaves a choice open, make it,
  and say what you chose and why in your commit message and handback.
- **Say "Veilhound", never "chorister"**: in replies, docs, commits and the names of files you
  make. `chorister` is only the old code identifier; type it only where code needs it.
- **No window on Wrysk's screen, ever.** Pictures and video go headless
  (`--gpu-target headless --gpu-capture DIR`). A window that is truly needed goes through
  `scripts/hidden.sh <cmd>`.
- **No tool call of yours may take longer than 5 minutes.** Wrap builds and tests in
  `timeout 270`. Never poll, sleep-loop, run Monitor or wait on anything.
- **Jobs go through labd** (`WORKING_POLICY.md`, "Jobs go through labd"):
  - a short check (minutes): `scripts/lab/lab quick --name N -- CMD…`;
  - a batch: write a spec and `scripts/lab/lab submit SPEC` (kinds in
    `scripts/lab/labkit/kinds/`; `design/lab-daemon.md`), then hand back at once with the job
    id, where outputs land, and what is left to do once it finishes. The parent keeps the one
    waiter. Batches run straight through: no chunking into 270 s pieces, no early aborts on
    zero counters (report zeros).
  - EC2 is never yours to launch.
- **Builds.** Your worktree has its own `target/`, seeded by reflink from main's. Never point
  `CARGO_TARGET_DIR` at a shared directory, and never put worktrees or targets under `/tmp`.
  Don't pass a `-j` above 8.
- **No `unsafe`** anywhere without Wrysk's approval.
- **Tests check mechanisms** on hand-built fixtures against the config. Never assert a tuned
  default, ratio, population or verdict; no pinned hashes; nothing steps past a few hundred
  ticks. Run only the crates you touched (`timeout 270 cargo nextest run -p <crate>`).
- **Every ecological or behavioural number is a knob**: a serde config field with a doc
  comment (unit and meaning) and a registry row. Regenerate `design/parameters.md` with
  `cargo run -p cubarium-voxel-sim --example params -- --write design/parameters.md`.
- **Heuristics read only what the animal senses.** Bookkeeping records (pack, mother id) may
  feed rewards and census, never behaviour.
- **Saves.** A new saved field bumps the fauna schema. New schemas refuse old worlds; never
  write a migration. Take the next free schema number at the time you commit (another thread
  also bumps it), and say which you took.
- **Presets and fauna defaults** belong to the ecology-training thread. Don't change an
  existing default's value; new knobs that keep today's behaviour by default are fine.
- **Commit often.** Commit on your branch at each milestone, with explicit paths only (never
  `commit -a`). Start a work-in-progress commit's message with `WIP:` plus what's done and
  what's next. The parent may stop a Claude worker when the weekly quota nears its limit;
  your last commit is then the handback, so keep it current.
  - Never merge, deploy, or touch the services on voidrunner, tachyon or eidolon.
  - Codex commits end with `Co-Authored-By: Codex (gpt-6.1-sol) <noreply@openai.com>` and no
    `Claude-Session:` line. Claude commits end with
    `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- **Images and video for Wrysk:** absolute paths. MP4 is x264, yuv444p, crf 12, real time,
  written under the main checkout's `art/gen/runs/<package>-2026-10-12/` (worktree `runs/`
  folders die with the worktree).
- **Return at most 40 lines:** commits, what works and how you checked it, the decisions you
  made and why, what looks wrong or is left, absolute paths.
