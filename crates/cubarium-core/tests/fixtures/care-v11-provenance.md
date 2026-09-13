# Genuine schema 11 snapshots, written before the adjustable care dose existed

These four `.cubw` files are what the **pre-dose build actually wrote**. They are not a
current struct re-serialized under an old name: the schema 12 shape did not exist anywhere
on disk when they were produced.

## Provenance

Isolated tree, made from the repository's own committed contents at the last pre-dose
commit — never the working tree, so no in-progress dose edit could leak in:

```text
commit   e55501dbd685f7f02c3599fbaf86c8710f299e7a  ("Specify bounded replayable care dose extension")
tree     /tmp/cubarium-pre-dose-lvTTlU              (git archive HEAD | tar -x -C …)
generator crates/cubarium-core/examples/pre_dose_fixtures.rs
          SHA256 f80fd6e71675b75cd6708fe99b34e6d8a5f579e13b8882dcfa9df51d31abf09b
          (committed verbatim beside the fixtures as `care-v11-generator.rs.txt`)
build    cargo build --release --example pre_dose_fixtures
binary   /tmp/cubarium-pre-dose-lvTTlU/target/release/examples/pre_dose_fixtures
          SHA256 405b02abe11247e1e9c3917941ba2f058abd2d527e5830fe15d7ba49fe92dcd6
run      ./pre_dose_fixtures <out-dir>
```

The generator asserts `SCHEMA_VERSION == 11` before writing anything, so a build that had
already been bumped could not have produced these files. Every header carries
`schema = 11` (`0b000000` little-endian at offset 4) and build id `pre-dose-fixture`.

This is a separate frozen tree from the schema 11 hunter recipe
(`/tmp/cubarium-reserve-recipe-QkCViu`, commit `b547ad0`); neither was edited.

## What each world is

`care-v11-shower-*` — default config and seed, stepped to 100, fed at Top (20, 20);
stepped to 200, cleaned at Front (30, 30); stepped to 300, rained on at Top (40, 40);
checkpointed at tick **360 with the shower 60 of 120 samples in**, so migrating it means
resuming an unfinished rain rather than an idle world. Continued 600 ticks to 960, by which
point the shower has finished.

`care-v11-hunters-*` — default config and seed, stepped to 50, an actual Lanternjaw trial
founded at face 4 (32, 32) — founder `OrganismId { slot: 24, generation: 1 }`, importing
4 m / 7 e — stepped to 120, fed at Top (24, 24), checkpointed at tick **200 with one live
member**, and continued 600 ticks to 800. This one exists because schema 11's own change was
the hunter shape: migrating it proves the dose mirror reads a real extension, not an empty one.

## The files

| File | Tick / population | Schema 11 payload FNV-1a 64 | SHA256 |
| --- | --- | --- | --- |
| care-v11-shower-360.cubw | 360 / 24 | 239ba63f523d93ab | 860eb105eb259205f51db9a17c100f524c1be0adfe85c321cfedae171724dece |
| care-v11-shower-360-plus600.cubw | 960 / 24 | dc91056c8370e7fa | f52bdd9a5da4434b246f61f76c30d4e261f47670da86a0e1f709d758ebd95ce9 |
| care-v11-hunters-200.cubw | 200 / 25 | 23f287b7b8faff79 | 3c9b2a3918a6358a8b8bccd3b2191403a7e07beff89d826c6178bfe5f4e275cc |
| care-v11-hunters-200-plus600.cubw | 800 / 25 | a4febcf32cf40b8b | e1b58972c4b9ee0e851134ca45365705289cadb21a003e89897b1d864ee58787 |

The payload hashes above were computed from the raw file bytes outside the crate, and
`tests/care_dose_migration.rs` recomputes them the same way rather than taking
`snapshot::state_hash` at its word.

## What the continuation must show

Loading the `-360` / `-200` file into the schema 12 build, stepping 600 ticks, and taking the
**schema 11 projection** of the result must reproduce the `-plus600` payload *byte for byte*.
That is the whole claim: a standard dose is not merely numerically close to the pre-dose
arithmetic, it is the same arithmetic. Header build ids differ, because a different binary
re-encoded the payload; the payload does not.
