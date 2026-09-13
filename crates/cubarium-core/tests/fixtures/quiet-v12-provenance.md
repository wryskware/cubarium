# Genuine schema 12 snapshots, written before the ordinary quiet extension existed

These four `.cubw` files are what the **pre-quiet build actually wrote**. `QuietState`,
`QuietPolicy` and the `WorldState.quiet` field did not exist anywhere in the tree that produced
them.

## Provenance

Isolated tree, made from the repository's own committed contents at the last pre-quiet commit —
never the working tree, so no in-progress edit could leak in:

```text
commit    1e6d053ef8cea549dd1d8faf1e042b2b23ff8de6  ("Locate early skimmer losses across all retained seed histories")
tree      /tmp/cubarium-pre-quiet-2qFLRm            (git archive HEAD | tar -x -C …)
generator crates/cubarium-core/examples/pre_quiet_fixtures.rs   (added in that isolated copy only)
          SHA256 ce76719bde869c0413dfb556e24b6f17429e4417d5df01a6cee0c35d579842d2
          (committed verbatim beside the fixtures as `quiet-v12-generator.rs.txt`)
build     cargo build --release -p cubarium-core --example pre_quiet_fixtures   ← release, not debug
          CARGO_TARGET_DIR=captures/build-cache/quiet-policy/pre-quiet
binary    captures/build-cache/quiet-policy/pre-quiet/release/examples/pre_quiet_fixtures
          SHA256 d48c81b2c015d5d952214aaddd25cc9cc6d10d8f25be16e3415fb2dbd0ba66e5
run       ./pre_quiet_fixtures <out-dir> captures/hunter-openings-2026-09-13/seed-1/world-144000.cubw
```

The generator asserts `SCHEMA_VERSION == 12` before writing anything, so a build that had already
been bumped could not have produced these files. Every header carries `schema = 12` (`0c000000`
little-endian at offset 4) and build id `pre-quiet-fixture`.

This is a tree of my own making for this package. It is **not** the frozen schema 12 sources at
`/tmp/cubarium-charge-rerun-512ee52` or `/tmp/cubarium-meal-rollout-d55d8af`, neither of which was
read from or written to here, and not the frozen executables of any running rerun.

## What the worlds are

Both resume the same frozen mature opening the quiet diagnosis used — seed 1 of the prescribed
cohort, `captures/hunter-openings-2026-09-13/seed-1/world-144000.cubw` (schema 9, population 101,
no care, no hunter). A freshly created world has not reproduced yet, and the migration has to be
shown across worlds that carry **real paid births**; by tick 147000 these carry 437.

| File | Tick / pop | Births | What it covers |
| --- | --- | ---: | --- |
| quiet-v12-plain-3000.cubw | 147000 / 98 | 437 | the default **no-care** trajectory |
| quiet-v12-plain-3000-plus600.cubw | 147600 / 99 | 439 | its 600-tick continuation |
| quiet-v12-care-3000.cubw | 147000 / 98 | 437 | the default **care** trajectory: one Standard Feed at elapsed 600, Front (32, 48) — the quiet diagnosis's own recipe |
| quiet-v12-care-3000-plus600.cubw | 147600 / 100 | 439 | its 600-tick continuation |

The care world's feed was `Applied` with `material_in: 3.0`, `energy_in: 6.0` over 5 cells, and
its state carries `care.admitted_seq == 1` with nonzero feed ledgers. The plain world's care state
is default. Both are covered because the proposal requires default no-care **and** default care
trajectories to stay byte-identical in the compatible projection.

## The files

| File | Payload FNV-1a 64 | SHA256 |
| --- | --- | --- |
| quiet-v12-plain-3000.cubw | 55e9f1e223954635 | 62efa71998c5f13877aeccb9ee507adde96288c32753d46b07016d0f85a33857 |
| quiet-v12-plain-3000-plus600.cubw | 77292b3e79ccfcd1 | f21b09e0dd51364ae9ef4f2c64f7e6989231c5088311fd8246ef603aed849e3c |
| quiet-v12-care-3000.cubw | 342d78f1ea06cb0b | 5cea7e575305de05eed228d8dd885e683e933ea9344f41548d0d47db895c355b |
| quiet-v12-care-3000-plus600.cubw | 19ef5ad1afd02b2b | f89bd8ad193002502d9eaadd287eea31839e93130bc3986a6645c5981d456ba5 |

Payload hashes and CRC32s were computed from the raw file bytes outside the crate.

## What the continuation must show

Loading either `-3000` file into the schema 13 build with the policy **Off**, stepping 600 ticks,
and re-encoding the **schema 12 projection** must reproduce the matching `-plus600` payload byte
for byte, with no quiet record published. That is the Off-identity claim: not "numerically close",
the same arithmetic. Header build ids differ, because a different binary re-encoded the payload;
the payload does not.

`tests/quiet_migration.rs` holds both continuations and the refusals beside them. Nothing in these
fixtures is a balance claim: they are one seed of one config, chosen because it reproduces.
