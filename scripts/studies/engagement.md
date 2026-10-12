# ENGAGE

The reusable entry points are `cubarium_voxel_sim::engagement::{found, run}`;
`Episode` exposes its `Sim`, roster IDs, mother relationship and pack for later
training arms. `step` performs the same readout used by `run` and the video.

`config/engagement.toml` defines the nine baseline arms and 50 episodes per arm.
All ecological scenario numbers have serde fields and registry rows, included
in the generated `design/parameters.md`. New arms select composition, pack size
and optional ordinary fauna registry `sets`; setup is finished before overrides.

The arena is hand-built bare soil, 32 m around the periodic ring and 24 m deep:
the train-flat map's 4.2 m plain cannot fit an approach and flanking bodies.
Terrain and flora are frozen using the trainer's static schedule. Today's fauna
physiology, sensory field and heuristics run unchanged. The daylight-only scene
is intentional: a controlled awake encounter, not a live-world sleep study.
Family reserve/stomach are 85–95% full, pack reserve/stomach 10–20% full.
The hunting latch engages through the usual empty-stomach rule. Adults start
at 90–100% maximum structure and 600–900 s age; calves at 16–22% and 0–30 s.
Arrival ages preserve each body's originally drawn remaining lifespan.

A seed determines world seed, bearing, centres, stores and ages. Each quantity
uses a fixed keyed stream. All seven roster slots are created before omitted
bodies are removed, preserving common IDs, positions, energies and kin links
across compositions and pack sizes. A lone calf retains its absent mother's ID.
`usual-prey` adds ordinary non-pack prey counts; default episodes contain only
the family and pack. Arms never participate in the random start.

Rows use kill identity events plus recorded carcass consumption for a kill
outcome. Giving up requires a recorded abandoned chase and homeward state for
all survivors; all hounds gone also ends the encounter. Other episodes timeout.
Wounds are sums/worst of each body's peak wound stock, with a recorded wound
kill represented by stock 1. Encounters are distinct sensed hound/family pairs;
tests are recorded BrokeOff chase endings, chases are ChaseStart events on the
family, kills are exact family victim IDs. Captures are null: today's bites
have no holding stage. These definitions are explicit and should be retained
when comparing later arms; they are not position-based proxies.

Run a short check through `lab quick`, for example:

```
scripts/lab/lab quick --name engage-smoke -- target/release/examples/engagement_probe config=config/engagement.toml arm=family-3 episodes=3 seconds=30
```

Read the resulting JSON rows and choose an active family-3 seed. Then generate
and submit a spec (the generated file records source commit and binary hash):

```
python3 scripts/studies/engagement-spec.py --probe target/release/examples/engagement_probe --film-seed N --out config/engagement-lab.toml
scripts/lab/lab submit config/engagement-lab.toml
```

The existing batch kind runs `engagement-task.py` as its binary adapter. Each
seed runs one complete episode, with batch.py's L3 placement; there are no
counter tripwires, segment restarts or early batch aborts. The last successful
child reduces atomic `outcome.json` files into one JSONL per arm and
`summary.tsv`, then films the selected seed. Reduction waits for every expected
row by checking at child completion, not by a polling loop. Desktop-only
placement keeps the built executable and presentation assets local. The adapter
checks the submitted executable hash before every episode.

Outputs are in `<job>/batch-task/batch/`: per-seed rows, per-arm JSONL,
`summary.tsv`, `video.log`, `VIDEO_DONE` or `VIDEO_FAILED`. A missing/failed
child is visible in `STATE.tsv`; no partial table pretends to be complete.
Video uses the normal headless voxel sink and existing art/models, one panel,
30 fps at real simulated time, x264/yuv444p/CRF 12. It streams RGBA directly to
ffmpeg, without a PNG frame archive, under main's
`art/gen/runs/engage-2026-10-12/family-3-seedN.mp4`.
