// Lanternjaw — Fable's megafauna body candidate.
//
// ART STUDY ONLY. This file draws a picture; it is not a creature, a species,
// a diet or a capability. Nothing here has been checked against the surface
// renderer's footprint budget, the seam atlas, or any ecology.
//
// Canvas2D, native resolution: one unit is one cube pixel. Faces right,
// anchored at (x, y). Deterministic pure function of (time, mode).

const TAU = Math.PI * 2;

// Palette: design/appearance.md "Palette (leaning, 2026-09-11)".
const BG = [11, 5, 37]; // #0B0525, the study background; used only to mix shades.
const INDIGO = [30, 39, 152]; // #1E2798
const VIOLET = [81, 11, 109]; // #510B6D
const PINK = [255, 42, 252]; // #FF2AFC
const CYAN = [66, 198, 255]; // #42C6FF
const CYAN_S = [66, 197, 248]; // #42C5F8
const ORANGE = [255, 155, 80]; // #FF9B50 — brief flash only.

// Hard bound on what this candidate is allowed to paint, relative to (x, y).
const MAX_X = 13;
const MAX_Y = 11;

function clamp(v, lo, hi) {
  return v < lo ? lo : v > hi ? hi : v;
}

function smoothstep(u) {
  const c = clamp(u, 0, 1);
  return c * c * (3 - 2 * c);
}

function frac(v) {
  return v - Math.floor(v);
}

function mix(a, b, t) {
  const u = clamp(t, 0, 1);
  return [
    a[0] + (b[0] - a[0]) * u,
    a[1] + (b[1] - a[1]) * u,
    a[2] + (b[2] - a[2]) * u,
  ];
}

function css(c) {
  const r = clamp(Math.round(c[0]), 0, 255);
  const g = clamp(Math.round(c[1]), 0, 255);
  const b = clamp(Math.round(c[2]), 0, 255);
  return "rgb(" + r + "," + g + "," + b + ")";
}

// Static shades. The carapace is "translucent" by being mixed toward the dark
// study background rather than by using alpha, so every pixel is painted once
// and nearest-neighbour enlargement stays exact.
const EDGE = mix(BG, INDIGO, 0.72); // carapace rim / outline
const C_EDGE = css(EDGE);
const C_SHELL = css(VIOLET); // carapace plate
const C_SEAM = css(mix(BG, VIOLET, 0.45)); // segment division
const C_PLATE = css(mix(VIOLET, PINK, 0.20)); // head plates
const C_JAW = css(mix(VIOLET, PINK, 0.26)); // folded mandibles
const C_EYE = css(PINK);
const EYE_SHUT = mix(BG, VIOLET, 0.55); // fully closed lid, only reached mid-blink
const C_FAN = css(mix(BG, CYAN_S, 0.40)); // trailing tail fan
const C_FAN_TIP = css(mix(BG, CYAN_S, 0.22));
const C_LEG = css(mix(BG, INDIGO, 0.82));
const C_LEG_FAR = css(mix(BG, INDIGO, 0.45));
// The raptorial limbs are a colder violet-blue than the head, and they are dark
// while folded: the resting silhouette stays one clean hull, and the strike is
// the only moment the arms carry light. Both are one continuous function of the
// same reach envelope, so neither can pop.
const LIMB_DARK = mix(BG, INDIGO, 0.80);
const LIMB_LIT = mix(INDIGO, PINK, 0.45);

// Strike accent, reached only at the peak of a 240 ms raised-cosine envelope.
const JAW_REST = mix(VIOLET, PINK, 0.26);
const JAW_HOT = mix(PINK, ORANGE, 0.45);
const CLAW_REST = mix(VIOLET, PINK, 0.32);
const C_CLAW = css(CLAW_REST);

// Lantern chain: the sockets glow at all times; the pulse is a travelling lift.
const LANTERN_DIM = mix(BG, CYAN, 0.44);

// Body template. One character per cube pixel; row 0 is dy = -4 and column 0
// is dx = -9, so the hull runs from the tail fan at x-9 to the jaw at x+8.
//
//   .            oooo .     head crown
//   f   ooooooooo Lvvvo     back ridge, brow lamp, head plates
//   ff oLdLdLdLdLccecj      lantern chain / segment seams, eye, jaw
//   fffocdcdcdcdccccjj      body core, segment seams, mandibles
//   ff  ooooooooooddj .     belly line
//   f                       tail fan
const ROWS = [
  "                  ",
  "             oooo ",
  "f   oooooooooLvvvo",
  "ff oLdLdLdLdLccecj",
  "fffocdcdcdcdccccjj",
  "ff  ooooooooooddj ",
  "f                 ",
  "                  ",
];
const ROW_TOP = -4; // ROWS[0] sits at dy = -4
const COL_LEFT = -9; // column 0 sits at dx = -9
const COL_COUNT = 18;
const LAST_COL = COL_COUNT - 1;

// Parsed once: [colIndex, row, char] in painting order (top-left to
// bottom-right), so the draw loop never re-scans the strings.
const CELLS = [];
for (let r = 0; r < ROWS.length; r += 1) {
  const row = ROWS[r];
  for (let i = 0; i < COL_COUNT; i += 1) {
    const ch = row.charAt(i);
    if (ch !== " " && ch !== "") CELLS.push([i, ROW_TOP + r, ch]);
  }
}

// Raptorial forelimb poses, in body-local pixels: shoulder, elbow, claw.
const LIMB_SHOULDER = [4.0, 1.0];
const LIMB_FOLD = { elbow: [6.4, 2.2], claw: [4.3, 2.5] };
const LIMB_COCK = { elbow: [5.8, 1.7], claw: [3.6, 0.7] };
const LIMB_STRIKE = { elbow: [8.4, 1.9], claw: [12.3, 0.6] };

const LEG_BASE = [-1, 1, 3]; // walking legs, body-local x

// Hunt cycle. One articulated strike inside a long calm cycle.
const HUNT_PERIOD = 6.0;
const T_COIL = 3.1; // coil begins
const T_SNAP = 3.22; // forelimbs release
const T_OPEN = 3.34; // full extension
const T_END = 3.54; // folded again -> strike lasts 0.44 s

const MODES = { rest: true, move: true, hunt: true, bud: true };

// Nothing in this body is allowed to switch on for a single frame. Every
// accent rides this raised-cosine envelope: a cosine ramp up over the first
// 40 % of its window, a short plateau, a cosine ramp down. Value and slope are
// both zero at each end, so an accent never pops.
function envelope(u) {
  if (u <= 0 || u >= 1) return 0;
  if (u < 0.4) return 0.5 - 0.5 * Math.cos((Math.PI * u) / 0.4);
  if (u < 0.6) return 1;
  return 0.5 - 0.5 * Math.cos((Math.PI * (1 - u)) / 0.4);
}

const BLINK_SECONDS = 0.28; // full close-and-open, eased at both ends
const ACCENT_SECONDS = 0.24; // strike accent on the jaw and claw

// Seconds since the most recent blink start, folded into the envelope.
function blinkClosure(t, period) {
  return envelope((frac(t / period) * period) / BLINK_SECONDS);
}

function limbPose(reach) {
  // reach < 0 : cocked back. 0 : folded. 1 : fully extended.
  const from = reach < 0 ? LIMB_COCK : LIMB_FOLD;
  const to = reach < 0 ? LIMB_FOLD : LIMB_STRIKE;
  const u = reach < 0 ? 1 + reach / 0.35 : reach;
  return {
    elbow: [
      from.elbow[0] + (to.elbow[0] - from.elbow[0]) * u,
      from.elbow[1] + (to.elbow[1] - from.elbow[1]) * u,
    ],
    claw: [
      from.claw[0] + (to.claw[0] - from.claw[0]) * u,
      from.claw[1] + (to.claw[1] - from.claw[1]) * u,
    ],
  };
}

// Everything the hunt cycle drives, as a pure function of the cycle phase.
function huntState(tH) {
  const s = { reach: 0, compress: 0, lunge: 0, charge: 0, accent: 0, blink: 0 };
  if (tH >= T_COIL && tH < T_SNAP) {
    const u = smoothstep((tH - T_COIL) / (T_SNAP - T_COIL));
    s.reach = -0.35 * u;
    s.compress = 1.7 * u;
    s.charge = u;
  } else if (tH >= T_SNAP && tH < T_OPEN) {
    const u = (tH - T_SNAP) / (T_OPEN - T_SNAP);
    const e = 1 - (1 - u) * (1 - u) * (1 - u); // fast release, soft arrival
    s.reach = -0.35 + 1.35 * e;
    s.compress = 1.7 - 2.0 * e;
    s.lunge = 1.1 * e;
  } else if (tH >= T_OPEN && tH < T_END) {
    const u = smoothstep((tH - T_OPEN) / (T_END - T_OPEN));
    s.reach = 1 - u;
    s.compress = -0.3 + 0.3 * u;
    s.lunge = 1.1 * (1 - u);
  }
  if (tH >= T_SNAP) {
    // Charge bleeds out of the lantern chain, and is forced to zero before the
    // cycle wraps so the loop has no step.
    s.charge =
      Math.exp(-(tH - T_SNAP) / 0.9) * smoothstep((HUNT_PERIOD - tH) / 0.6);
  }
  // The jaw accent is an envelope over the strike, not a flash: it opens with
  // the forelimbs and is gone before the recoil ends.
  s.accent = envelope((tH - (T_SNAP - 0.02)) / ACCENT_SECONDS);
  // One blink after the recoil; the stalk itself is unblinking.
  s.blink = envelope((tH - (T_END + 0.3)) / BLINK_SECONDS);
  return s;
}

export const candidate = {
  name: "Lanternjaw",
  author: "Fable 5.1",
  description:
    "A long, low segmented ambusher, 18 px from tail fan to jaw. Translucent " +
    "violet carapace plates carry a cyan lantern chain that pulses head-to-tail " +
    "along the spine; two raptorial forelimbs stay folded under the head until " +
    "the strike throws them four pixels past the jaw for a sixth of a second.",

  draw(ctx, options) {
    const o = options || {};
    const rawT = o.time;
    const t = typeof rawT === "number" && isFinite(rawT) ? rawT : 0;
    const mode = MODES[o.mode] ? o.mode : "rest";
    const ax = Math.round(
      typeof o.x === "number" && isFinite(o.x) ? o.x : 0,
    );
    const ay = Math.round(
      typeof o.y === "number" && isFinite(o.y) ? o.y : 0,
    );

    // --- motion parameters -------------------------------------------------
    let waveAmp = 0.55;
    let wavePeriod = 5.5;
    let pulsePeriod = 3.0;
    let pulseGain = 1.0;
    let gait = 0; // 0 = legs planted
    let compress = 0;
    let lunge = 0;
    let reach = 0;
    let tailLift = 0;
    let closure = blinkClosure(t, 4.7);
    let accent = 0;
    let bud = false;

    if (mode === "move") {
      waveAmp = 1.15;
      wavePeriod = 2.4;
      pulsePeriod = 2.3;
      gait = 1.2;
      closure = blinkClosure(t, 5.9);
    } else if (mode === "hunt") {
      const tH = t - Math.floor(t / HUNT_PERIOD) * HUNT_PERIOD;
      const h = huntState(tH);
      // Every sub-rhythm divides HUNT_PERIOD, so the whole mode is one clean
      // 6 s loop with a single strike in it.
      waveAmp = 0.22;
      wavePeriod = 6.0;
      pulsePeriod = 3.0;
      pulseGain = 0.38 + 0.92 * h.charge;
      compress = h.compress;
      lunge = h.lunge;
      reach = h.reach;
      accent = h.accent;
      closure = h.blink;
    } else if (mode === "bud") {
      waveAmp = 0.4;
      wavePeriod = 6.5;
      pulsePeriod = 3.6;
      pulseGain = 0.8;
      tailLift = 1;
      bud = true;
      closure = blinkClosure(t, 5.3);
    }

    // Accent colours for this frame, interpolated by the envelopes.
    const lit = clamp(reach, 0, 1);
    const jawColor = accent > 0 ? css(mix(JAW_REST, JAW_HOT, accent)) : C_JAW;
    const clawColor = accent > 0 ? css(mix(CLAW_REST, JAW_HOT, accent)) : C_CLAW;
    const limbColor = css(mix(LIMB_DARK, LIMB_LIT, lit));
    const limbFarColor = css(mix(BG, mix(LIMB_DARK, LIMB_LIT, lit), 0.55));
    const eyeColor = closure > 0 ? css(mix(PINK, EYE_SHUT, closure)) : C_EYE;

    // --- pixel buffer ------------------------------------------------------
    // Each cube pixel is written at most once, last writer wins, so overlaps
    // are resolved as draw order instead of as alpha soup.
    const cells = new Map();
    const put = (sx, sy, color) => {
      const px = Math.round(sx);
      const py = Math.round(sy);
      if (px < -MAX_X || px > MAX_X || py < -MAX_Y || py > MAX_Y) return;
      cells.set(((py + 16) << 6) | (px + 16), color);
    };
    const line = (a, b, color) => {
      const steps = Math.max(
        1,
        Math.round(Math.max(Math.abs(b[0] - a[0]), Math.abs(b[1] - a[1]))),
      );
      for (let i = 0; i <= steps; i += 1) {
        const u = i / steps;
        put(a[0] + (b[0] - a[0]) * u, a[1] + (b[1] - a[1]) * u, color);
      }
    };

    // Per-column body offsets: a travelling sinuous wave, looser at the tail,
    // plus the coil (shorten) and the head lunge.
    const colDx = new Array(COL_COUNT);
    const colDy = new Array(COL_COUNT);
    for (let i = 0; i < COL_COUNT; i += 1) {
      const rel = i / LAST_COL;
      const taper = 0.25 + 0.75 * (1 - rel);
      let dy = waveAmp * taper * Math.sin((TAU * t) / wavePeriod + i * 0.4);
      if (i < 3) dy -= tailLift;
      colDy[i] = Math.round(dy);
      colDx[i] = Math.round(
        compress * (1 - rel) + lunge * clamp((i - 9) / 8, 0, 1),
      );
    }

    // --- far forelimb (behind the body) ------------------------------------
    const drawLimb = (r, dx, dy, body, tip) => {
      const pose = limbPose(r);
      const sh = [LIMB_SHOULDER[0] + dx, LIMB_SHOULDER[1] + dy];
      const el = [pose.elbow[0] + dx, pose.elbow[1] + dy];
      const cl = [pose.claw[0] + dx, pose.claw[1] + dy];
      line(sh, el, body);
      line(el, cl, body);
      put(cl[0], cl[1], tip);
    };
    const headDx = colDx[13];
    const headDy = colDy[13];
    // The far limb lags a frame and a half and rides a pixel higher.
    const farReach =
      mode === "hunt"
        ? huntState(
            (() => {
              const p = t - 0.045;
              return p - Math.floor(p / HUNT_PERIOD) * HUNT_PERIOD;
            })(),
          ).reach
        : reach;
    const farLit = css(
      mix(BG, mix(LIMB_DARK, LIMB_LIT, clamp(farReach, 0, 1)), 0.55),
    );
    drawLimb(farReach, headDx, headDy - 1, farLit, farLit);

    // --- legs ---------------------------------------------------------------
    for (let k = 0; k < LEG_BASE.length; k += 1) {
      const base = LEG_BASE[k];
      const ci = base - COL_LEFT;
      const dx = colDx[ci];
      const dy = colDy[ci];
      // One visible pair per segment only: doubling the legs turned the whole
      // underside into a slab at 13x.
      for (let far = 0; far >= 0; far -= 1) {
        let swing = 0;
        let lift = 0;
        if (gait > 0) {
          const u = frac(t / gait + k * 0.34 + far * 0.5);
          swing = 1.3 * Math.sin(TAU * u);
          lift = u < 0.38 ? 1 : 0;
        }
        const color = far ? C_LEG_FAR : C_LEG;
        const ox = far ? -0.6 : 0;
        const oy = far ? -1 : 0;
        put(base + dx + ox, 2 + dy + oy, color);
        put(base + dx + ox + swing, 3 + dy + oy - lift, color);
      }
    }

    // --- cocoon (bud) -------------------------------------------------------
    if (bud) {
      const p = 0.5 + 0.5 * Math.sin((TAU * t) / 2.6);
      const shell = css(mix(BG, CYAN_S, 0.34 + 0.16 * p));
      const rim = css(mix(BG, CYAN, 0.5 + 0.24 * p));
      const core = css(mix(mix(BG, CYAN, 0.62), PINK, 0.18 + 0.3 * p));
      const dy = colDy[4];
      put(-5, 2 + dy, shell);
      put(-4, 2 + dy, rim);
      put(-3, 2 + dy, shell);
      put(-5, 3 + dy, shell);
      put(-4, 3 + dy, core);
      put(-3, 3 + dy, shell);
    }

    // --- hull ---------------------------------------------------------------
    // Lantern brightness for a column: one crest travelling head to tail.
    const lantern = (i) => {
      const u = frac(t / pulsePeriod + (LAST_COL - i) * 0.06);
      const d = Math.min(u, 1 - u);
      const b = smoothstep(1 - d / 0.26);
      return clamp(b * pulseGain, 0, 1.25);
    };

    for (let n = 0; n < CELLS.length; n += 1) {
      const cell = CELLS[n];
      const i = cell[0];
      const sx = COL_LEFT + i + colDx[i];
      const sy = cell[1] + colDy[i];
      const ch = cell[2];
      if (ch === "o") put(sx, sy, C_EDGE);
      else if (ch === "c") put(sx, sy, C_SHELL);
      else if (ch === "d") put(sx, sy, C_SEAM);
      else if (ch === "v") put(sx, sy, C_PLATE);
      else if (ch === "f") put(sx, sy, i === 0 ? C_FAN_TIP : C_FAN);
      else if (ch === "e") put(sx, sy, eyeColor);
      else if (ch === "j") put(sx, sy, jawColor);
      else if (ch === "L") {
        const b = lantern(i);
        put(sx, sy, css(mix(LANTERN_DIM, CYAN, b)));
        // The bloom fades up out of the exact colour already under it — the
        // carapace ridge, then the background — so no halo pixel ever pops on.
        const halo = smoothstep((b - 0.35) / 0.45);
        if (halo > 0) put(sx, sy - 1, css(mix(EDGE, CYAN, 0.5 * halo)));
        const glow = smoothstep((b - 0.72) / 0.28);
        if (glow > 0) put(sx, sy - 2, css(mix(BG, CYAN, 0.22 * glow)));
      }
    }

    // --- near forelimb ------------------------------------------------------
    drawLimb(reach, headDx, headDy, limbColor, clawColor);

    // --- flush --------------------------------------------------------------
    ctx.save();
    cells.forEach((color, key) => {
      ctx.fillStyle = color;
      ctx.fillRect(ax + ((key & 63) - 16), ay + ((key >> 6) - 16), 1, 1);
    });
    ctx.restore();
  },
};
