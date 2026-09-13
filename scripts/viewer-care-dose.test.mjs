// Exercise the actual inline care script, without Three.js, a network, or a world.
// This is DOM/transport logic coverage, not browser layout or durable-core evidence.
import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import vm from 'node:vm';

const html = readFileSync(new URL('../crates/cubarium/src/sink/web/index.html', import.meta.url), 'utf8');
const start = html.indexOf('const CARE_HEADERS =');
const end = html.indexOf('\nregister();\npollCare();\nsetInterval(pollCare, STATUS_MS);', start);
assert(start >= 0 && end > start, 'actual viewer care script boundaries exist');
const source = html.slice(start, end);
const capability = {version: 1, min_permille: 250, max_permille: 2000, default_permille: 1000};
const flush = () => new Promise(resolve => setImmediate(resolve));

function harness() {
  const elements = new Map();
  function element() {
    return {
      textContent: '', className: '', dataset: {}, children: [], disabled: true,
      value: '1000', options: [500, 1000, 1500].map(n => ({value: String(n), disabled: false})),
      addEventListener() {},
      getContext: () => ({}),
      prepend(child) { this.children.unshift(child); },
      removeChild(child) { this.children.splice(this.children.indexOf(child), 1); },
      get lastChild() { return this.children.at(-1); },
    };
  }
  const el = id => {
    if (!elements.has(id)) elements.set(id, element());
    return elements.get(id);
  };
  const requests = [];
  let status = {enabled: true, epoch: 'test', care: 'ready', world_tick: 10, receipts: []};
  let response = {status: 202, body: {seq: 1, apply_after_tick: 10}};
  let failSubmit = false;
  const context = vm.createContext({
    document: {getElementById: el, createElement: element},
    netCanvas: {...element(), width: 768, height: 384},
    performance: {now: () => 0},
    fetch: async (url, options) => {
      if (url === '/care/status') return {ok: true, json: async () => status};
      if (url === '/care/register') return {ok: true, json: async () => ({client: 'test.1', epoch: 'test'})};
      assert.equal(url, '/care');
      requests.push({url, method: options.method, headers: options.headers, payload: JSON.parse(options.body)});
      if (failSubmit) throw new Error('connection lost');
      return {status: response.status, json: async () => response.body};
    },
  });
  vm.runInContext(source, context, {filename: 'viewer-inline-care.js'});
  const run = js => vm.runInContext(js, context);
  run('careEnabled = true; careClient = "test.1"; careEpoch = "test"; careTarget = {face: 4, u: 32, v: 32};');
  return {
    el, requests, run,
    async poll(dose, extra = {}) {
      status = {...status, ...extra, dose};
      run('pollCare()');
      await flush();
    },
    async send(kind, amount) {
      if (amount !== undefined) el('careDose').value = String(amount);
      run(`send(${JSON.stringify(kind)})`);
      await flush();
    },
    response(value) { response = value; },
    fail() { failSubmit = true; },
  };
}

test('markup offers explicit amounts behind the initially closed care panel', () => {
  assert.match(html, /<details class="panel" id="carePanel">/);
  assert.match(html, /<select id="careDose" disabled aria-describedby="careDoseNote">/);
  assert.match(html, /<option value="1000" selected>Standard/);
  assert.match(html, /<option value="500">Gentle/);
  assert.match(html, /<option value="1500">Generous/);
});

test('legacy host offers standard-only and receives the old payload', async () => {
  const h = harness();
  await h.poll(undefined);
  assert.equal(h.el('careDose').disabled, true);
  assert.match(h.el('careDoseNote').textContent, /standard amounts only/);
  await h.send('feed', 1500); // Even a programmatically changed disabled select cannot raise it.
  assert.equal(h.requests.length, 1);
  assert.equal(Object.hasOwn(h.requests[0].payload, 'dose_permille'), false);
  assert.match(h.el('careRows').children[0].textContent, /feed \(standard\)/);
});

test('negotiated host receives exactly the selected integer for each action', async () => {
  for (const [kind, amount] of [['feed', 500], ['rain', 1000], ['clean', 1500]]) {
    const h = harness();
    await h.poll(capability);
    assert.equal(h.el('careDose').disabled, false);
    await h.send(kind, amount);
    assert.deepEqual(h.requests[0].payload, {
      client: 'test.1', request: 1, kind, target: {face: 4, u: 32, v: 32}, dose_permille: amount,
    });
    assert.equal(h.requests[0].method, 'POST');
    assert.equal(h.requests[0].headers['X-Cubarium-Care'], '1');
    assert.match(h.el('careRows').children[0].textContent, /accepted as seq 1 at boundary 10/);
    assert.doesNotMatch(h.el('careRows').children[0].textContent, /m fed|m removed/);
  }
});

test('selection and target changes cannot rewrite an in-flight request', async () => {
  const h = harness();
  await h.poll(capability);
  h.el('careDose').value = '500';
  h.run('send("feed"); careTarget.u = 7;');
  h.el('careDose').value = '1500';
  await flush();
  assert.equal(h.requests[0].payload.dose_permille, 500);
  assert.equal(h.requests[0].payload.target.u, 32);
  assert.match(h.el('careRows').children[0].textContent, /feed \(gentle\)/);
});

test('receipt uses server amount and quantities, not the current selection', async () => {
  const h = harness();
  await h.poll(capability);
  h.el('careDose').value = '1500';
  await h.poll(capability, {receipts: [{client: 'test.1', request: 3, kind: 'feed',
    dose_permille: 500, state: 'applied', apply_after_tick: 10, applied: {material_in: 1.5, cells: 5}}]});
  const row = h.el('careRows').children[0].textContent;
  assert.match(row, /feed \(gentle\).*applied/);
  assert.match(row, /1.50 m fed/);
  assert.doesNotMatch(row, /generous/);
});

test('unknown, malformed or absent capability resets to standard', async () => {
  const invalid = [undefined, null, {}, {...capability, version: 2},
    {...capability, min_permille: '250'}, {...capability, max_permille: 2001},
    {...capability, min_permille: 0}, {...capability, default_permille: 500},
    {...capability, min_permille: 1100}];
  for (const cap of invalid) {
    const h = harness();
    await h.poll(capability);
    h.el('careDose').value = '1500';
    await h.poll(cap);
    assert.equal(h.el('careDose').value, '1000');
    assert.equal(h.el('careDose').disabled, true);
    await h.send('rain');
    assert.equal(Object.hasOwn(h.requests[0].payload, 'dose_permille'), false);
  }
});

test('narrower supported bounds disable unavailable presets and reset selection', async () => {
  const h = harness();
  await h.poll(capability);
  h.el('careDose').value = '1500';
  await h.poll({...capability, min_permille: 750, max_permille: 1250});
  assert.equal(h.el('careDose').value, '1000');
  assert.deepEqual(h.el('careDose').options.map(o => o.disabled), [true, false, true]);
  await h.send('clean', 1500);
  assert.equal(h.requests.length, 0);
});

test('invalid selector values never submit a coerced dose', async () => {
  for (const value of ['NaN', '', '0', '1000.5', '2001', '-1']) {
    const h = harness();
    await h.poll(capability);
    await h.send('feed', value);
    assert.equal(h.requests.length, 0);
    assert.match(h.el('careNote').textContent, /choose an amount/);
  }
});

test('disabled care clears negotiated capability and prevents submission', async () => {
  const h = harness();
  await h.poll(capability);
  await h.poll(capability, {enabled: false});
  await h.send('feed', 1500);
  assert.equal(h.requests.length, 0);
  assert.equal(h.el('careDose').disabled, true);
});

test('refusals and lost connections do not claim a deposit', async () => {
  const h = harness();
  await h.poll(capability);
  h.response({status: 429, body: {error: 'cooldown'}});
  await h.send('feed', 500);
  assert.match(h.el('careRows').children[0].textContent, /feed \(gentle\).*429 cooldown/);
  h.fail();
  await h.send('rain', 1500);
  assert.match(h.el('careRows').children[0].textContent, /rain \(generous\).*unknown after a lost connection/);
});
