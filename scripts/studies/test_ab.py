"""Small fake census executables and authored configs; no tuned defaults."""

from contextlib import redirect_stdout
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parent))
import ab


class ABTests(unittest.TestCase):
    def setUp(self):
        # Stay in this worktree; in particular never make a target/worktree in /tmp.
        self.temp = tempfile.TemporaryDirectory(prefix=".ab-test-", dir=Path(__file__).resolve().parents[2])
        self.root = Path(self.temp.name)
        self.wrapper = Path(ab.__file__).resolve().parents[1] / "run-study.py"
        self.trees = {}
        for name in ("main", "branch"):
            tree = self.root / name
            (tree / "config").mkdir(parents=True)
            (tree / "config/input.toml").write_text('[world]\nwidth=8\n[flora]\nrate=2\n[fauna]\nrate=3\n')
            subprocess.run(["git", "init", "-q", str(tree)], check=True)
            subprocess.run(["git", "-C", str(tree), "add", "config/input.toml"], check=True)
            subprocess.run(["git", "-C", str(tree), "-c", "user.name=Test", "-c", "user.email=test@example.invalid",
                            "commit", "-qm", "authored fixture"], check=True)
            self.trees[name] = tree
        self.binary = self.root / "census"
        self.binary.write_text('''#!/bin/sh
for arg in "$@"; do
  case "$arg" in
    config=*) config=${arg#config=} ;;
    expand=*) expand=${arg#expand=} ;;
    params=*) params=${arg#params=} ;;
    run=*) run=${arg#run=} ;;
    load=*) load=${arg#load=} ;;
  esac
done
if [ -n "$expand" ]; then
  printf 'gene = []\\n' > "$expand"
  exit 0
fi
if [ -n "$params" ]; then
  cat "$config" > "$params"
  exit 0
fi
mkdir -p "$run/ckpt/m000020"
printf '20\\n' > "$run/ckpt/m000020/minute"
printf 'minute=20 state=done\\n' > "$run/status"
touch "$run/DONE"
if [ -n "$load" ]; then
  printf 'sim_min,count\\n20,7\\n24,9\\n'
else
  count=7
  case "$config" in */branch/*) count=8 ;; esac
  printf 'sim_min,count\\n0,0\\n20,%s\\n' "$count"
fi
''')
        self.binary.chmod(0o755)

    def tearDown(self):
        self.temp.cleanup()

    def arguments(self, out, *extra):
        return [str(out), '--arm', f'main={self.trees["main"]}', '--arm', f'branch={self.trees["branch"]}',
                '--bin', f'main={self.binary}', '--bin', f'branch={self.binary}',
                '--commit', 'main=HEAD', '--commit', 'branch=HEAD',
                '--config-path', 'config/input.toml', *extra]

    def preflight(self, *extra):
        out = self.root / 'out'
        with redirect_stdout(io.StringIO()) as stream:
            result = ab.main(self.arguments(out, '--preflight-only', *extra))
        return result, stream.getvalue(), out

    def change(self):
        config = self.trees['branch'] / 'config/input.toml'
        config.write_text(config.read_text().replace('rate=3', 'rate=5'))

    def test_identical_binary_and_resolved_config_aborts(self):
        code, log, out = self.preflight()
        self.assertEqual(code, 1)
        self.assertIn('identical binary and expanded config', log)
        self.assertIn('identical', (out / 'DONE').read_text())

    def test_wrong_tree_config_is_refused(self):
        code, log, _ = self.preflight('--config', f'branch={self.trees["main"] / "config/input.toml"}')
        self.assertEqual(code, 1)
        self.assertIn('different tree than binary source', log)

    def test_symlink_config_escape_is_refused(self):
        (self.trees['branch'] / 'config/escape.toml').symlink_to(self.trees['main'] / 'config/input.toml')
        code, log, _ = self.preflight('--config', 'branch=config/escape.toml')
        self.assertEqual(code, 1)
        self.assertIn('different tree', log)

    def test_expected_key_absent_aborts(self):
        self.change()
        code, log, _ = self.preflight('--expect', 'flora.rate,fauna.rate')
        self.assertEqual(code, 1)
        self.assertIn('expected keys absent from diff: flora.rate', log)

    def test_expected_changed_key_passes_and_manifest_has_identity(self):
        self.change()
        code, log, out = self.preflight('--expect', 'fauna.rate')
        self.assertEqual(code, 0, log)
        self.assertIn('fauna.rate: 3 → 5', log)
        manifest = json.loads((out / 'ab.json').read_text())
        for name, arm in manifest['arms'].items():
            self.assertEqual(arm['commit'], ab.git(self.trees[name], 'rev-parse', 'HEAD').decode().strip())
            self.assertEqual(arm['dirty'], name == 'branch')
            self.assertEqual(arm['binary_sha256'], ab.sha256(self.binary))
            self.assertEqual(arm['config_sha256'], ab.sha256(self.trees[name] / 'config/input.toml'))
            self.assertEqual([c['kind'] for c in arm['commands']], ['expand', 'parameters'])
            self.assertIn('--slot', arm['commands'][0]['argv'])
            self.assertIn('chrt -i 0 nice -n 19', arm['commands'][0]['line'])
        self.assertIn('preflight-only', (out / 'DONE').read_text())

    def test_different_binary_same_config_passes_without_expect(self):
        second = self.root / 'census2'
        second.write_text(self.binary.read_text() + '\n# distinct executable\n')
        second.chmod(0o755)
        out = self.root / 'out'
        arguments = self.arguments(out, '--preflight-only')
        arguments[arguments.index(f'branch={self.binary}')] = f'branch={second}'
        with redirect_stdout(io.StringIO()):
            self.assertEqual(ab.main(arguments), 0)

    def test_historical_commit_reads_its_config_without_checkout(self):
        old = ab.git(self.trees['main'], 'rev-parse', 'HEAD').decode().strip()
        config = self.trees['main'] / 'config/input.toml'
        config.write_text(config.read_text().replace('rate=3', 'rate=9'))
        subprocess.run(['git', '-C', str(self.trees['main']), 'commit', '-qam', 'later fixture'], check=True)
        current = ab.git(self.trees['main'], 'rev-parse', 'HEAD')
        self.change()
        args = self.arguments(self.root / 'out', '--preflight-only', '--expect', 'fauna.rate')
        args[args.index('main=HEAD')] = f'main={old}'
        with redirect_stdout(io.StringIO()) as log:
            self.assertEqual(ab.main(args), 0, log.getvalue())
        self.assertEqual(ab.git(self.trees['main'], 'rev-parse', 'HEAD'), current)
        manifest = json.loads((self.root / 'out/ab.json').read_text())
        self.assertEqual(manifest['diff']['branch']['fauna.rate'], [3, 5])

    def rows(self, counts, missing=False):
        stage = self.root / 'early'
        for name, values in counts.items():
            for seed, value in enumerate(values, 1):
                path = stage / name / f's{seed}'
                path.mkdir(parents=True)
                (path / 'census.csv').write_text('sim_min,count\n' + ('' if missing else f'20,{value}\n'))
        return stage

    def test_early_identical_writes_abort_and_done(self):
        stage = self.rows({'main': ['7', '4'], 'branch': ['7.0', '4']})
        with redirect_stdout(io.StringIO()):
            self.assertFalse(ab.early_read(self.root, stage, ['main', 'branch'], [1, 2], ['count'], 20))
        self.assertTrue((self.root / 'ABORTED.early-identical').exists())
        self.assertIn('early-identical', (self.root / 'DONE').read_text())

    def test_one_differing_seed_continues(self):
        stage = self.rows({'main': ['7', '4'], 'branch': ['7', '5']})
        with redirect_stdout(io.StringIO()) as log:
            self.assertTrue(ab.early_read(self.root, stage, ['main', 'branch'], [1, 2], ['count'], 20))
        self.assertIn('main=4 | branch=5', log.getvalue())
        self.assertFalse((self.root / 'DONE').exists())
        self.assertFalse((self.root / 'ABORTED.early-identical').exists())

    def test_missing_early_row_or_column_refused(self):
        stage = self.rows({'main': ['7'], 'branch': ['7']}, missing=True)
        with self.assertRaisesRegex(ab.GuardError, 'no census row'):
            ab.early_read(self.root, stage, ['main', 'branch'], [1], ['count'], 20)
        with self.assertRaisesRegex(ab.GuardError, 'missing sim_min/expected metric'):
            ab.early_read(self.root, stage, ['main', 'branch'], [1], ['typo'], 20)

    def test_actual_short_stages_continue_only_after_all_seeds_checked(self):
        if len(os.sched_getaffinity(0)) < 2:
            self.skipTest('requires one CPU reserved')
        self.change()
        out = self.root / 'out'
        # Exercise worker stages without starting a real systemd unit.
        with redirect_stdout(io.StringIO()) as log:
            code = ab.main(self.arguments(out, '--worker', '--expect', 'fauna.rate', '--expect-metric', 'count',
                                          '--seeds', '1-2', '--hours', '0.4', '--max-parallel', '1'))
        self.assertEqual(code, 0, log.getvalue())
        self.assertIn('seed=2 count: main=7 | branch=8', log.getvalue())
        manifest = json.loads((out / 'ab.json').read_text())
        self.assertEqual(list(manifest['stages']), ['early', 'continuation'])
        for job in manifest['stages']['continuation']:
            self.assertTrue(any(arg.startswith(f'load={out / "early" / job["arm"]}') for arg in job['command']))
            self.assertFalse(any(arg.startswith('config=') for arg in job['command']))
        self.assertIn('ok:', (out / 'DONE').read_text())

    def test_identical_short_stage_never_launches_continuation(self):
        if len(os.sched_getaffinity(0)) < 2:
            self.skipTest('requires one CPU reserved')
        self.change()
        self.binary.write_text(self.binary.read_text().replace('count=8', 'count=7'))
        out = self.root / 'out'
        with redirect_stdout(io.StringIO()) as log:
            code = ab.main(self.arguments(out, '--worker', '--expect-metric', 'count', '--seeds', '1',
                                          '--hours', '0.4', '--max-parallel', '1'))
        self.assertEqual(code, 1, log.getvalue())
        self.assertTrue((out / 'ABORTED.early-identical').exists())
        self.assertFalse((out / 'continuation').exists())
        self.assertIn('early-identical', (out / 'DONE').read_text())

    def test_systemd_detachment_instead_of_session_fork(self):
        out = self.root / 'out'
        with patch.object(ab.subprocess, 'run') as run, redirect_stdout(io.StringIO()):
            self.assertEqual(ab.main(self.arguments(out, '--unit', 'fixture-ab')), 0)
        command = run.call_args.args[0]
        self.assertEqual(command[:4], ['systemd-run', '--user', '--unit=fixture-ab', '--collect'])
        self.assertIn('--worker', command)
        self.assertIn('chrt', command)
        self.assertIn(f'--property=StandardOutput=append:{out / "launch.log"}', command)

    def test_builds_override_shared_target_and_ignore_own_output_in_dirty_flag(self):
        self.change()
        cargo = self.root / 'cargo'
        cargo.write_text('''#!/bin/sh
mkdir -p "$CARGO_TARGET_DIR/release/examples"
cp "$FAKE_CENSUS" "$CARGO_TARGET_DIR/release/examples/voxel_census"
''')
        cargo.chmod(0o755)
        out = self.trees['main'] / 'study-output'
        arguments = [str(out), '--arm', f'main={self.trees["main"]}', '--arm', f'branch={self.trees["branch"]}',
                     '--config-path', 'config/input.toml', '--expect', 'fauna.rate', '--preflight-only']
        environment = {'PATH': str(self.root) + os.pathsep + os.environ['PATH'],
                       'FAKE_CENSUS': str(self.binary), 'CARGO_TARGET_DIR': str(self.root / 'shared-target')}
        with patch.dict(os.environ, environment), redirect_stdout(io.StringIO()) as log:
            code = ab.main(arguments)
        self.assertEqual(code, 0, log.getvalue())
        manifest = json.loads((out / 'ab.json').read_text())
        for name in self.trees:
            self.assertEqual(manifest['arms'][name]['binary'], str(out / 'build' / name / 'release/examples/voxel_census'))
        self.assertFalse(manifest['arms']['main']['dirty'])
        self.assertFalse((self.root / 'shared-target').exists())

    def test_no_early_read_disables_metric_gate(self):
        if len(os.sched_getaffinity(0)) < 2:
            self.skipTest('requires one CPU reserved')
        self.change()
        out = self.root / 'out'
        with redirect_stdout(io.StringIO()) as log:
            code = ab.main(self.arguments(out, '--worker', '--expect-metric', 'missing-column', '--no-early-read',
                                          '--seeds', '1', '--hours', '0.1', '--max-parallel', '1'))
        self.assertEqual(code, 0, log.getvalue())
        self.assertFalse((out / 'early').exists())
        self.assertIn('ok:', (out / 'DONE').read_text())

    def test_registry_arrays_use_declared_source_aliases(self):
        source = self.trees['main'] / 'crates/cubarium-voxel-fauna/src'
        source.mkdir(parents=True)
        (source / 'manifest.rs').write_text('''impl Founder {
    pub const ALL: [Founder; 2] = [Founder::Grazer, Founder::Gnawer];
    pub fn name(self) -> &'static str {
        match self { Founder::Grazer => "grazer", Founder::Gnawer => "gnawer" }
    }
}
''')
        (source / 'lib.rs').write_text('''impl Food {
    pub const ALL: [Food; 2] = [Food::Leaf, Food::Litter];
    pub fn name(self) -> &'static str {
        match self { Food::Leaf => "leaf", Food::Litter => "litter" }
    }
}
''')
        arm = dict(tree=str(self.trees['main']), commit=ab.git(self.trees['main'], 'rev-parse', 'HEAD').decode().strip())
        data = dict(fauna=dict(founders=[dict(core=dict(rate=3), yield_by_food=[4, 5]),
                                       dict(core=dict(rate=6), yield_by_food=[7, 8])]))
        values = ab.resolved_values(data, arm)
        self.assertEqual(values['fauna.founders.grazer.core.rate'], 3)
        self.assertEqual(values['fauna.founders.gnawer.yield_by_food.litter'], 8)

    def test_existing_output_cannot_be_overwritten(self):
        out = self.root / 'out'
        out.mkdir()
        (out / 'DONE').write_text('earlier result')
        with redirect_stdout(io.StringIO()):
            self.assertEqual(ab.main(self.arguments(out, '--preflight-only')), 1)
        self.assertEqual((out / 'DONE').read_text(), 'earlier result')


if __name__ == '__main__':
    unittest.main()
