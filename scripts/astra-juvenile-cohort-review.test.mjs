// Independent desired-behavior regressions for 02f84ce. Red cases deliberately
// preserve demonstrated acceptance gaps; no original artifacts are changed.
import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile, writeFile, mkdir, mkdtemp, rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join, resolve} from 'node:path';
import {createHash} from 'node:crypto';
import {spawnSync} from 'node:child_process';
import {checkGates, cohortTotals, boundaryAccounting} from './reduce-juvenile-flow-cohort.mjs';

const load = async path => JSON.parse(await readFile(path, 'utf8'));
const evidence = await load('design/7_Research/assets/hunter-juvenile-flow-cohort-reduction-2026-09-13.json');
const originals = await Promise.all(evidence.arms.map(async arm => {
  const bytes = await readFile(arm.ledger);
  assert.equal(createHash('sha256').update(bytes).digest('hex'), arm.ledger_sha256);
  const dir = join(evidence.sources.retained_cohort, `seed-${arm.seed}`, arm.arm);
  return {report: JSON.parse(bytes), opening: await load(join(dir, 'opening.json')),
    summary: await load(join(dir, 'summary.json'))};
}));

test('retained raw evidence has 29 sound member boundaries and eight scavenging children', () => {
  const members = originals.flatMap(x => x.report.ledger.members);
  assert.equal(members.length, 29);
  assert(members.every(m => boundaryAccounting(m).identity_holds));
  for (const m of members) {
    assert.equal(m.residual.violations, 0);
    for (const field of ['max_structure', 'max_reserve', 'max_energy'])
      assert(Number.isFinite(m.residual[field]) && m.residual[field] >= 0 && m.residual[field] < 1e-9);
    for (const stocks of [m.open_stocks, m.close_stocks, m.death_stocks].filter(Boolean))
      for (const field of ['structure', 'reserve', 'energy'])
        assert(Number.isFinite(stocks[field]) && stocks[field] >= 0);
  }
  assert.equal(evidence.children.filter(c => c.reserve.in_by_source.scavenging > 0).length, 8);
  assert.equal(evidence.children.filter(c => c.reserve.in_total === 0).length, 5);
  assert.equal(evidence.children.filter(c => c.gate.max_reserve > c.reserve.opening).length, 4);
});

test('zero reserve intake means all sources, not zero digestion', () => {
  assert.equal(cohortTotals(evidence.children).children_with_zero_reserve_intake, 5);
});

for (const [name, mutate] of [
  ['member violation hidden by aggregate zero', m => {m.residual.violations = 1;}],
  ['nonfinite member residual', m => {m.residual.max_energy = NaN;}],
  ['negative member residual', m => {m.residual.max_reserve = -1;}],
  ['over-limit member residual', m => {m.residual.max_structure = 1;}],
  ['negative member stock', m => {m.open_stocks.reserve = -1;}],
  ['nonfinite member stock', m => {m.close_stocks.energy = Infinity;}],
]) test(`gates reject ${name}`, () => {
  const {report, opening, summary} = structuredClone(originals[0]);
  mutate(report.ledger.members[0]);
  assert(checkGates(report, opening, summary).length > 0, name);
});

// Tiny copied JSON fixture set; the substitute executable is only a SHA fixture.
// Neither retained snapshots nor the real diagnostic executable are executed.
async function cliFixture(mutate) {
  const dir = await mkdtemp(join(tmpdir(), 'astra-juvenile-reducer-probe-'));
  try {
    const rows = structuredClone(originals);
    const census = await load(evidence.sources.census_reduction);
    await mutate(rows, census);
    const ledgerDir = join(dir, 'ledgers'), cohort = join(dir, 'cohort');
    await mkdir(ledgerDir);
    for (let i = 0; i < rows.length; i++) {
      const {report, opening, summary} = rows[i];
      const arm = join(cohort, `seed-${report.opening.seed}`, report.opening.arm);
      await mkdir(arm, {recursive: true});
      await writeFile(join(arm, 'opening.json'), JSON.stringify(opening));
      await writeFile(join(arm, 'summary.json'), JSON.stringify(summary));
      await writeFile(join(ledgerDir, `${i}.json`), JSON.stringify(report));
    }
    const binary = join(dir, 'not-executed'), censusFile = join(dir, 'census.json');
    await writeFile(binary, 'fixture');
    await writeFile(censusFile, JSON.stringify(census));
    const run = spawnSync(process.execPath, [resolve('scripts/reduce-juvenile-flow-cohort.mjs'),
      '--ledgers', ledgerDir, '--cohort', cohort, '--census', censusFile,
      '--binary', binary, '--binary-sha256', createHash('sha256').update('fixture').digest('hex')],
    {encoding: 'utf8', maxBuffer: 4 * 1024 * 1024});
    assert.ifError(run.error);
    return {status: run.status, result: run.stdout ? JSON.parse(run.stdout) : null, stderr: run.stderr};
  } finally { await rm(dir, {recursive: true, force: true}); }
}

test('CLI fixture control reproduces complete 11-arm/18-child acceptance', async () => {
  const run = await cliFixture(() => {});
  assert.equal(run.status, 0, run.stderr);
  assert.equal(run.result.arms_reduced, 11);
  assert.equal(run.result.cohort.children, 18);
});

for (const [name, mutate] of [
  ['missing expected arm', rows => {rows.pop();}],
  ['duplicate arm', rows => {rows.push(structuredClone(rows[0]));}],
  ['census disagreement', (_rows, census) => {census.juvenile_evidence.children_detail[0].birth_tick++;}],
  ['child boundary mismatch', rows => {
    rows[0].report.ledger.members.find(m => m.origin === 'Descendant').residual.checked_ticks++;
  }],
  ['founder boundary mismatch', rows => {
    rows[0].report.ledger.members.find(m => m.origin !== 'Descendant').residual.checked_ticks++;
  }],
]) test(`CLI refuses ${name}`, async () => {
  const run = await cliFixture(mutate);
  assert.notEqual(run.status, 0, JSON.stringify({name, arms: run.result?.arms_reduced,
    gateFailures: run.result?.arms_with_gate_failures,
    boundary: run.result?.boundary_identity_holds,
    disagreements: run.result?.cross_check_vs_census_reduction.disagreements.length}));
});
