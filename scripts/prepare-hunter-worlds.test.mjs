import test from 'node:test';
import assert from 'node:assert/strict';
import * as fs from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { parseArgs, inspectSnapshot, parseFinalTelemetry, stratify, prepare, addInitialSnapshots } from './prepare-hunter-worlds.mjs';

test('arguments require an explicit executable and new destination', () => {
  for (const args of [[], ['one'], ['a', 'b', 'c'], ['--help', 'b'], ['same', 'same']])
    assert.throws(() => parseArgs(args));
  assert.ok(parseArgs(['runner', 'output']).runner.endsWith('/runner'));
});

test('snapshot identity comes from genuine payload bytes and rejects corruption', async () => {
  const bytes = await fs.readFile(new URL('../crates/cubarium-core/tests/fixtures/pre-hunter-v9-173400.cubw', import.meta.url));
  const metadata = inspectSnapshot(bytes);
  assert.equal(metadata.schema, 9);
  assert.equal(metadata.state_hash, BigInt('0x134f4db0135d8a0a').toString());
  assert.equal(metadata.sha256, 'bd3b4e2ead0298806f49375e82b1c56335b1028d4cfbee7dc7b386a7d24a12a8');
  assert.throws(() => inspectSnapshot(bytes.subarray(0, 20)), /header/);
  assert.throws(() => inspectSnapshot(bytes.subarray(0, bytes.length - 1)), /length/);
  const bad = Buffer.from(bytes); bad[bad.length - 1] ^= 1;
  assert.throws(() => inspectSnapshot(bad), /checksum/);
  const foreign = Buffer.from(bytes); foreign.writeUInt32LE(10, 4);
  assert.throws(() => inspectSnapshot(foreign), /schema9/);
});

test('exact final census preserves u64 hashes and refuses incomplete or cared histories', () => {
  const text = '{"tick":144000,"population":3,"population_by_form":[1,1,1,0,0,0,0,0],"care_admitted_seq":0,"state_hash":18446744073709551615,"ecology_hash":9007199254740993}';
  const row = parseFinalTelemetry(text + '\n');
  assert.equal(row.state_hash, '18446744073709551615');
  assert.equal(row.ecology_hash, '9007199254740993');
  assert.throws(() => parseFinalTelemetry(text.replace('144000', '143900')), /endpoint/);
  assert.throws(() => parseFinalTelemetry(text.replace('"population":3', '"population":4')), /census/);
  assert.throws(() => parseFinalTelemetry(text.replace('"care_admitted_seq":0', '"care_admitted_seq":1')), /care/);
});

test('all twelve seeds are retained including extinction, with deterministic tie strata', () => {
  const openings = Array.from({ length: 12 }, (_, i) => ({ seed: 12 - i, population: 0 }));
  const ranked = stratify(openings);
  assert.deepEqual(ranked.map(o => o.seed), Array.from({ length: 12 }, (_, i) => i + 1));
  assert.equal(ranked.filter(o => o.population_stratum === 'low').length, 4);
  assert.equal(ranked.filter(o => o.population_stratum === 'middle').length, 4);
  assert.equal(ranked.filter(o => o.population_stratum === 'high').length, 4);
  assert.equal(openings[0].seed, 12, 'ranking must not mutate source order');
  assert.throws(() => stratify(openings.slice(1)), /all prescribed seeds/);
  assert.throws(() => stratify([...openings.slice(1), openings[1]]), /all prescribed seeds/);
});

test('existing destination is refused without running or replacing anything', async () => {
  const dir = await fs.mkdtemp(join(tmpdir(), 'cubarium-preparation-guard-'));
  try {
    const marker = join(dir, 'user-marker');
    await fs.writeFile(marker, 'preserve');
    await assert.rejects(prepare({ runner: process.execPath, out: dir }), { code: 'EEXIST' });
    assert.equal(await fs.readFile(marker, 'utf8'), 'preserve');
    assert.deepEqual(await fs.readdir(dir), ['user-marker']);
  } finally { await fs.rm(dir, { recursive: true }); }
});

test('initial archive refuses incomplete cohorts or a changed runner before creating destinations', async () => {
  const dir = await fs.mkdtemp(join(tmpdir(), 'cubarium-initial-guard-'));
  try {
    const manifest = { kind: 'pre-hunter-cohort-preparation', complete: false };
    await fs.writeFile(join(dir, 'manifest.json'), JSON.stringify(manifest));
    await assert.rejects(addInitialSnapshots(dir), /completed aged cohort/);
    manifest.complete = true;
    manifest.openings = Array.from({ length: 12 }, (_, i) => ({ seed: i + 1, population: 24 }));
    manifest.runner_sha256 = 'wrong';
    await fs.writeFile(join(dir, 'manifest.json'), JSON.stringify(manifest));
    await fs.copyFile(process.execPath, join(dir, 'pre-hunter-runner'));
    await assert.rejects(addInitialSnapshots(dir), /provenance/);
    assert.deepEqual((await fs.readdir(dir)).sort(), ['manifest.json', 'pre-hunter-runner']);
  } finally { await fs.rm(dir, { recursive: true }); }
});
