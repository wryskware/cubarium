// Original code-native cutout study by Codex. No simulation behavior is implied.
const C = {
  edge: '#141d43', dark: '#253461', shell: '#466c91', light: '#87c7c7',
  mint: '#b8ead9', violet: '#7852a4', rose: '#c37aaf', eye: '#f4d6a0',
};
function polygon(ctx, points, color) {
  ctx.fillStyle = color;
  ctx.beginPath();
  points.forEach(([x, y], i) => i ? ctx.lineTo(x, y) : ctx.moveTo(x, y));
  ctx.closePath(); ctx.fill();
}
function limb(ctx, points, color, width = 1) {
  ctx.strokeStyle = color; ctx.lineWidth = width; ctx.lineCap = 'round';
  ctx.lineJoin = 'round'; ctx.beginPath();
  points.forEach(([x, y], i) => i ? ctx.lineTo(x, y) : ctx.moveTo(x, y));
  ctx.stroke();
}
function ease(t) { t = Math.max(0, Math.min(1, t)); return t * t * (3 - 2 * t); }
function envelope(t, start, peak, end) {
  return t < peak ? ease((t - start) / (peak - start)) : 1 - ease((t - peak) / (end - peak));
}

export const candidate = {
  name: 'Veilwarden', author: 'Codex',
  description: 'A folded canopy sentinel: plated abdomen, hooked forelimbs, a warm eye. Stillness first; one precise reach when it hunts.',
  draw(ctx, { time = 0, mode = 'rest', x = 32, y = 30 }) {
    const t = Math.max(0, Number.isFinite(time) ? time : 0);
    const walking = mode === 'move';
    const hunt = mode === 'hunt' ? envelope(t % 8, 3.0, 3.9, 4.65) : 0;
    const recoil = mode === 'hunt' ? envelope(t % 8, 4.25, 4.75, 5.7) : 0;
    const breath = Math.sin(t * Math.PI / 3.8);
    const gait = t * Math.PI * 1.25;
    ctx.save(); ctx.translate(x, y);

    // Far legs and folded limb: restrained shadow establishes depth at 64 px.
    for (let i = 0; i < 3; i++) {
      const bx = -6 + i * 3.6;
      const step = walking ? Math.sin(gait + i * 2.2) : 0;
      limb(ctx, [[bx, 1], [bx - 1.6 + step, 4], [bx - 3 + step, 5.5]], C.dark, 1.2);
    }
    limb(ctx, [[3, -1], [6 + hunt * 2, -4 - hunt], [4 + hunt * 7, -5 + hunt]], C.violet, 1.3);

    // Abdomen is long but light, with an empty gap beneath the neck.
    polygon(ctx, [[-10, 0], [-8, -2], [-5, -3], [-1, -2], [2, -1], [3, 1], [0, 3], [-5, 3], [-8, 2]], C.edge);
    polygon(ctx, [[-9, 0], [-7, -1.7], [-4, -2.2], [0, -1], [1.7, 1], [-1, 2], [-6, 2]], C.shell);
    for (let i = 0; i < 3; i++) {
      const bx = -7 + i * 2.5;
      const dy = breath * 0.09 * (3 - i);
      polygon(ctx, [[bx - 1, dy], [bx, -2 + dy], [bx + 2, -1 + dy], [bx + 1, 1 + dy]], i === 0 ? C.violet : C.light);
      limb(ctx, [[bx - 0.5, dy + 1], [bx + 1.7, dy + 1.5]], C.dark, 0.6);
    }
    polygon(ctx, [[0, -1], [3, -2], [6, -1 - hunt * 0.3], [6, 1], [2, 2]], C.dark);
    limb(ctx, [[0, -0.5], [3, -1], [6, 0]], C.light, 1.1);

    // A small forward head gives the whole long silhouette an unambiguous direction.
    ctx.save(); ctx.translate(hunt * 0.65 - recoil * 0.25, -hunt * 0.25);
    polygon(ctx, [[5, -2], [8, -2], [10, -0.5], [9, 1], [6, 1.5], [5, 0]], C.edge);
    polygon(ctx, [[6, -1.5], [8, -1.5], [9, -0.5], [8, 0.5], [6, 0.5]], C.mint);
    ctx.fillStyle = C.eye; ctx.fillRect(8, -1.25, 1, 1);
    limb(ctx, [[7, -2], [5.8, -4.5], [7.3, -5.2]], C.light, 0.65);
    limb(ctx, [[9, 0.5], [10, 1.5], [8.8, 2]], C.rose, 0.7);
    ctx.restore();

    // Near feet hold a stable perch when resting; locomotion has a slow tripod rhythm.
    for (let i = 0; i < 3; i++) {
      const bx = -6 + i * 3.6;
      const step = walking ? Math.sin(gait + i * 2.2 + Math.PI) : 0;
      const lift = walking ? Math.max(0, Math.cos(gait + i * 2.2 + Math.PI)) * 0.85 : 0;
      limb(ctx, [[bx, 1.5], [bx + 1.8 + step * 0.5, 4], [bx + step * 1.3, 6 - lift]], C.shell, 1.35);
      limb(ctx, [[bx + step * 1.3, 6 - lift], [bx + 1.4 + step * 1.3, 6 - lift]], C.light, 0.7);
    }
    // Single deliberate unfurl, with a curved hook; not a blinking attack effect.
    const elbow = [5 + hunt * 1.6, 3 + hunt * 1.6];
    const wrist = [3 + hunt * 7.5, 4 - hunt * 3.5 + recoil * 0.3];
    limb(ctx, [[3, 0], elbow, wrist], C.edge, 2.2);
    limb(ctx, [[3, 0], elbow, wrist], C.light, 1.1);
    limb(ctx, [wrist, [wrist[0] - 1.1, wrist[1] - 1.4]], C.mint, 0.8);

    if (mode === 'bud') {
      // A proposed single cocoon, not a claim that apex reproduction is implemented.
      limb(ctx, [[-6, 2], [-7, 4.5]], C.violet, 0.7);
      polygon(ctx, [[-8, 4], [-6, 4], [-5, 5.5], [-6, 7.5], [-8, 7.5], [-9, 5.5]], C.dark);
      polygon(ctx, [[-8, 5], [-6, 4.8], [-6, 6.5], [-8, 7]], C.violet);
      ctx.globalAlpha = 0.55 + 0.12 * breath;
      limb(ctx, [[-7.5, 5], [-7, 6.5]], C.mint, 0.7);
    }
    ctx.restore();
  },
};
