// Read-only reproduction of root's validator findings. Run at6d4ac7b with its retained smoke.
// Later corrected reducers should reject the four deliberately inconsistent summaries.
import {readFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {fileURLToPath} from 'node:url';
import {resolve} from 'node:path';
import {verifyArm} from '../../../scripts/reduce-quiet-compare.mjs';
const root = fileURLToPath(new URL('../../../', import.meta.url));
const directory = resolve(process.argv[2] || `${root}/captures/quiet-smoke-probe`);
const raw = async arm => JSON.parse(await readFile(`${directory}/seed-1/${arm}/summary.json`, 'utf8'));
const source = await readFile(`${root}/scripts/reduce-quiet-compare.mjs`);
const cases = [
  ['untouched smoke summary (low-level gate only)', 'off_nocare', () => {}, true],
  ['inflated drift limit hides a material error', 'off_nocare', a => {
    a.pre_intervention_baseline.limits.material = 1e20;
    a.max_absolute_drift.material = 1e10;
  }, false],
  ['no-care arm has an unreceipted Feed ledger', 'off_nocare', a => {
    a.care.ledgers.feed_material_in += 2;
    a.care.ledgers.feed_energy_in += 4;
  }, false],
  ['Feed has a wrong absolute application tick', 'off_feed', a => {
    a.care.applied_at_tick = 1;
  }, false],
  ['Feed receipt has a rejected outcome', 'off_feed', a => {
    a.care.receipts[0].outcome = {Rejected: {reason: 'forensic_fixture'}};
  }, false],
];
const results = [];
for (const [name, arm, mutate, shouldAccept] of cases) {
  const a = await raw(arm);
  mutate(a);
  let accepted = true, error = null;
  try { verifyArm(a, a.planned_ticks, a.closing_tick - a.elapsed_ticks, arm); }
  catch (e) { accepted = false; error = e.message; }
  results.push({name, accepted, shouldAccept, gap: accepted !== shouldAccept, error});
}
console.log(JSON.stringify({
  scope: 'read-only mutation probes against copied in-memory summaries, not a biological run or completed-horizon certification',
  reducer_sha256: createHash('sha256').update(source).digest('hex'), results,
}, null, 2));
