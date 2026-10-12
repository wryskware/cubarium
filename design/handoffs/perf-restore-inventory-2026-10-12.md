---
design_status: exploration
last_reviewed: 2026-10-12
decision_refs: [D-0005]
---

# Renderer performance inventory

Inventory committed before implementation. Ash references below name `953e688f`; port state names `ce035f3a`. Paths abbreviate `crates/cubarium-gpu/src/` as GPU and `crates/cubarium/src/sink/gpu/` as host. Implementation references will be filled in after restoration. No visual changes are intended; the existing art direction (`design/art-direction/Cubarium_Art_Direction_v0.1.md`) and shader interfaces remain fixed.

The source/history audit includes GPU allocation/recording, geometry, lighting, targets and host packing. Perf history: `54745a08`, `fd610869`, `47b4c7d1`, `46c789ab`, `af1f61b8`, `bbc2bd0e`, `aaa87047`, `7fa68e59`, `bd6964df`, `e9259da0`, `f7ea66a8`, `ffaed6ea`, `3c3e2552`, `5e4cc97b`, `77ab4676`, `0a9856da`, `f69ee9cc`, `ab2dcf6a`, `4b42e85e`, `fcb8c5ff`, and cache adoption `087ed6ab`.

| Mechanism / review finding | Ash evidence | State in port | Restoration / disposition |
| --- | --- | --- | --- |
| F1: persistent upload ring; full/brick copies from one mapped slot | GPU voxel.rs:2511,3298–3330,3573–3636; vk.rs:307–357 | lost: buffer and padded vector per region | Pending: mapped COPY_SRC ring, rows packed once, all regions share its buffer. |
| F2: aligned uniform ring and fixed world descriptor sets | GPU voxel.rs:2514–2625,3540 | lost: fresh uniform/emitter buffers and six groups each recording | Pending: persistent ring offsets and groups per resource set. |
| F3: append-only figure models | GPU figures.rs:1251–1263 | lost: entire model store uploaded on redraw | Pending: change-only model uploads and persistent frame buffers/groups. |
| F3: versioned plant cells | GPU plants.rs:338–351,696–750 | lost: cells and tips rebuilt each redraw | Pending: persistent per-slot cells/tips; cells only on version changes. |
| F3: emitter table only when needed | GPU voxel.rs:2635,3475–3540 | changed: rebuilt/uploaded even empty | Pending: versioned table, persistent ring storage. |
| F4 / H2: errors and warm-up completion | GPU vk.rs:366–437; host warm.rs:122–150 | lost: default wgpu panic can strand phase | Pending: logging callbacks, device loss reporting and warm completion guard. |
| F5 / M3: WSI thread, replace stale waiting frame, MAILBOX/FIFO choice | GPU target/window.rs:19–26,552–560; target/presenter.rs:385–408 | lost: WSI in redraw callback, forced FIFO | Pending: presenting thread and bounded latest-frame mailbox. |
| F6: persistent allocations / allocator sizing | GPU vk.rs:307–357; voxel.rs:2511–2635 | changed: MemoryUsage with resource churn | Pending: Performance hint plus persistent resources. |
| F7 / M5: borrowed readback, ordered tags, recycled host buffers | GPU target/readback.rs:144–160; host hosted.rs:909–945 | changed: extra allocated de-padding copy | Pending: borrowed mapped rows, single copy into consumer buffer. |
| F7 / M1: panel direct scanout, no CPU pixels | GPU target/dmabuf.rs:1–6 | lost: GPU readback + three CPU copies | Pending: de-pad/swizzle directly into one persistent DRM mapping; native image export is not exposed by safe wgpu. |
| F8: flat uniforms/instances/fields/scratch persistent | GPU render.rs:331–413,587–635,681–698,769–812 | lost: fresh buffers/groups per frame/region | Pending: persistent flat frame resources and scratch page upload. |
| F9: plant tip rollback without per-frame vector cloning | GPU plants.rs:741–750 | changed: frame_recorded clones even skipped frames | Pending: recycled snapshots, skips retain a marker. |
| F10: readback ownership returned on failure | GPU target/readback.rs:144–175 | changed: map failure pops only readback slot | Pending: release renderer slots on map/poll/wait failures. |
| F11: pack slot lifetime | GPU voxel.rs:1887–1951 | kept: CPU vectors held to retirement | Becomes necessary again with mapped staging; preserve ring conservation. |
| F12: parallel specialized pipeline compilation; first NVIDIA miss serialized | GPU voxel.rs:2754–2832; render.rs:1438–1489 | kept; resident dedup added | Keep; restore disk cache. WGSL parse/emission cost belongs to the shader worker. |
| F13: three present sources / dead emitter region / duplicate scratch size / tool dependencies | GPU render.rs:42; voxel.rs:3475–3540 | changed | Shader sources/dependency split belong to shader worker; restore emitter handling and use shared scratch constant. |
| H1: source of truth / regeneration | GPU shaders (SPIR-V runtime) | changed: generated WGSL runtime | Shader worker owns this; no shader edits here. |
| M2: DRM buffer export without master | GPU target/dmabuf.rs (Vulkan allocation) | changed: DRM dumb allocation | Physical board behavior unverified; safe path retained, no board deployment authorized. |
| M4: adapter-keyed disk pipeline cache, atomic save and app directory | GPU pipeline_cache.rs:80–104,141–155,187–227; voxel.rs:2905 | lost: directory API dead, process cache only | Pending: approved create_pipeline_cache call, feature negotiation, keyed validated blob, atomic write; app/CLI directories. |
| M6: shader bounds checks | GPU render.rs:1370–1432 (precompiled shaders) | changed: safe wgpu checks | Required to keep: no authorization for unsafe shader construction. |
| M7: portable parity harness / frozen evidence | host capture paths outside renderer | changed: worktree-bound specs | Pending: portable copies and strict port-to-branch comparator. |
| L1 / L2: presentation and timestamp reuse | GPU present.rs; render.rs:257; voxel.rs:2287–2295 | lost | Pending: prebuilt present groups/uniforms, per-slot queries/resolve/read buffers. |
| L3: shader lookup copies source | GPU render.rs:1370–1432 | changed: WGSL string copy/hash on hit | Shader worker owns source hashing; retain interface. |
| L4: resident cache lifetime | GPU pipeline_cache.rs:76–80 | changed: device-resident objects never evict | Keep: finite specialization/format/layout set; no measured need for eviction. |
| L5: hidden fallback window guard / readback reuse | host target.rs:111–131 | changed: second fallback window permitted | Pending: restore hidden fallback refusal; recycle fallback readback. |
| L6: sampled-texture capability constant | GPU voxel.rs descriptor layout | changed: duplicated requirement | Pending: common constant; early error for missing immediates too. |
| L7: standalone window dependencies kept off board | window/Cargo.toml:6–11 | changed: winit unconditional | Platform/dependency restructuring outside performance restoration; record for parent. |
| L8: failed panel wait returns frame slot | GPU target/readback.rs:144–175 | lost on error | Pending: unconditional retirement after submitted wait. |
| L9: README migration status | README.md | stale | Doc wording outside renderer performance; record for parent. |
| Slow-plane stamps, image versions, atlas/roof only when changed | GPU voxel.rs:1985–2035; host voxel.rs:2753–2813 | kept | Keep, route copies through persistent staging. |
| Owed bricks and overflow retry survive displacement/newer pack | GPU voxel.rs:1576–1767,3375–3398; host voxel.rs:2918–3128 | kept | Preserve and test with existing conservation tests. |
| Cached raster / content version avoids redundant draws | GPU voxel.rs:3453–3475; host hosted.rs:997–1028 | kept | Preserve unchanged/world-clock behavior. |
| Pack off simulation loop, one posted/recycled snapshot, bounded stage turns | host hosted.rs:90–158,208–286,1084; snapshot.rs | kept | Keep. |
| Terrain Arc/version, roof/plant/animal stamps and sparse water windows | host snapshot.rs; voxel.rs:2813–3128 | kept | Keep: port diff changes backend seam, not packing mechanism. |
| Plant gather only on stand changes; budgeted pack workers | host plants.rs; voxel.rs; perf 77ab4676 / 0a9856da | kept | Keep. |
| Figure tile supersampling / atlas budgeting / seam and swap prioritization | GPU figures.rs:483–722; perf 5e4cc97b | kept | Keep fixed layouts and packing results. |
| Whole-cell specialization avoids shrinking-light walk when absent | GPU voxel.rs:2754–2832; perf f69ee9cc | kept | Keep variant selection. |
| Sunvis Owed/Recorded/Idle, async bake and fades | GPU sunvis.rs; host sunvis.rs | kept, upload allocations lost | Preserve ownership; share world staging ring. |
| Bloom prebuilt resources, immediate parameters, reduced resolution ladder | GPU bloom.rs | kept | Keep. |
| Fast preview Still→All, priority/cancel/lighting ease, first picture unblocked | host warm.rs / hosted.rs; perf 46c789ab / ab2dcf6a | kept except panic failure | Preserve and repair terminal completion. |
| Panel newest-frame mailbox, duplicate-version skip, slot lending/reattach | GPU target/presenter.rs:370–558; host target.rs | kept | Preserve while repairing readback path. |
| One encoder/ordered submit, asynchronous FIFO retirement, bounded slots | GPU target/readback.rs / target/presenter.rs | changed: safe encoder equivalent | Preserve ownership and repair failure conservation. |

All review IDs F1–F13, H1–H2, M1–M7 and L1–L9 appear above. Physical panel verification and native Windows/macOS execution are not claims made by this inventory.
