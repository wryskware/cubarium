# Lanternjaw — Fable's megafauna body candidate

**Art study only.** This is a picture and a motion language, nothing else. It is
not a species, a diet, a capability or a founder. No ecology, no hunting
outcome, no footprint or seam validation is implied or claimed anywhere below.
Source: `fable.js`, drawn by `art/studies/megafauna/index.html` (root's gallery).

## What it is

A long, low, segmented ambusher — mantis shrimp crossed with a glasswing. A
translucent violet carapace of nine plates, a chain of cyan lanterns down the
spine, two raptorial forelimbs folded dark under the head, three pairs of short
walking legs on the thorax, and a thin trailing tail fan. It is 18 px from the
fan to the jaw, 4 px deep through the abdomen and 5 px at the head.

## Why this body

The four common kinds are compact, 8–10 px, and read as *one blob with a
feature*: the burrower is a stub, the grazer a rounded core, the glider a body
with a span, the skimmer a low hull with paddles. Scaling any of them up gives a
bigger blob. A megafauna body has to earn its size with **structure**, so this
one is built from parts a small creature cannot afford:

- **Length, not bulk.** 18 px long but only 4 px deep. Against a 9 px grazer it
  reads as twice the animal at a glance without ever being twice as bright or
  twice as heavy on screen, and it keeps the "dim substrate, occasional brighter
  moving form" hierarchy of `design/appearance.md` intact.
- **Segmentation.** Four full-height dark seams divide the abdomen into plates.
  Segments are what make the sinuous wave legible: each column carries its own
  vertical offset, so the body *articulates* instead of sliding. None of the
  common kinds is jointed, so this alone separates it at any distance.
- **A lantern chain, not a glow.** Five spine lanterns plus a brow lamp echo the
  lanternstalk, tying the animal to the world's plant vocabulary rather than to
  its creatures. The sockets are lit at all times and a single crest travels
  head-to-tail; the chain is the identity read, and at native size it is the
  first thing visible.
- **Folded weapons.** The raptorial forelimbs are *dark* while folded and only
  carry light as they extend. The resting animal is a clean hull with no visible
  armament; the strike is the only moment the arms exist. That is the proposal's
  "folded appendages at rest, a brief committed hunting burst" read as a picture.
- **A tail fan** gives the silhouette a direction from behind, so the animal is
  orientable even when the head is occluded by a plant.

## Palette

Everything comes from the "Palette (leaning, 2026-09-11)" family in
`design/appearance.md`:

| Part | Colour |
| --- | --- |
| Carapace rim, belly line, legs | `#1E2798` mixed toward the dark |
| Carapace plates, segment seams | `#510B6D`, seams mixed further down |
| Head plates, mandibles, claw | `#510B6D` → `#FF2AFC`, 20–32 % |
| Eye | `#FF2AFC` |
| Lantern chain, brow lamp, bloom | `#42C6FF` over a `#42C6FF` socket at 44 % |
| Tail fan | `#42C5F8` at 22–40 % |
| Cocoon | `#42C5F8` / `#42C6FF`, pink only in the core |
| Strike accent | `#FF2AFC` → `#FF9B50` at 45 %, envelope only |

The carapace is "translucent" by being *mixed toward the study background*
rather than by using alpha, so every cube pixel is written exactly once and a
nearest-neighbour enlargement is exact. Value structure: a dim indigo rim, a
dark violet body, a magenta head, cyan points. The orange never appears on its
own — it only tints the pink at the peak of the strike envelope, and it is
absent from `rest`, `move` and `bud` entirely — verified over 1 800 frames at
60 fps, none of which paints anything on the strike's colour ramp.

## Footprint and timing

Anchor `(x, y)` is the mid-line of the body; the animal faces right.

| | value |
| --- | --- |
| Hull | x−9 … x+8, y−3 … y+1 (18 × 5) |
| Painted extent, all modes and times | x−9 … x+12, y−4 … y+4 |
| Contract bound | x ± 14, y ± 12 — 2 px of margin ahead of the strike, 7 px above the back |
| Pixels per frame | 70–89 (`fillRect(…, 1, 1)`, each pixel written once) |

| Mode | rhythms |
| --- | --- |
| `rest` | spine wave 5.5 s (±0.55 px, tapered to the tail), lantern pulse 3.0 s head-to-tail, blink every 4.7 s |
| `move` | gait 1.2 s (three leg pairs at 0.34 phase steps), wave 2.4 s at ±1.15 px, pulse 2.3 s, blink every 5.9 s |
| `hunt` | 6.0 s loop: stalk to 3.10 s, **coil 3.10–3.22** (120 ms, body shortens 2 px, chain charges), **snap 3.22–3.34** (120 ms, cubic ease-out), **recoil 3.34–3.54** (200 ms) — articulated strike **440 ms** — then a 2.46 s still pause; blink at 3.84 s |
| `bud` | wave 6.5 s at ±0.4 px, pulse 3.6 s at 0.8 gain, tail lifted 1 px, cocoon breathing on 2.6 s |

Accent envelopes (raised cosine: 40 % cosine ramp, 20 % plateau, 40 % cosine
ramp, value *and* slope zero at both ends):

- **blink 280 ms**, in every mode — the eye walks to the lid colour and back
  over ~17 frames.
- **strike accent 240 ms**, jaw and claw, opening with the forelimbs and gone
  before the recoil ends.
- The lantern bloom above a bright lantern fades up out of the exact colour
  already beneath it (carapace ridge, then background), so no halo pixel ever
  switches on.

The forelimb tips pass the jaw for 133 ms of the 6 s cycle, reaching x+12 —
four pixels past the jaw at x+8. Every sub-rhythm of `hunt` divides 6.0 s, so
the mode is one clean loop: the frame at t and at t+6 are pixel-identical.

## Verification

`node --check` passes. A stub-context harness drew 240 frames (4 modes × 60
sampled times over 12 s) plus ~5 000 frames at 60 fps for the envelope
measurements:

- all 19 677 fills are 1×1 at integer coordinates, inside x−9…x+12 / y−4…y+4;
- `save`/`restore` balanced on every frame, no `clearRect`, no transform, no
  `drawImage`, no `clip`, no stroke or path calls;
- no cube pixel painted twice in a frame;
- identical `(time, mode, x, y)` produces an identical call log;
- `hunt` is exactly periodic at 6.0 s;
- the strike accent spans 12 frames (200 ms above a 0.08 threshold, on a 240 ms
  envelope) and its largest single-frame step is 0.30 of its own peak; the blink
  shows 16–24 intermediate frames per 12 s with a largest single-frame step of
  0.23 of its full swing — a hard cut would be 1.0 with zero intermediate frames;
- the strike's warm accent never appears in `rest`, `move` or `bud`;
- degenerate input (`NaN` time, unknown mode, no options object at all) neither
  throws nor escapes the box.

Contact sheets were rendered with headless Chromium at native, 3× and 6×
nearest-neighbour on `#0B0525` and iterated on three times: the lantern chain
was invisible at native size and the pulse crest too narrow; the head was one
magenta blob with no eye socket; a dorsal antenna read as a bar floating beside
the head and was cut; and the folded forelimbs plus a doubled set of legs turned
the whole underside into a blue slab at 13×.

## Limitations — read these before choosing

1. **It is an art study.** Nothing here has been checked against the surface
   renderer's nine-pixel radial stamp budget, the seam atlas, the vertex
   ownership rule, or the cost of drawing a body this large. An 18 px body is
   *twice* the 9 px extent the renderer currently enforces; if this direction is
   chosen, that is the first engineering question, and it may well change the
   proportions.
2. **The motion is deliberately stepped.** Every position is rounded to whole
   cube pixels, so the wave, the gait and the lunge move in 1 px jumps rather
   than smoothly. That is a pixel-art choice made for crispness under
   nearest-neighbour enlargement, and it is a *study* property: this is not a
   claim of smooth production playback, and it has not been judged on LEDs at
   room distance, where 1 px steps at 60 fps may read differently than they do
   on a screen. Only the colour envelopes (blink, strike accent, lantern
   brightness) are continuous.
3. **No ecology, and no outcome.** `hunt` is a piece of choreography on a 6 s
   timer. It depicts a strike; it does not depict a kill, a miss, handling time,
   satiety, or prey. `bud` shows one cocoon under the tail; it says nothing
   about gestation cost, parental reserve, maturation or recruitment.
4. **The palette assumes a dark backdrop.** Translucency is faked by mixing
   toward `#0B0525`. On a light or mid backdrop the carapace would show a dark
   halo. The renderer's linear-light conversion has not been applied here
   either — these are sRGB values chosen by eye in a browser.
5. **Anchor rounding.** The anchor is rounded to whole pixels inside `draw`, so
   a caller animating a fractional position gets 1 px snapping rather than
   subpixel brightness. The world's own bodies express subpixel motion through
   fractional brightness; this study does not.
6. **The four-kind comparison is my own reading** of the existing rigs and of
   `design/appearance.md`, not a measured legibility test with a person at
   office distance. The gallery's habitat view and the 8 px grazer reference are
   the honest comparison; the paragraph above is an argument, not evidence.
