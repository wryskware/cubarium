# The pre-ring cube fixture, written by an unmodified `main` build

`cube-projection-v16-2a1cedd.cubw` is what a build **without any topology work** actually
wrote. It is not a current struct re-serialized under an old name: schema 17 did not exist
anywhere in that tree, and `WorldConfig` there has neither `topology` nor `world_scale`.

It is the pre-change half of the `CubeProjection` evidence of
`design/flat-world-plan-2026-09-16.md` §4, read by
`crates/cubarium-core/tests/cube_projection.rs`.

## Provenance

A detached `git worktree` of `main` at the commit `tachyon-screen` branched from — so the
only difference between the two builds is the branch's own work — with its own target
directory, never the main checkout's working tree:

```text
commit    2a1cedd  ("Ecology v1 next steps, round 2: briefs for the intake diagnostic (H), …")
          the merge base of `tachyon-screen` and `main`; `SCHEMA_VERSION` there is 16 and
          `CONFIG_VERSION` is 8
tree      a temporary `git worktree --detach` under the task's scratch directory, removed
          after the fixture was taken
generator crates/cubarium-core/examples/export_cube_fixture.rs
          SHA256 d7a80029488592268506e66d4ef434fd5d5ef0b03f75f11676f0b1139706a0d0
          the *same file* now committed in this tree — it is deliberately self-contained
          (no `Topology`, no `Scale`, nothing the pre-change API lacks) so it can be dropped
          into either checkout unchanged and produce the matching half of the comparison
build     CARGO_TARGET_DIR=<scratch> cargo build --release -p cubarium-core \
              --example export_cube_fixture
binary    SHA256 beab9ace88d52f7a7820a03ae1ceaa093b9c39023206f67fdd8dcbbe11a2b5e4
run       ./export_cube_fixture <out> 6000 20260916
```

The run is `WorldConfig::default()` with `seed = 20_260_916`, then **6,000 `World::step`
calls** — five simulated minutes at `TICK_HZ = 20`, so the weather's per-minute random walk
has fired four times and the world has committed births (24 founders, 46 organisms at the
end) before it is written. The header carries `schema = 16` (`10000000` little-endian at
offset 4) and build id `cube-projection-fixture`.

SHA256 of the fixture itself:
`0a05bb78380599823549c2192802ed58643c061c703fad70713fd3f4f75ac938`.

**Debug and release agree.** The same generator built without `--release` produced a
byte-identical file, so the comparator may run in either profile and the `debug_assertions`
audits inside the step are confirmed to change nothing.

---

# The second pre-ring cube fixture: `main` at its head, after ecology v1 rounds 4 and 5

`cube-projection-v17-15a2210.cubw` is the same run, taken again from an unmodified `main`
build **100 commits later**, when the ring branch merged `main` (SYNC-1,
`design/7_Research/flat-world-sync-main-2026-09-16.md`). The first fixture proves the cube
unchanged against the commit the branch left; this one proves it against the base it
actually merges into, which is the claim that has to hold from here on.

## Provenance

The same recipe as above, with one commit changed:

```text
commit    15a2210  ("Ecology v1 round 4, re-check repairs: W names the ordinary-body motor …")
          `main`'s head when the export was taken; `SCHEMA_VERSION` there is **17** and
          `CONFIG_VERSION` is still 8. Schema 17 on `main` is a semantics-only bump — the
          reach-envelope pursuit rule — and moves no bytes, so its payload shape is 16's and
          the frozen mirror reads it (`crates/cubarium-core/src/snapshot/v17.rs`).
          The merge's second parent is `cdcee03`, one commit later; `cdcee03` differs from
          `15a2210` by **one line of one design document** and by nothing under `crates/`, so
          this binary is the merge base's code.
tree      a temporary `git worktree --detach` under the task's scratch directory, removed
          after the fixture was taken; the main checkout's working tree was never touched
generator crates/cubarium-core/examples/export_cube_fixture.rs — **the same file**, unchanged,
          SHA256 d7a80029488592268506e66d4ef434fd5d5ef0b03f75f11676f0b1139706a0d0, which is
          the hash recorded for the first fixture. It is self-contained on purpose.
build     CARGO_TARGET_DIR=<scratch> cargo build --release -p cubarium-core \
              --example export_cube_fixture
binary    SHA256 55a432d892c57012b6e9b704024fa2da73bf56a29b4af114c786392d60bc4bdd
run       ./export_cube_fixture <out> 6000 20260916
```

SHA256 of the fixture: `b891f1ab2768ead4b36f4c4c3695b6c8c6249c9852d97ab2ab7662f63034d28e`.
Header `schema = 17` (`11000000` little-endian at offset 4), build id
`cube-projection-fixture`, 135,447 bytes, tick 6,000, 46 organisms.

**Debug and release agree** here too: the same generator built without `--release` produced a
byte-identical file.

## The result, which is stronger than the comparison it was taken for

The two fixtures differ in **exactly one byte**: offset 5, the schema field's low byte,
`0x10` against `0x11`. Every one of the other 135,446 bytes is equal.

```text
$ cmp -l cube-projection-v16-2a1cedd.cubw cube-projection-v17-15a2210.cubw
     5  20  21
```

So `main`'s hundred commits — the apex pursuit predicate and its adoption, two motor models,
the apex turn radius, the intake trace, the plant budget, the strike record, the founder
roster and the second founding door — moved **nothing** in a default world. Every one of them
is opt-in, or reaches only a hunter member, and a default world founds no hunter. That is why
the pinned projection hash `10304345502826573087` is the same number before and after the
merge, and why no cube frame golden moved: the world those frames draw is the same world,
byte for byte.

`tests/cube_projection.rs` pins that one-byte statement directly
(`the_two_fixtures_differ_only_in_the_schema_they_declare`) rather than leaving it in prose.
