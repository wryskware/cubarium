# Vendored dependencies

## cube-proto

Copied verbatim (sources and tests) from the display shim's `crates/cube-proto`
at Git revision `7a21b5f7c7a22ee36b4930471cdf1afaef45abdd` of
`~/vuzic/led-cube-shim` (local checkout, no published remote at vendoring time).

Only `Cargo.toml` differs: workspace-inherited `edition`/`license` and the
`serde` dependency are spelled out so the crate builds inside this workspace,
and `tests/python_interop.rs` is omitted because it depends on the shim's
Python client outside the crate.

The shim owns this code and has been verified independently on the cube; do not
edit the copy here. Refresh it with `scripts/sync-cube-proto.sh`, which records
the new revision in `vendor/cube-proto.rev`. Switch to a `git` dependency with a
pinned `rev` once the shim has a published remote.
