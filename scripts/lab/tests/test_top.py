"""Dashboard reads fake jobs and status files, never a running daemon."""
import contextlib
import io
import json
from pathlib import Path
import sqlite3
import tempfile
import unittest
from unittest.mock import patch

from labkit.cli import main
from labkit.core import Task
from labkit.store import Paths, Store
from labkit.top import Dashboard, Progress


class TopTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='.top-test-', dir=Path(__file__).resolve().parents[3])
        self.root = Path(self.temp.name)
        self.paths = Paths(self.root / 'state', self.root / 'cache', self.root / 'runs', self.root / 'hosts.toml')
        self.paths.hosts.write_text('[hosts.desktop]\nkind="local"\ncpus=8\nmax_mem_gb=16\n'
                                    '[hosts.eidolon]\nkind="ssh"\ncpus=4\nmax_mem_gb=8\n')
        self.store = Store(self.paths)
        self.dashboard = Dashboard(self.paths)
        self.addCleanup(self.temp.cleanup)
        self.addCleanup(self.store.db.close)
        self.addCleanup(self.dashboard.close)

    def job(self, kind='ab', spec=None, state='running', name='job'):
        spec = spec or dict(hours=2, early_minute=20, early_read=True, expect_metric=['count'],
                            seed_list=[1, 2], arms={'a': {}, 'b': {}})
        out = self.paths.runs / name
        out.mkdir(parents=True, exist_ok=True)
        self.store.x('INSERT INTO jobs(id,kind,state,spec,out,submitted) VALUES(?,?,?,?,?,?)',
                     name, kind, state, json.dumps(spec), str(out), 0)
        return self.store.one('SELECT * FROM jobs WHERE id=?', name)

    def task(self, job, name='early-a-s1', stage='early', state='running', status=None, **extra):
        root = Path(job['out']) / name
        task = Task(name=name, argv=['true'], outputs=str(root), meta={'stage': stage}, **extra)
        self.store.x('INSERT INTO tasks(job,name,seq,state,spec,host,grp,started) VALUES(?,?,?,?,?,?,?,?)',
                     job['id'], name, 0, state, json.dumps(task.to_dict()), 'desktop', 1, 100)
        if status:
            (root / 'run').mkdir(parents=True)
            (root / 'run/status').write_text(status)
        return self.store.one('SELECT * FROM tasks WHERE job=? AND name=?', job['id'], name)

    def test_status_pair_progress_and_eta(self):
        job = self.job()
        row = self.task(job, status='minute=10 end=20 speed=2x state=running')
        first = self.dashboard.task(job, row, 100)
        self.assertEqual(first.fraction, .5)
        self.assertEqual(first.eta, 300)
        root = Path(json.loads(row['spec'])['outputs'])
        (root / 'run/status').write_text('minute=12 end=20 speed=2x')
        second = self.dashboard.task(job, row, 160)
        self.assertEqual(second.fraction, .6)
        self.assertEqual(second.eta, 240)
        (root / 'run/status').write_text('minute=13 end=20 speed=9x')
        self.assertEqual(self.dashboard.task(job, row, 220).eta, 420)
        self.assertEqual(self.dashboard.task(job, row, 222).eta, 420)

    def test_multistage_total_includes_unspawned_continuation(self):
        job = self.job()
        rows = [self.task(job, f'early-{arm}-s{seed}', state='done')
                for arm in ('a', 'b') for seed in (1, 2)]
        progress = self.dashboard.job(job, rows, 100)
        self.assertEqual(progress.total, 480)
        self.assertEqual(progress.done, 80)
        self.assertAlmostEqual(progress.fraction, 1 / 6)
        continuation = self.task(job, 'continuation-a-s1', stage='continuation',
                                 status='minute=50 end=120 speed=1x')
        progress = self.dashboard.task(job, continuation, 100)
        self.assertEqual(progress.done, 30)
        self.assertEqual(progress.total, 100)
        self.assertEqual(progress.eta, 70 * 60)
        combined = self.dashboard.job(job, rows + [continuation], 100)
        self.assertEqual(combined.done, 110)
        self.assertEqual(combined.eta, 370 * 60)

    def test_batch_saved_starts_and_pending_runs(self):
        job = self.job('batch', {'args': []})
        row = self.task(job, name='batch', status=None)
        root = Path(json.loads(row['spec'])['outputs']) / 'batch'
        root.mkdir(parents=True)
        (root / 'batch.json').write_text(json.dumps(dict(args={'hours': 2}, load_minutes={'1': 240},
            max_parallel=1, runs=[{'arm': a, 'seed': 1} for a in ('a', 'b')])))
        run = root / 'a/s1/run'
        run.mkdir(parents=True)
        (run / 'status').write_text('minute=300 end=360 speed=2x')
        progress = self.dashboard.task(job, row, 100)
        self.assertEqual((progress.done, progress.total), (60, 240))
        self.assertEqual(progress.eta, 5400)
        (run / 'DONE').touch()
        progress = self.dashboard.task(job, row, 160)
        self.assertEqual(progress.done, 120)
        self.assertIsNone(progress.eta)

    def test_unknown_and_marked_estimate(self):
        job = self.job('command', {'argv': ['true']})
        row = self.task(job, name='run')
        self.assertEqual(self.dashboard.task(job, row, 100).label(), '[?] ? ETA ?')
        estimated = self.task(job, name='estimated', est_h=1)
        result = self.dashboard.task(job, estimated, 1900)
        self.assertEqual(result.fraction, .5)
        self.assertEqual(result.eta, 1800)
        self.assertIn('~50%', result.label())
        self.assertIn('~30m', result.label())
        self.assertIsNone(self.dashboard.job(job, [row, estimated], 1900).fraction)

    def test_once_contains_hosts_jobs_and_done_reason_readonly(self):
        job = self.job()
        self.task(job, status='minute=10 end=20 speed=1x')
        finished = self.job('command', {'argv': ['true']}, state='done', name='finished')
        (Path(finished['out']) / 'DONE').write_text('ok: fixture finished\n')
        output = io.StringIO()
        with contextlib.redirect_stdout(output), patch('labkit.top.time.sleep'):
            rc = main(['--state', str(self.paths.state), '--hosts', str(self.paths.hosts), 'top', '--once'])
        self.assertEqual(rc, 0)
        for text in ('HOSTS', 'desktop', 'eidolon', 'JOBS', 'job', 'desktop:g1', 'ETA', 'ok: fixture finished'):
            self.assertIn(text, output.getvalue())
        self.assertNotIn('\x1b', output.getvalue())
        with self.assertRaises(sqlite3.OperationalError):
            self.dashboard.store.x("UPDATE jobs SET state='bad'")
        for width in (80, 100):
            self.assertTrue(all(len(line) <= width for line in self.dashboard.frame(width)))

    def test_job_eta_waits_for_slowest_task(self):
        job = self.job(spec=dict(hours=2, early_read=False, seed_list=[1], arms={'a': {}, 'b': {}}))
        fast = self.task(job, 'full-a-s1', stage='full', status='minute=100 end=120 speed=10x')
        slow = self.task(job, 'full-b-s1', stage='full', status='minute=60 end=120 speed=1x')
        self.assertEqual(self.dashboard.job(job, [fast, slow], 100).eta, 3600)
        (Path(json.loads(slow['spec'])['outputs']) / 'run/status').unlink()
        self.assertEqual(self.dashboard.task(job, slow, 100).label(), '[?] ? ETA ?')
        other = Dashboard(self.paths)
        try:
            self.assertIsNone(other.job(job, [fast, slow], 100).eta)
        finally:
            other.close()

    def test_interactive_q_closes_store(self):
        class Screen:
            def timeout(self, value):
                pass

            def getmaxyx(self):
                return 24, 80

            def erase(self):
                pass

            def addstr(self, y, x, line):
                assert len(line) < 80

            def refresh(self):
                pass

            def getch(self):
                return ord('q')

        from labkit.top import cmd_top
        from types import SimpleNamespace
        with patch('curses.wrapper', side_effect=lambda draw: draw(Screen())), \
                patch('labkit.top.Dashboard', return_value=self.dashboard):
            cmd_top(SimpleNamespace(once=False), self.paths)
        with self.assertRaises(sqlite3.ProgrammingError):
            self.dashboard.store.q('SELECT * FROM jobs')

    def test_cpu_uses_busy_deltas_excluding_guest(self):
        with patch('labkit.top.Path.read_text', side_effect=[
                'cpu 10 0 10 80 0 0 0 0 100 0\n', 'cpu 20 0 20 100 0 0 0 0 200 0\n']):
            self.assertIsNone(self.dashboard.cpu())
            self.assertEqual(self.dashboard.cpu(), 50)

    def test_missing_state_stays_missing(self):
        missing = Paths(self.root / 'absent', self.paths.cache, self.paths.runs, self.paths.hosts)
        view = Dashboard(missing)
        try:
            self.assertIn('HOSTS', '\n'.join(view.frame()))
        finally:
            view.close()
        self.assertFalse(missing.state.exists())


if __name__ == '__main__':
    unittest.main()
