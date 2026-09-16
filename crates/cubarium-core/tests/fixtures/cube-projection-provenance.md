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
          SHA256 4610bec9789efb416d293d347832c07a2a09b1c9c72d61d7a6873f88be850d3e
          the *same file* now committed in this tree — it is deliberately self-contained
          (no `Topology`, no `Scale`, nothing the pre-change API lacks) so it can be dropped
          into either checkout unchanged and produce the matching half of the comparison
build     CARGO_TARGET_DIR=<scratch> cargo build --release -p cubarium-core \
              --example export_cube_fixture
binary    SHA256 fa46dc88eac068ed4d2bece78dcdf369719db55708b2b4951cdedc1010e0ae70
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
