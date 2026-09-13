import test from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp, mkdir, readFile, rm, symlink, writeFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join, resolve} from 'node:path';
import {spawnSync} from 'node:child_process';

for (const mode of ['prepare', 'capture']) {
  test(`rain study ${mode} refuses existing directories and symlinks without touching evidence`, async () => {
    const directory = await mkdtemp(join(tmpdir(), 'cubarium-rain-study-safety-'));
    try {
      const evidence = join(directory, 'evidence');
      await mkdir(evidence);
      const sentinel = join(evidence, 'retained.txt');
      await writeFile(sentinel, 'original evidence\n');
      const linked = join(directory, 'linked');
      await symlink(evidence, linked);
      for (const target of [evidence, linked]) {
        const env = {...process.env, [mode === 'prepare' ? 'RAIN_STUDY_SRC' : 'RAIN_STUDY_OUT']: target};
        const result = spawnSync('bash', [resolve('art/studies/rain-response/run.sh'), mode],
          {env, encoding: 'utf8', timeout: 5000});
        assert.equal(result.error, undefined);
        assert.equal(result.status, 1);
        assert.match(result.stderr, /refusing existing/);
        assert.equal(await readFile(sentinel, 'utf8'), 'original evidence\n');
      }
    } finally {
      await rm(directory, {recursive: true});
    }
  });
}
