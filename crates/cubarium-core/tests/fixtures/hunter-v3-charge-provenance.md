# Genuine schema 12 / profile version 3 snapshots, written before the charging policy existed

These three `.cubw` files are what the **pre-change build actually wrote**. The paid-charging
policy did not exist anywhere in the tree that produced them: `OxidationPolicy`,
`PROFILE_VERSION_CHARGE80` and the member threshold branch were all added afterwards.

## Provenance

Isolated tree, made from the repository's own committed contents at the last pre-change commit —
never the working tree, so no in-progress policy edit could leak in:

```text
commit    0ae10223ae8e56496ee43c0d071f9eac7e016738  ("Plan paid hunter charging threshold experiment")
tree      /tmp/cubarium-pre-charge-fDU26F           (git archive HEAD | tar -x -C …)
generator crates/cubarium-core/examples/pre_charge_fixtures.rs   (added in that isolated copy only)
          SHA256 1c5b7474311d45460415b6897f1c8d389abffef3ec9e3cbf6540cc514919697b
          (committed verbatim beside the fixtures as `hunter-v3-charge-generator.rs.txt`)
build     cargo build --release --example pre_charge_fixtures     ← release, not debug
binary    /tmp/cubarium-pre-charge-fDU26F/target/release/examples/pre_charge_fixtures
          SHA256 cbf0421dc14396d989e3f6d91a5136806cdb52213122d38a6766bdc562a29e64
run       ./pre_charge_fixtures <out-dir>
```

The generator asserts `SCHEMA_VERSION == 12` **and** `PROFILE_VERSION == 3` before writing
anything, so a build that had already been changed could not have produced these files. Every
header carries `schema = 12` (`0c000000` little-endian at offset 4) and build id
`pre-charge-fixture`.

This is a tree of my own making for this package. It is **not** the frozen schema 11 reserve
recipe (`/tmp/cubarium-reserve-recipe-QkCViu`, commit `b547ad0`), which was not read from or
written to here, and not the frozen executables inside
`captures/hunter-reserve-baseline-two-hour-b547ad0` or its candidate counterpart, which are
terminal artifacts and were not touched.

The same isolated tree also produced the **pre-change host binary** used for the old-reader
check, by `cargo build --release -p cubarium --bin cubarium`:
`/tmp/cubarium-pre-charge-fDU26F/target/release/cubarium`, SHA256
`b32db44484077bcccced02929c694c71c2737f55b02189fc23b30406a34ad309`.

## What the world is

Default config and seed, stepped to tick 50, then an actual Lanternjaw trial founded at face 4
(32, 32) — founder `OrganismId { slot: 24, generation: 1 }`, importing 4 m / 7 e — and stepped
on. Three checkpoints, chosen for what they exercise rather than for a round number:

| File | Tick | Founder battery | Why it exists |
| --- | ---: | --- | --- |
| hunter-v3-charge-window.cubw | 1400 | 2.4930211647755267 of 4 = **0.623255** | Strictly inside `(0.50, 0.80)`: the band where the configured threshold does *not* activate oxidation and the candidate policy does. The two policies visibly disagree from these exact bytes. |
| hunter-v3-charge-active.cubw | 5570 | 1.9033987135957944 of 4 = **0.475850** | Below the configured 0.50 threshold and **actually oxidizing** — 200 ticks below it by this point, 0.1 m of reserve already burned — so the version 3 continuation exercises the branch rather than sitting beside it. |
| hunter-v3-charge-active-plus600.cubw | 6170 | 1.7953845713950098 of 4 = 0.448846 | The continuation target. |

At the active checkpoint the founder's reserve is 1.274 of 4, down from the 2.0 it was founded
with: this is a world that has been paying for itself, not a fresh one.

## The files

| File | Tick / population | Payload FNV-1a 64 | SHA256 |
| --- | --- | --- | --- |
| hunter-v3-charge-window.cubw | 1400 / 25 | e4737a3eb5954033 | a14cff3162064abf619fcfa44d53f08e2ad9541002b079255cdaaa861b8d0752 |
| hunter-v3-charge-active.cubw | 5570 / 50 | 669a8b4d7d38869b | 0232120c4121b84f476f9dc7ec6066d65e68908a81ebf30e670b1568c4af0824 |
| hunter-v3-charge-active-plus600.cubw | 6170 / 56 | 8a9242a4f9d0f89e | af1cc582c477af79f46cc5fe8bd4e35b4d675ed365f30ba9ea7b766682f260c7 |

Payload hashes and CRC32s were computed from the raw file bytes outside the crate.

## What the continuation must show

Loading `hunter-v3-charge-active.cubw` into the policy build, stepping 600 ticks, and re-encoding
must reproduce `hunter-v3-charge-active-plus600.cubw`'s payload **byte for byte**, and the run
must report zero extra-oxidation transactions. That is the version 3 identity claim: not
"numerically close", the same arithmetic. Header build ids differ, because a different binary
re-encoded the payload; the payload does not.

`tests/hunter_charging.rs` holds both tests. Nothing in these fixtures is a balance claim: they
are one seed of one config, chosen because it exercises a branch.
