# Committed pictures

`synthetic-320x180-s1.png`, `synthetic-640x360-s2.png` — GS-1's synthetic ring scene at
two rungs of the ladder, compared **byte for byte** by `tests/golden.rs`. A change here is
a change to the renderer's output and has to be argued for, not re-recorded.

`hunter-cpu-320x180.png`, `hunter-gpu-320x180.png` — GS-1c item 3: one frame of a
`ring:320x180` world at seed 20260916, tick 3,000, `f = 0`, with the Lanternjaw trial
started at its centre, drawn by the CPU presenter and by `--sink gpu` in its own pixel-art
sampler. **A review pair, not a golden**: the two are not expected to be identical (the
samplers differ), and nothing compares them automatically. The rig is the body at
`[154..166] x [86..104]`; `cubarium`'s `tests/gpu_fidelity.rs::the_rig_composites_part_
over_part_as_the_cpus_single_query_does` is what actually measures them, and re-records
these with `CUBARIUM_GPU_FIDELITY_DUMP=<dir>`.
