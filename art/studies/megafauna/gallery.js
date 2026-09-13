const params = new URLSearchParams(location.search);
const modes = ['rest', 'move', 'hunt', 'bud'];
let mode = modes.includes(params.get('mode')) ? params.get('mode') : 'rest';
let time = Math.max(0, Number(params.get('t')) || 0);
let paused = params.has('t');
let last = null;
const cards = [];
function habitat(ctx, plain) {
  ctx.fillStyle = '#090b20'; ctx.fillRect(0, 0, 64, 64);
  if (!plain) {
    ctx.fillStyle = '#152039'; ctx.fillRect(0, 46, 64, 18);
    ctx.fillStyle = '#25405a';
    for (const [x,y,w] of [[0,45,13],[15,43,12],[27,44,13],[42,43,10],[55,45,9]]) ctx.fillRect(x,y,w,2);
    for (const [x,h] of [[5,14],[17,9],[49,12],[59,20]]) {
      ctx.fillStyle = '#214450'; ctx.fillRect(x,45-h,1,h);
      ctx.fillStyle = '#355f69'; ctx.fillRect(x-3,44-h,7,2);
      ctx.fillStyle = '#4d7180'; ctx.fillRect(x-1,43-h,3,1);
    }
    ctx.fillStyle = '#392c53'; ctx.fillRect(25,48,5,2); ctx.fillRect(54,52,4,1);
  }
  // A common lantern-shaped 8px body for scale, not a simulated animal.
  ctx.fillStyle = '#193e68'; ctx.fillRect(7,54,7,4); ctx.fillRect(8,53,5,6);
  ctx.fillStyle = '#568fa6'; ctx.fillRect(8,54,4,2);
  ctx.fillStyle = '#8ee2cf'; ctx.fillRect(8,54,2,1);
  ctx.fillStyle = '#f5d7a1'; ctx.fillRect(12,55,1,1);
}
function addCard(candidate, key) {
  const article = document.createElement('article'); article.className = 'card';
  const title = document.createElement('h2'); title.textContent = candidate.name;
  const author = document.createElement('div'); author.className = 'author'; author.textContent = candidate.author + (key === 'fable' ? ' · Wrysk’s preferred direction' : ' · retained alternate');
  const views = document.createElement('div'); views.className = 'views';
  const main = document.createElement('canvas'); main.width = main.height = 64; main.className = 'large'; main.id = key;
  main.setAttribute('aria-label', candidate.name + ' enlarged animation study');
  const small = document.createElement('canvas'); small.width = small.height = 64; small.className = 'native';
  small.setAttribute('aria-label', candidate.name + ' native 64 pixel study');
  for (const [canvas, label] of [[main, '5× / nearest neighbour'], [small, '1× / native']]) {
    const group = document.createElement('div'); group.append(canvas);
    const caption = document.createElement('div'); caption.className = 'caption'; caption.textContent = label; group.append(caption); views.append(group);
  }
  const desc = document.createElement('p'); desc.className = 'description'; desc.textContent = candidate.description;
  article.append(title, author, views, desc); document.querySelector('#cards').append(article);
  cards.push({ candidate, main, small });
}
for (const key of ['root', 'fable']) {
  try {
    const { candidate } = await import(`./${key}.js`);
    if (!candidate || typeof candidate.draw !== 'function') throw new Error('Invalid candidate module');
    addCard(candidate, key);
  } catch (error) {
    const placeholder = document.createElement('article'); placeholder.className = 'card pending';
    placeholder.textContent = `${key === 'fable' ? 'Fable' : 'Codex'} candidate unavailable — reload when ready.`;
    document.querySelector('#cards').append(placeholder); console.error(error);
  }
}
function draw() {
  for (const card of cards) {
    const ctx = card.main.getContext('2d');
    ctx.setTransform(1,0,0,1,0,0); ctx.globalAlpha = 1; ctx.globalCompositeOperation = 'source-over';
    habitat(ctx, document.querySelector('#habitat').value === 'plain');
    card.candidate.draw(ctx, {time, mode, x:32, y:38});
    const small = card.small.getContext('2d'); small.clearRect(0,0,64,64); small.drawImage(card.main,0,0);
  }
  document.querySelector('#clock').textContent = `Study time ${time.toFixed(2)} s · ${mode}`;
}
function controls() {
  for (const button of document.querySelectorAll('[data-mode]')) button.setAttribute('aria-pressed', String(button.dataset.mode === mode));
  document.querySelector('#pause').textContent = paused ? 'Play' : 'Pause';
}
for (const button of document.querySelectorAll('[data-mode]')) button.onclick = () => {mode = button.dataset.mode; time = 0; controls(); draw();};
document.querySelector('#pause').onclick = () => {paused = !paused; controls();};
document.querySelector('#restart').onclick = () => {time = 0; draw();};
document.querySelector('#habitat').onchange = draw;
window.study = { set(t, m = mode) {time = Math.max(0, Number(t) || 0); if(modes.includes(m)) mode = m; paused = true; controls(); draw();}, candidates: cards.map(c => c.candidate.name) };
document.documentElement.dataset.ready = String(cards.length);
document.querySelector('#status').textContent = `${cards.length}/2 candidates loaded · art only`;
controls(); draw();
requestAnimationFrame(function frame(now) {
  if (!paused && last !== null) time += Math.min(0.1, (now - last) / 1000);
  last = now; draw(); requestAnimationFrame(frame);
});
