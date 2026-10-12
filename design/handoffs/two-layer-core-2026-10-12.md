---
status: open
date: 2026-10-12
owner: Opus (high), worktree `.claude/worktrees/two-layer`, branch `two-layer`
---

# TWO-LAYER (core): the two-layer brain network and its cloning

## Rules

Read `design/handoffs/behaviour-ai-worker-rules-2026-10-12.md` first. It covers windows, the
5-minute limit, labd, builds, tests, knobs, saves, commits and the return format. Use graft
(`graft ask … --source`, `graft callers`) before grepping.

## Why

This is the first half of package 7 of `design/behaviour-ai-2026-10-11.md`. Wrysk's topic 3
(§0): every brain becomes two learned layers. A slow top layer picks and holds a named
intent and its subject; a fast bottom layer (today's GRU) reads the senses plus that intent
and steers. The rules are teachers only. Read §0, §2.2, §10 (all) and §11.3; skim §11.1 and
§11.7.

This half builds the network, its arbitration and its cloning **inside
`crates/cubarium-search/`**, against plain inputs, while the CONTRACTS package
(`design/handoffs/contracts-2026-10-12.md`) builds the fauna-side records and controller seam
in parallel. The adapter joining them, the per-lineage label runs and the closed-loop
competence checks come after both merge (the parent briefs that second half).

## Do

Commit after each numbered step.

1. **Read today's brain first.**
   - the GRU runtime: `crates/cubarium-search/src/es/voxel/controller.rs` (P3, merged
     71b5d8ef);
   - imitation: `imitate.rs`, including the option head at ~822;
   - how ES flattens and perturbs parameters: `trainer.rs`, `live/trainer.rs`;
   - the policy file format: `VoxelPolicyFile`.

   Build on these; don't fork a second GRU.
2. **The network** (§10.1), in a new module beside the controller.
   - **Bottom layer:** today's GRU. Its input is the observation plus:
     - the held intent, one-hot over the menu plus two spare slots kept masked;
     - the subject's candidate row: features plus a type tag, zeros when there's no
       subject. This stands in for §10.1's "mark on the subject's slot": slot-order
       independent and the same information. Say if you choose otherwise.
     - time in the current intent, squashed.
   - **Bottom outputs:** today's continuous actions, plus discrete heads where the lineage
     has them: a kick trigger, a call kind (including none) and a rest request.
   - **Top layer:** a small head over the GRU hidden state, the held intent and the time
     held. It gives:
     - masked logits over the menu;
     - a subject pointer over K candidate rows, conditioned on the proposed intent and
       masked per intent.
   - **Dimensions** (menu size, K, row width F, which discrete heads exist) are runtime
     configuration from the lineage, not constants.
3. **Order and arbitration** (§11.3).
   - At a decision sample:
     1. The GRU updates once with the observation and the previously held intent.
     2. The top reads that state and proposes.
     3. Arbitration settles the held intent and subject.
     4. The settled intent and subject condition the bottom from the next sample.
   - The top runs at the intent rate (4 Hz, a knob). The bottom runs at every controller
     sample.
   - `hold_s` and `switch_margin` apply to probabilities at a fixed temperature (all
     per-lineage knobs), not raw logits.
   - A hold ends early when told: subject lost or dead, intent masked, or a physical
     emergency. The top then chooses again.
   - Choices are deterministic (argmax), as today's ES brains are.
4. **A plain brain is a one-item menu** (§10.1).
   - No top head, a constant intent input, K = 0, no discrete heads.
   - An existing plain policy file (frondgrazer, littershredder) loads unchanged and gives
     **identical** outputs to today's controller. This is a hard requirement: their training
     runs in another thread now.
5. **ES layout** (§10.4).
   - The parameter vector names its top and bottom ranges, so stage 1 can perturb only the
     top.
   - Expose that as an API (a mask or ranges); don't wire it into the trainers. KIN-TRAIN
     does that.
   - The policy file gets a versioned two-layer format. Plain files keep today's.
6. **State for saves.** Recurrent state, held intent, subject candidate, hold age and the
   proposal are serde-serialisable, so a load or a matched training start continues rather
   than cold-starts (§11.3).
7. **Cloning** (§10.2, §11.3).
   - A trajectory record type carries, per sample:
     - the observation, intent mask and candidate rows;
     - the teacher's **proposal before its hold** (the label for the top);
     - the teacher's held intent and subject (the bottom's conditioning);
     - the teacher's actions.
   - Losses:
     - the top: cross-entropy on the proposal, plus the subject pointer's cross-entropy;
     - the bottom: today's action-imitation loss, teacher-forced on the teacher's held
       intent and subject.
     - The strategic loss also trains the shared representation.
   - Run the same order as step 3, so no label leaks in through the input.
   - Leave room in the record type for alternative-mode examples and a closed-loop
     correction pass (the learner picks intents, the teacher labels the states reached).
     Producing those needs the fauna side, so it's the second half's job.

## The interface shared with CONTRACTS

The fauna side gives the brain one decision sample at a time. The CONTRACTS worker builds the
producer. Keep your inputs as plain slices and structs in this crate (no fauna types), so the
adapter is a thin layer written after both merge.

- **The fauna side gives, per decision sample:**
  - the observation vector, in manifest order;
  - the menu size and a validity mask over intents;
  - K subject-candidate rows, each:
    - a fixed width F per lineage;
    - a type tag (body or place);
    - a validity mask per intent;
  - an early-end flag with its reason: subject lost or dead, intent masked, or a physical
    emergency.
- **The brain answers with:**
  - today's continuous actions;
  - the discrete actions where the lineage has them: the kick trigger, the call kind
    (including none) and the rest request;
  - its held intent index and subject candidate index (or none), after its own hold and
    margin;
  - its proposal probabilities, for logging.

## Boundaries

- Don't edit `crates/cubarium-voxel-fauna/`: CONTRACTS is reworking it.
- Don't wire into the trainers (`trainer.rs`, `live/trainer.rs`, `live/window.rs`). That's
  KIN-TRAIN's job.
- Extend `imitate.rs` only where sharing its machinery is clearly better than a new module
  beside it.
- No labelled data from the real heuristics yet. That waits for CONTRACTS' records.

## Checks (written into tests as you go)

- A plain policy file gives outputs identical to today's controller over a fixed observation
  sequence.
- **Arbitration:**
  - a proposal below the margin doesn't switch;
  - nothing switches before `hold_s`;
  - an early-end flag forces a fresh choice;
  - a masked intent is never held;
  - a subject is never picked from an invalid row;
  - a logit-scale change at a fixed temperature doesn't change switching on its own
    account. Test this as a mechanism, not a number.
- **Order:** with the teacher's intent fed as conditioning, the top's label at sample t never
  appears in the bottom's input at sample t.
- **Saves:** serialise mid-hold, restore, and the next outputs are identical.
- **Cloning, on a synthetic teacher** (the right intent depends on the input, and the
  actions depend on the intent): the cloned net picks the teacher's intent and copies each
  mode's actions far better than an intent-blind clone, on held-out samples. Assert the
  comparison, not a tuned accuracy.
- Report one decision sample's cost (top plus bottom) beside today's GRU step.

## Return

The rules file's format, plus:
- the module and type names with file:line;
- the policy file format;
- the ES top/bottom API;
- every departure from §10–§11.3, and why.
