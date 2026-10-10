"""Stop rules use authored census rows and isolated state, never the live labd."""
import contextlib
from concurrent.futures import Future
import io
import json
from pathlib import Path
import platform
import subprocess
import sys
from types import SimpleNamespace
from unittest.mock import Mock, patch

import test_lab
from labkit.cli import cmd_status, cmd_submit
from labkit.core import Kind, SpecError, Task, register
from labkit.kinds.ab import AB
from labkit.kinds.batch import Batch
from labkit.stops import CensusRun, Expression, evaluate, run_hook, validate_rules
from labkit.top import Dashboard


@register
class StopKind(Kind):
    name = 'test-stop'

    def finish(self, job, ctx):
        job.state['finish_called'] = True
        return 'partial outputs retained'


class StopTests(test_lab.LabCase):
    def census(self, path, rows='0,1\n30,400\n', header='sim_min,count'):
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(header + '\n' + rows)
        return path

    def expression_job(self, **rule):
        spec = dict(kind='ab', arms={'base': {}, 'branch': {}},
                    stop=[dict(when='count > 300', scope='any', action='abort', after_minute=30, **rule)])
        return SimpleNamespace(kind='ab', spec=spec)

    def test_any_all_after_minute_and_missing_runs(self):
        a = self.census(self.root / 'a.csv', '0,1\n29,400\n')
        b = self.root / 'b.csv'
        runs = [CensusRun('branch', '1', [a]), CensusRun('branch', '2', [b])]
        job = self.expression_job()
        self.assertIsNone(evaluate(job, runs))
        self.census(a)
        self.assertIsNotNone(evaluate(job, runs))
        job.spec['stop'][0]['scope'] = 'all'
        self.assertIsNone(evaluate(job, runs))
        self.census(b, '30,400\n')
        self.assertIsNotNone(evaluate(job, runs))
        self.census(b, '30,100\n')
        self.assertIsNone(evaluate(job, runs))

    def test_delta_across_stages_and_arm_same_seed(self):
        early = self.census(self.root / 'early.csv', '0,2\n20,4\n')
        continuation = self.census(self.root / 'rest.csv', '20,4\n30,10\n')
        base = self.census(self.root / 'base.csv', '30,3\n')
        other_seed = self.census(self.root / 'other.csv', '30,100\n')
        runs = [CensusRun('branch', '1', [early, continuation]), CensusRun('base', '1', [base]),
                CensusRun('base', '2', [other_seed])]
        job = self.expression_job()
        job.spec['stop'][0].update(arms=['branch'], when='delta(count, 10) == 6 and count - arm(base, count) == 7')
        self.assertIsNotNone(evaluate(job, runs))
        job.spec['stop'][0]['when'] = 'delta(count, 40) > 0'
        self.assertIsNone(evaluate(job, runs))

    def test_safe_arithmetic_boolean_and_comparisons(self):
        expr = Expression('not (count < 2 or count > 8) and -count + 4 * (count / 2) == +count')
        self.assertTrue(expr.value([dict(sim_min='30', count='4')], {}))
        for text in ('__import__("os")', 'count.real', 'count[0]', 'count ** 2', '[count]',
                     'delta(count, -1)', 'arm(base, count)'):
            with self.subTest(text=text), self.assertRaises(SpecError):
                Expression(text)

    def test_unknown_column_at_first_evaluation_even_short_circuit(self):
        path = self.census(self.root / 'census.csv')
        job = self.expression_job()
        job.spec['stop'][0]['when'] = 'count > 0 or missing > 0'
        with self.assertRaisesRegex(SpecError, 'unknown census column.*missing'):
            evaluate(job, [CensusRun('branch', '1', [path])])

    def test_unknown_column_rejected_at_submit_with_known_header(self):
        daemon = self.daemon()
        out = self.root / 'existing'
        self.census(out / 'run/census.csv')
        spec = dict(kind='test-stop', out=str(out), stop=[dict(when='missing > 0')])
        with self.assertRaisesRegex(SpecError, 'unknown census column'):
            daemon.insert_job('bad-header', spec)
        with self.assertRaisesRegex(SpecError, 'unknown census column'):
            daemon.insert_job('bad-outputs-header', dict(kind='command', argv=['true'], outputs=str(out / 'run'),
                                                        stop=[dict(when='missing > 0')]))
        file = self.root / 'bad.json'
        file.write_text(json.dumps(spec))
        args = SimpleNamespace(spec=str(file), name=None)
        with self.assertRaisesRegex(SpecError, 'unknown census column'):
            cmd_submit(args, self.paths)
        self.assertFalse(list((self.paths.state / 'incoming').iterdir()))

    def test_incomplete_csv_tail_is_ignored(self):
        path = self.census(self.root / 'census.csv', '30,1\n31,400')
        job = self.expression_job()
        self.assertIsNone(evaluate(job, [CensusRun('branch', '1', [path])]))
        with path.open('a') as stream:
            stream.write('\n')
        self.assertIsNotNone(evaluate(job, [CensusRun('branch', '1', [path])]))

    def job(self, rule, kind='test-stop'):
        daemon = self.daemon()
        daemon.insert_job('stop-job', dict(kind=kind, stop=[rule], out=str(self.root / f'output{len(self.daemons)}')))
        job = daemon.job('stop-job')
        job.out.mkdir(parents=True)
        daemon.add_tasks(job, [Task(name='run', argv=['true']), Task(name='later', argv=['true'])])
        daemon.store.update('jobs', dict(id=job.id), state='running')
        daemon.store.update('tasks', dict(job=job.id, name='run'), state='running', host='fake', unit='fake-task')
        daemon.launchers['fake'].stop = Mock()
        return daemon, daemon.job(job.id)

    def test_finish_keeps_outputs_calls_finish_and_skips_pending(self):
        daemon, job = self.job(dict(when='count > 300', action='finish', reason='answered'))
        path = self.census(job.tasks['run'].out / 'census.csv')
        daemon.after_task(job.id, 'run')
        daemon.finish_jobs()
        final = daemon.job(job.id)
        self.assertEqual(final.status, 'done')
        self.assertEqual(final.tasks['later'].state, 'skipped')
        self.assertTrue(final.state['finish_called'])
        self.assertTrue(path.exists())
        daemon.launchers['fake'].stop.assert_called_once()
        for name in ('DONE', 'summary.txt'):
            self.assertIn('answered', (job.out / name).read_text())
        self.assertIn('answered', daemon.store.one('SELECT summary FROM jobs WHERE id=?', job.id)['summary'])
        self.assertEqual(self.events('stopped', job.id)[0]['action'], 'finish')

    def test_abort_and_expression_error_are_visible(self):
        for expression, expected in (('count > 300', 'boom'), ('missing > 0', 'stop rule error')):
            with self.subTest(expression=expression):
                # Give each fake daemon/job its own state directory.
                if self.daemons:
                    self.paths = test_lab.Paths(self.root / 'state2', self.root / 'cache2', self.root / 'runs2',
                                                self.root / 'hosts2.toml').ensure()
                daemon, job = self.job(dict(when=expression, reason='boom'))
                self.census(job.tasks['run'].out / 'census.csv')
                self.assertTrue(daemon.check_stops(job))
                daemon.finish_jobs()
                self.assertEqual(daemon.job(job.id).status, 'aborted')
                self.assertIn(expected, (job.out / 'DONE').read_text())

    def hook(self, text):
        path = self.root / 'hook.py'
        path.write_text('#!' + sys.executable + '\nimport sys\n' + text + '\n')
        path.chmod(0o755)
        return str(path)

    def test_hook_json_stops_job_with_output_dir_argument(self):
        hook = self.hook('import json\nprint(json.dumps({"stop": True, "action": "finish", "reason": sys.argv[1]}))')
        daemon, job = self.job(dict(hook=hook, every_min=10))
        daemon.kinds[job.kind].census_runs = Mock(side_effect=AssertionError('hook needs no census'))
        daemon.check_stops(job)
        daemon.kinds[job.kind].census_runs.assert_not_called()
        test_lab.wait_for(lambda: all(f.done() for f in daemon.stop_hooks.values()))
        daemon.collect_stop_hooks()
        daemon.finish_jobs()
        self.assertEqual(daemon.job(job.id).status, 'done')
        self.assertIn(str(job.out), (job.out / 'DONE').read_text())

    def test_bad_hook_logged_ignored_and_interval_persisted(self):
        daemon, job = self.job(dict(hook=self.hook('print("bad JSON")'), every_min=10))
        daemon.check_stops(job)
        test_lab.wait_for(lambda: all(f.done() for f in daemon.stop_hooks.values()))
        daemon.log = Mock()
        daemon.collect_stop_hooks()
        self.assertEqual(daemon.job(job.id).status, 'running')
        daemon.log.assert_called_once()
        self.assertTrue(self.events('stop-hook-error', job.id))
        daemon.check_stops(daemon.job(job.id))
        self.assertFalse(daemon.stop_hooks)
        daemon.clock = lambda: job.state['_lab_stop']['hooks']['0'] + 601
        daemon.check_stops(daemon.job(job.id))
        self.assertTrue(daemon.stop_hooks)

    def test_hook_timeout_non_json_and_invalid_schema(self):
        rule = dict(hook='dummy')
        with patch('labkit.stops.subprocess.run', side_effect=subprocess.TimeoutExpired('dummy', 60)) as run:
            with self.assertRaises(subprocess.TimeoutExpired):
                run_hook(rule, self.root)
            self.assertEqual(run.call_args.kwargs['timeout'], 60)
        for reply in ('oops', '{"stop": "yes", "action": "finish", "reason": "x"}',
                      '{"stop": true, "action": "delete", "reason": "x"}', '{}\n{}'):
            with self.subTest(reply=reply), patch('labkit.stops.subprocess.run', return_value=SimpleNamespace(stdout=reply)):
                with self.assertRaises(ValueError):
                    run_hook(rule, self.root)
        daemon, job = self.job(dict(hook=self.hook('print("bad")')))
        future = Future()
        future.set_exception(subprocess.TimeoutExpired('dummy', 60))
        daemon.stop_hooks[job.id, 0] = future
        daemon.collect_stop_hooks()
        self.assertTrue(self.events('stop-hook-error', job.id))
        self.assertEqual(daemon.job(job.id).status, 'running')

    def test_invalid_rules_rejected(self):
        daemon = self.daemon()
        for rule in (dict(when='count > 0', scope='some'), dict(when='count > 0', action='delete'),
                     dict(when='count > 0', after_minute=-1), dict(when='count > 0', arms=['branch']),
                     dict(hook=self.hook('print("bad")'), every_min=0), dict(when='1', hook='x')):
            with self.subTest(rule=rule), self.assertRaises(SpecError):
                validate_rules(dict(kind='test-stop', stop=[rule]), daemon.ctx, StopKind())

    def test_ab_and_batch_declare_unwritten_runs_and_partial_finish(self):
        daemon = self.daemon()
        job = SimpleNamespace(out=self.root, spec=dict(seed_list=[1, 2], arms={'base': {}, 'branch': {}},
                                                      preflight_only=False), tasks={}, state={})
        runs = AB().census_runs(job, daemon.ctx)
        self.assertEqual(len(runs), 4)
        job.tasks['partial'] = SimpleNamespace(task=Task(name='partial', argv=['true'], meta=dict(stage='continuation')),
                                              state='cancelled')
        self.assertIn('continuation 1 cancelled', AB().finish(job, daemon.ctx))
        root = self.root / 'batch-task/batch'
        root.mkdir(parents=True)
        (root / 'batch.json').write_text(json.dumps(dict(runs=[dict(arm='a', seed=1), dict(arm='a', seed=2)])))
        runs = Batch().census_runs(job, daemon.ctx)
        self.assertEqual(len(runs), 2)
        self.assertFalse(any(Path(r.paths[0]).exists() for r in runs))
        job.kind = 'batch'
        job.spec['stop'] = [dict(when='count > 300', scope='all', action='finish', after_minute=0)]
        self.census(runs[0].paths[0])
        self.assertIsNone(evaluate(job, runs))
        self.census(runs[1].paths[0])
        self.assertEqual(evaluate(job, runs)['action'], 'finish')

    def test_status_and_top_show_rules_and_reason(self):
        daemon, job = self.job(dict(when='count > 300', action='finish', reason='question answered'))
        self.census(job.tasks['run'].out / 'census.csv')
        self.paths.hosts.write_text('[hosts.fake]\nkind="local"\ncpus=4\nmax_mem_gb=8\n')
        dashboard = Dashboard(self.paths)
        self.addCleanup(dashboard.close)
        self.assertIn('count > 300', '\n'.join(dashboard.frame(width=180)))
        daemon.check_stops(job)
        daemon.finish_jobs()
        args = SimpleNamespace(limit=10, all=True, recent=3, json=False)
        with contextlib.redirect_stdout(io.StringIO()) as output:
            cmd_status(args, self.paths)
        self.assertIn('count > 300', output.getvalue())
        self.assertIn('question answered', output.getvalue())
        self.assertIn('question answered', '\n'.join(dashboard.frame(width=180)))

    def test_remote_mirrored_outputs_evaluated_before_progress(self):
        data = self.inventory()
        data['hosts']['far'] = dict(kind='ssh', address='local', arch=platform.machine(), cpus=2,
                                   groups='auto', max_cpu_fraction=1, reserve_cpus=0, max_mem_gb=8,
                                   launcher='setsid', root=str(self.root / 'far'), python=sys.executable,
                                   idle=True, external_load=False)
        daemon = self.daemon(data)
        job_id = self.submit(dict(kind='command', hosts=['far'],
                                  argv=['sh', '-c', "printf 'sim_min,count\\n30,400\\n' > {out}/census.csv; sleep 20"],
                                  stop=[dict(when='count > 300', action='finish', reason='remote boom')]))
        state, reason = self.finished(daemon, job_id)
        self.assertEqual(state, 'done', reason)
        job = daemon.job(job_id)
        self.assertEqual(job.tasks['run'].host, 'far')
        self.assertIn('30,400', (job.tasks['run'].out / 'census.csv').read_text())
        self.assertEqual(self.events('stopped', job_id)[0]['reason'], 'remote boom')
