// Freeze all twelve prescribed prey openings using a PRE-HUNTER schema9 runner.
// No filtering, care, display transport, live state, or dependency installation.
import { createHash } from 'node:crypto';
import { constants } from 'node:fs';
import * as fs from 'node:fs/promises';
import { resolve, join } from 'node:path';
import { pathToFileURL } from 'node:url';
import { spawn } from 'node:child_process';

export const OPENING_TICK = 144000;

export function parseArgs(args) {
  if (args.length !== 2 || args.some(s => !s || s.startsWith('--')))
    throw new Error('Usage: node scripts/prepare-hunter-worlds.mjs PRE_HUNTER_RUNNER NEW_OUTPUT_DIRECTORY');
  const [runner, out] = args.map(s => resolve(s));
  if (runner === out) throw new Error('Runner and output directory must differ');
  return { runner, out };
}

export function inspectSnapshot(bytes) {
  if (bytes.length < 22 || bytes.subarray(0, 4).toString() !== 'CUBW')
    throw new Error('Invalid snapshot header');
  const schema = bytes.readUInt32LE(4);
  if (schema !== 9) throw new Error(`Expected pre-hunter schema9, received${schema}`);
  const nameLength = bytes.readUInt16LE(8), offset = 10 + nameLength;
  if (offset + 12 > bytes.length) throw new Error('Truncated snapshot header');
  const payload = bytes.subarray(offset + 12);
  if (bytes.readBigUInt64LE(offset) !== BigInt(payload.length))
    throw new Error('Snapshot payload length mismatch');
  let crc = 0xffffffff, hash = 0xcbf29ce484222325n;
  for (const value of payload) {
    crc ^= value;
    for (let bit = 0; bit < 8; bit++) crc = (crc >>> 1) ^ ((crc & 1) ? 0xedb88320 : 0);
    hash = BigInt.asUintN(64, (hash ^ BigInt(value)) * 0x100000001b3n);
  }
  crc = (crc ^ 0xffffffff) >>> 0;
  if (crc !== bytes.readUInt32LE(offset + 8)) throw new Error('Snapshot checksum mismatch');
  return { schema, build: bytes.subarray(10, offset).toString(), payload_bytes: payload.length,
    crc32: crc, state_hash: hash.toString(), sha256: createHash('sha256').update(bytes).digest('hex') };
}

export function parseFinalTelemetry(text) {
  const lines = text.trim().split('\n');
  // Preserve JSON's u64 hashes BEFORE parsing; JS numbers cannot represent them.
  const row = JSON.parse(lines.at(-1).replace(/"((?:state|ecology)_hash)":\s*(\d+)/g, '"$1":"$2"'));
  if (row.tick !== OPENING_TICK || !Number.isInteger(row.population) || row.population < 0)
    throw new Error('Missing exact two-hour telemetry endpoint');
  if (!Array.isArray(row.population_by_form) || row.population_by_form.length !== 8
      || row.population_by_form.some(n => !Number.isInteger(n) || n < 0)
      || row.population_by_form.reduce((a, b) => a + b, 0) !== row.population)
    throw new Error('Invalid opening form census');
  if (row.care_admitted_seq !== 0) throw new Error('Preparation must not admit care');
  return row;
}

export function stratify(openings) {
  if (openings.length !== 12 || new Set(openings.map(o => o.seed)).size !== 12
      || openings.some(o => !Number.isInteger(o.seed) || o.seed < 1 || o.seed > 12
        || !Number.isInteger(o.population) || o.population < 0))
    throw new Error('Stratification requires all prescribed seeds1–12, including empty populations');
  return [...openings].sort((a, b) => a.population - b.population || a.seed - b.seed)
    .map((o, rank) => ({ ...o, population_rank: rank + 1,
      population_stratum: ['low', 'middle', 'high'][Math.floor(rank / 4)] }));
}

async function runSeed(runner, state, log) {
  const handle = await fs.open(log, 'wx');
  try {
    await new Promise((ok, fail) => {
      const child = spawn(runner, ['run', '--sink', 'none', '--fresh', '--seed',
        String(state.seed), '--speed', '0', '--seconds', '7200', '--state', state.path],
        { stdio: ['ignore', handle.fd, handle.fd] });
      child.on('error', fail);
      child.on('exit', (code, signal) => code === 0 ? ok() : fail(
        new Error(`Seed${state.seed} runner exited${code}, signal${signal}; inspect${log}`)));
    });
  } finally { await handle.close(); }
}

export async function prepare({ runner, out }) {
  await fs.access(runner, constants.R_OK | constants.X_OK);
  const executable = await fs.readFile(runner);
  // Must be a brand-new destination. Never fresh-start, overwrite, or delete an
  // existing state directory; a partial preparation is retained after failure.
  await fs.mkdir(out);
  const frozen = join(out, 'pre-hunter-runner');
  await fs.copyFile(runner, frozen, constants.COPYFILE_EXCL);
  const runnerHash = createHash('sha256').update(executable).digest('hex');
  if (createHash('sha256').update(await fs.readFile(frozen)).digest('hex') !== runnerHash)
    throw new Error('Runner changed while being frozen; incomplete destination retained');
  const manifest = {
    kind: 'pre-hunter-cohort-preparation', complete: false,
    prescribed_seeds: Array.from({ length: 12 }, (_, i) => i + 1),
    opening_tick: OPENING_TICK, simulated_age_seconds: 7200,
    runner_sha256: runnerHash,
    note: 'Age-mature openings, not a stability claim. All seeds retained; no care or hunter. Preparation census is not an independent conservation audit.',
    openings: [],
  };
  async function record() {
    await fs.writeFile(join(out, 'manifest.next.json'), JSON.stringify(manifest, null, 2) + '\n');
    await fs.rename(join(out, 'manifest.next.json'), join(out, 'manifest.json'));
  }
  await record();
  for (const seed of manifest.prescribed_seeds) {
    const state = join(out, `seed-${seed}`);
    await fs.mkdir(state);
    await runSeed(frozen, { seed, path: state }, join(out, `seed-${seed}.log`));
    const snapshot = join(state, `world-${OPENING_TICK}.cubw`);
    const metadata = inspectSnapshot(await fs.readFile(snapshot));
    const telemetry = parseFinalTelemetry(await fs.readFile(join(state, 'telemetry.jsonl'), 'utf8'));
    if (telemetry.state_hash !== metadata.state_hash)
      throw new Error(`Seed${seed} telemetry and saved snapshot describe different states`);
    manifest.openings.push({ seed, population: telemetry.population,
      population_by_form: telemetry.population_by_form, snapshot,
      ...metadata, ecology_hash: telemetry.ecology_hash, telemetry });
    await record();
    console.log(`Prepared seed${seed}/12: tick${OPENING_TICK}, population${telemetry.population}, forms${telemetry.population_by_form.slice(0, 4).join('/')}`);
  }
  manifest.openings = stratify(manifest.openings);
  manifest.complete = true;
  await record();
  console.log(`Prepared all12 openings without filtering: ${join(out, 'manifest.json')}`);
  await addInitialSnapshots(out);
}

// Separate entry point also finishes cohorts prepared by the first version of
// this script. It never changes aged worlds or their snapshots. The same frozen
// deterministic runner reconstructs each exact tick-zero seed before any step.
export async function addInitialSnapshots(out) {
  out = resolve(out);
  const manifestPath = join(out, 'manifest.json');
  const manifest = JSON.parse(await fs.readFile(manifestPath, 'utf8'));
  if (manifest.kind !== 'pre-hunter-cohort-preparation' || !manifest.complete)
    throw new Error('Initial archive requires a completed aged cohort');
  stratify(manifest.openings);
  const runner = join(out, 'pre-hunter-runner');
  if (createHash('sha256').update(await fs.readFile(runner)).digest('hex') !== manifest.runner_sha256)
    throw new Error('Frozen runner no longer matches cohort provenance');
  const initialDir = join(out, 'initial-seeds');
  await fs.mkdir(initialDir); // Refuse to overwrite any earlier archive.
  const initials = [];
  for (let seed = 1; seed <= 12; seed++) {
    const dir = join(initialDir, `seed-${seed}`);
    await fs.mkdir(dir);
    const log = await fs.open(join(initialDir, `seed-${seed}.log`), 'wx');
    try {
      await new Promise((ok, fail) => {
        // Positive sub-tick duration rounds to zero ticks in this pinned runner;
        // --seconds0 means unlimited and must NEVER be substituted here.
        const child = spawn(runner, ['run', '--sink', 'none', '--fresh', '--seed',
          String(seed), '--speed', '0', '--seconds', '0.001', '--state', dir],
          { stdio: ['ignore', log.fd, log.fd] });
        child.on('error', fail);
        child.on('exit', (code, signal) => code === 0 ? ok() : fail(
          new Error(`Initial seed${seed} failed: code${code}, signal${signal}`)));
      });
    } finally { await log.close(); }
    const snapshot = join(dir, 'world-0.cubw');
    initials.push({ seed, tick: 0, snapshot, ...inspectSnapshot(await fs.readFile(snapshot)) });
  }
  const archive = { runner_sha256: manifest.runner_sha256,
    note: 'Tick-zero worlds reconstructed by the same frozen deterministic runner and seed, with no configuration override; aged snapshots untouched.',
    initials };
  await fs.writeFile(join(out, 'initial-manifest.json'), JSON.stringify(archive, null, 2) + '\n', { flag: 'wx' });
  console.log(`Archived all12 exact tick-zero worlds: ${join(out, 'initial-manifest.json')}`);
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  const args = process.argv.slice(2);
  const task = args.length === 2 && args[0] === '--initials'
    ? addInitialSnapshots(args[1]) : prepare(parseArgs(args));
  task.catch(error => { console.error(error); process.exitCode = 1; });
}
