# Paired meal-continuity review

Development diagnostics only; no live world or care input. `meal_capture` produces
old/new native nets on one identical copied-world trajectory. Use its three
prescribed output names: `seed1-t0-feed`, `seed8-t0-feed`, `seed1-noinput`.
See [the candidate record](../../../design/7_Research/meal-onset-continuity-2026-09-13.md).

```sh
node art/studies/meals/package.mjs captures/meal-reviewed-8d489f7 captures/meal-viewer-8d489f7
python3 -m http.server 7395 --bind 127.0.0.1 --directory captures/meal-viewer-8d489f7
```

The package refuses existing output, validates native dimensions and frame counts,
and fingerprints the concatenated PNG bytes for each stream. It copies reports
and observational records; it does not edit source captures or snapshots.
Bundled data URLs avoid thousands of individual HTTP requests. Open the local page
in a development browser, select the case, play/pause or scrub. `?case=seed1-noinput`
selects a case on load. Full-ID tracking shows a24px crop clipped inside its actual
face; it is not a seam-unfolded neighborhood. All enlargement is nearest-neighbor.

Playback is1× at60fps with an explicit excerpt replay cut. `window.review.samples`
records bounded callback timestamps/frame indices for cadence inspection. These
are browser playback measurements, not physical cube scanout or ecological
evidence. Native frame sampling and authored gesture comparison still require
visual judgment; a different picture alone is not proof of improvement.
