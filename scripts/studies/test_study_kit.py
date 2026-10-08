"""Tiny authored telemetry; no simulations or assertions on tuned defaults."""

import copy
import csv
import io
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import tomllib
import unittest
from contextlib import redirect_stdout
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parent))
import batch
import score
from study_contract import founding_required, read_required, sha256


REQUIRED = dict(animals=['frondgrazer', 'lanternjaw'], plants=['turf', 'flower'],
                N_ref=dict(frondgrazer=2, lanternjaw=2))


def write_csv(path, rows, fields=None):
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open('w', newline='') as stream:
        writer = csv.DictWriter(stream, fields or list(rows[0]))
        writer.writeheader()
        writer.writerows(rows)


def fixture(root, seed=1, load=0, end=6, surviving=True):
    (root / 'run').mkdir(parents=True, exist_ok=True)
    (root / 'run/DONE').write_text(str(load + end))
    census, vitals, plants = [], [], []
    for offset in range(end + 1):
        minute = load + offset
        count = 8 if offset == 2 else 1 if offset >= 5 else 2
        census.append(dict(sim_min=minute, seed=seed, mode='Dry', founders_frondgrazer=count,
            eggs_frondgrazer=0, founders_lanternjaw=0, eggs_lanternjaw=2,
            stands_turf=2, stands_flower=1, meadow_patch_max=offset + 1,
            grove_patch_max=2, recruits_vaulttree=offset,
            kills_chorister_frondgrazer=offset, kills_lanternjaw_frondgrazer=0,
            births_frondgrazer=int(offset >= 4), eggs_laid_frondgrazer=0,
            eggs_hatched_frondgrazer=0, riders_now=0))
        if offset == 0:
            continue
        for name in ('frondgrazer', 'lanternjaw'):
            browser = name == 'frondgrazer'
            vitals.append(dict(minute=minute, lineage=name, n=count if browser else 0,
                breeding_eligible_n=int(offset == 2 and browser),
                fullness_p10=0.1 if offset == 4 and browser else 0.8 if browser else '',
                fullness_p50=0.8 if browser else '',
                awake_body_s=offset if browser else 0, energy_body_hours=1 if browser else 0,
                net_energy_per_body_hour=offset if browser else '',
                moving_share=int(offset != 4) if browser else '',
                stuck_n=1 if browser else '', stuck_observed_n=2 if browser else '',
                n_adult=count if browser else 0, n_juvenile=0, n_pup=0, n_rider=0))
        for name in ('turf', 'flower'):
            plants.append(dict(minute=minute, species=name, stands=2 if name == 'turf' else 1,
                               vigour_mean=1 - offset / (end + 10)))
    write_csv(root / 'census.csv', census)
    write_csv(root / 'vitals/vitals.csv', vitals)
    write_csv(root / 'vitals/plants_vitals.csv', plants)
    lives = [dict(id=1, lineage='frondgrazer', native='true', origin='observed',
                  born_minute=load - 1, died_minute='', matured_minute='', cause='', last_seen_minute=load + end),
             dict(id=2, lineage='frondgrazer', native='true', origin='born', born_minute=load + 4,
                  died_minute='' if surviving else load + 5, matured_minute=load + 5,
                  cause='' if surviving else 'killed_by_lanternjaw', last_seen_minute=load + end)]
    write_csv(root / 'vitals/life.csv', lives)


class StudyKitTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        document = tomllib.loads(score.DEFAULT_CONFIG.read_text())
        spec = document['profiles']['screen-2h']
        spec.update(to_minute=6, slow=['lanternjaw'])
        document['population'].update(k_hi=2.0, k_peak=2.0)
        document['targets'].update(fullness_p10=0.5, moving_share=0.5, voxel_m=0.5,
            meadow_m2=4.0, grove_m2=2.0, vaulttree_recruits=6.0,
            hunts_per_hour_lo=10.0, hunts_per_hour_hi=40.0)
        document['reproduction'].update(eligible_deficit=0.2, ineligible_deficit=0.9)
        self.document = document
        self.cfg = score.profile_config(document)

    def tearDown(self):
        self.temporary.cleanup()

    def world(self, **kwargs):
        fixture(self.root, **kwargs)
        result = score.score_world(self.root, 'base', 1, self.cfg, required=REQUIRED,
                                   load_minute=kwargs.get('load', 0))
        return result

    def rescore(self):
        return score.score_world(self.root, 'base', 1, self.cfg, required=REQUIRED)

    def mutate(self, file, function):
        path = self.root / file
        rows = score.read_csv(path, [])
        rows = function(rows) or rows
        write_csv(path, rows)

    def test_clipping_and_bands_follow_passed_config(self):
        self.assertEqual([score.h(x) for x in (-2, 0, 0.25, 2)], [1, 1, 0.75, 0])
        self.assertEqual(score.band_penalty([2, 1], 2, self.cfg['population']['k_hi']), 0.25)
        self.assertEqual(score.peak_penalty([2, 8, 1], 2, self.cfg['population']['k_peak']), 1)
        self.assertEqual(score.d(1, 0, 0), 1)

    def test_reproduction_all_branches_follow_config(self):
        cfg = self.cfg['reproduction']
        self.assertEqual(score.reproductive_penalty(1, False, cfg), 0)
        self.assertEqual(score.reproductive_penalty(0, True, cfg), cfg['eligible_deficit'])
        self.assertEqual(score.reproductive_penalty(0, False, cfg), cfg['ineligible_deficit'])
        self.assertIsNone(score.reproductive_penalty(0, None, cfg))

    def test_complete_fixture_profile_and_half_open_means(self):
        world = self.world()
        self.assertTrue(world['rankable'], world['warnings'])
        self.assertEqual(world['components']['H']['raw'], 6 / (6 / 60))
        # Late snapshots are minutes 3,4,5; endpoint 6 is boundary stock only.
        self.assertEqual(world['components']['M']['raw'], 5 * self.cfg['targets']['voxel_m']**2)
        self.assertAlmostEqual(world['components']['F']['per_lineage_penalty']['frondgrazer'],
            sum(score.h(x / self.cfg['targets']['fullness_p10']) for x in (0.8, 0.1, 0.8)) / 3)
        self.assertEqual(world['components']['Q']['weight'], 0)
        self.assertEqual(world['profile_hash'], self.cfg['profile_hash'])
        self.assertEqual(world['components']['A']['raw']['lanternjaw']['N_ref'], 2)
        self.assertEqual(world['soft_P'], sum(v['contribution'] for k, v in world['components'].items() if k != 'L'))
        json.dumps(world, allow_nan=False)

    def test_missing_vitals_unrankable_and_cannot_win(self):
        baseline = self.world()
        (self.root / 'vitals/vitals.csv').unlink()
        trial = self.rescore()
        trial['arm'] = 'trial'
        self.assertFalse(trial['rankable'])
        self.assertIsNone(trial['P'])
        paired = score.pair_worlds([baseline, trial], 'base', self.cfg)['trial']
        self.assertEqual((paired['wins'], paired['invalid']), (0, 1))

    def test_missing_required_lineage_or_minute_unrankable(self):
        for filter_rows in (lambda rows: [r for r in rows if r['lineage'] != 'lanternjaw'],
                            lambda rows: [r for r in rows if r['minute'] != '3']):
            fixture(self.root)
            self.mutate('vitals/vitals.csv', filter_rows)
            self.assertFalse(self.rescore()['rankable'])

    def test_required_founding_plant_absent_at_start_is_lost(self):
        fixture(self.root)
        self.mutate('census.csv', lambda rows: [dict(r, stands_flower=0) for r in rows])
        world = self.rescore()
        self.assertTrue(world['rankable'], world['warnings'])
        self.assertEqual(world['lost'], ['plant:flower'])
        self.assertFalse(score.arm_summary([world], self.cfg)['feasible'])

    def test_unequal_observed_horizons_unrankable_and_refused_pair(self):
        baseline = self.world()
        trial = self.world(end=5)
        trial['arm'] = 'trial'
        self.assertFalse(trial['rankable'])
        # A scorer configured to another legitimate profile cannot be paired.
        trial = copy.deepcopy(baseline)
        trial.update(arm='trial', horizon=dict(start=0, end=5))
        with self.assertRaisesRegex(ValueError, 'different profile or horizon'):
            score.pair_worlds([baseline, trial], 'base', self.cfg)
        self.assertFalse(score.score_world(self.root, 'base', 1, self.cfg, 0, 5, required=REQUIRED)['rankable'])

    def test_extinction_cannot_hide_in_mean_sd(self):
        template = self.world()
        base, trial = [], []
        for seed in range(1, 9):
            a, b = copy.deepcopy(template), copy.deepcopy(template)
            a.update(arm='base', seed=seed, soft_P=35)
            b.update(arm='trial', seed=seed, soft_P=0, lost=['plant:flower'] if seed == 8 else [])
            base.append(a)
            trial.append(b)
        summary = score.arm_summary(trial, self.cfg, {w['seed']: w for w in base})
        self.assertFalse(summary['feasible'])
        self.assertLess(summary['rank'], score.arm_summary(base, self.cfg)['rank'])
        self.assertGreater(summary['rank_key'], score.arm_summary(base, self.cfg)['rank_key'])
        paired = score.pair_worlds(base + trial, 'base', self.cfg)['trial']
        self.assertEqual(paired['wins'], 8)
        self.assertEqual(paired['worst_seed'], 8)
        self.assertIn('not a significance test', paired['note'])

    def test_baseline_loss_allows_same_loss_but_not_another_species(self):
        baseline = self.world()
        baseline['lost'] = ['plant:flower']
        trial = copy.deepcopy(baseline)
        self.assertTrue(score.arm_summary([trial], self.cfg, {1: baseline})['feasible'])
        trial['lost'] = ['plant:turf']
        self.assertFalse(score.arm_summary([trial], self.cfg, {1: baseline})['feasible'])

    def test_doomed_recruit_does_not_clear_J(self):
        world = self.world(surviving=False)
        life = world['components']['J']['raw']['frondgrazer']
        self.assertEqual((life['late_births'], life['surviving_recruits']), (1, 0))
        self.assertEqual(life['penalty'], self.cfg['reproduction']['eligible_deficit'])
        self.assertEqual(world['health']['frondgrazer']['events']['live_births'], 1)
        self.assertEqual(world['health']['frondgrazer']['deaths']['killed_by_lanternjaw'], 1)
        self.assertIn('this profile', world['health']['lanternjaw']['flag'])

    def test_observed_body_does_not_clear_J(self):
        fixture(self.root)
        self.mutate('vitals/life.csv', lambda rows: [dict(r, origin='observed') for r in rows])
        world = self.rescore()
        self.assertEqual(world['components']['J']['raw']['frondgrazer']['surviving_recruits'], 0)
        self.assertGreater(world['components']['J']['penalty'], 0)

    def test_new_energy_names_geometry_and_plant_trends(self):
        fixture(self.root)
        self.mutate('vitals/vitals.csv', lambda rows: [dict(r, survivor_stock_change_per_body_hour=r.pop('net_energy_per_body_hour'), cohort_energy_change=-2 if r['lineage'] == 'frondgrazer' else '') for r in rows])
        (self.root / 'vitals/meta.json').write_text(json.dumps(dict(voxel_m=1, width=3, depth=3, seed=1)))
        world = self.rescore()
        self.assertTrue(world['rankable'], world['warnings'])
        self.assertEqual(world['components']['M']['raw'], 5)
        self.assertEqual(world['health']['frondgrazer']['cohort_energy_change'], -2)
        self.assertIsNotNone(world['health']['frondgrazer']['survivor_stock_change_per_body_hour'])
        self.assertLess(world['plants']['turf']['vigour_trend_per_minute'], 0)
        self.assertLess(world['trends']['frondgrazer']['count_slope_per_minute_per_N_ref'], 0)
        self.assertEqual(world['trends']['frondgrazer']['drawdown_per_N_ref'], 3.5)

    def test_no_stuck_exposure_is_NA_per_lineage_and_not_stuck(self):
        # No eligible exposure (asleep, fed or resting all window) is measured data, not
        # missing telemetry: the lineage's share stays NA, it counts as not stuck in S, and
        # it is listed as unexposed.
        fixture(self.root)
        self.mutate('vitals/vitals.csv', lambda rows: [dict(r, stuck_n='', stuck_observed_n=0) for r in rows])
        world = self.rescore()
        self.assertTrue(world['rankable'])
        self.assertIsNone(world['health']['frondgrazer']['stuck_share'])
        self.assertEqual(world['components']['S']['penalty'], 0.0)
        self.assertIn('frondgrazer', world['components']['S']['unexposed_lineages'])

    def test_nonfinite_negative_duplicate_and_gap_telemetry(self):
        for file, mutate in (
            ('census.csv', lambda rows: [dict(r, founders_frondgrazer=-1) for r in rows]),
            ('census.csv', lambda rows: rows + [rows[-1]]),
            ('census.csv', lambda rows: [r for r in rows if r['sim_min'] != '3']),
            ('vitals/vitals.csv', lambda rows: [dict(r, fullness_p10='nan') for r in rows]),
            ('vitals/vitals.csv', lambda rows: rows + [rows[-1]]),
            ('vitals/plants_vitals.csv', lambda rows: [dict(r, stands=-1) for r in rows])):
            fixture(self.root)
            self.mutate(file, mutate)
            self.assertFalse(self.rescore()['rankable'], file)

    def test_resumed_segments_count_minutes_and_life_events_once(self):
        original = self.world()
        for file, time in (('census.csv', 'sim_min'), ('vitals/vitals.csv', 'minute'), ('vitals/plants_vitals.csv', 'minute')):
            rows = score.read_csv(self.root / file, [])
            first = [dict(r, segment=0) for r in rows if float(r[time]) <= 4]
            second = [dict(r, segment=1) for r in rows if float(r[time]) >= 4]
            write_csv(self.root / file, first + second)
        records = score.read_csv(self.root / 'vitals/life.csv', [])
        write_csv(self.root / 'vitals/life.csv', [dict(r, segment=0) for r in records] + [dict(r, segment=1, origin='observed') for r in records])
        resumed = self.rescore()
        self.assertTrue(resumed['rankable'], resumed['warnings'])
        self.assertEqual(resumed['soft_P'], original['soft_P'])
        self.assertEqual(resumed['health'], original['health'])
        self.assertEqual(resumed['plants'], original['plants'])

    def test_failed_paused_missing_runs_remain_unrankable(self):
        self.assertIsNone(self.rescore()['P'])
        fixture(self.root)
        for marker in ('FAILED', 'PAUSED'):
            (self.root / 'run' / marker).touch()
            self.assertIsNone(self.rescore()['P'])
            (self.root / 'run' / marker).unlink()
        self.assertIsNone(score.score_world(self.root, 'base', 2, self.cfg, required=REQUIRED)['P'])

    def test_founding_roster_not_loaded_survivors(self):
        source = self.root / 's7'
        fixture(source, seed=7)
        start = source / 'run/ckpt/m000060'
        start.mkdir(parents=True)
        roster = founding_required(start)
        self.assertIn('flower', roster['plants'])
        self.assertEqual(roster['N_ref']['lanternjaw'], 2)
        self.assertEqual(batch.start_seed(start), 7)
        self.assertEqual(roster['source_sha256'], sha256(source / 'census.csv'))
        self.mutate_file_minute(source / 'census.csv')
        with self.assertRaisesRegex(ValueError, 'minute-0'):
            founding_required(start)

    def mutate_file_minute(self, path):
        rows = score.read_csv(path, [])
        rows[0]['sim_min'] = '1'
        write_csv(path, rows)

    def test_profile_validates_masks_and_hashes_exact_file(self):
        raw = b'authored fixture profile text'
        cfg = score.profile_config(self.document, text=raw)
        self.assertEqual(cfg['profile_hash'], score.hashlib.sha256(raw).hexdigest())
        changed = copy.deepcopy(self.document)
        changed['profiles']['screen-2h']['disabled'] = {}
        with self.assertRaises(ValueError):
            score.profile_config(changed)
        changed = copy.deepcopy(self.document)
        changed['profiles']['screen-2h']['to_minute'] = 6.5
        with self.assertRaises(ValueError):
            score.profile_config(changed)

    def batch_fixture(self, out, binary_hash='binary', arms=('base', 'trial')):
        out.mkdir(parents=True, exist_ok=True)
        manifest = dict(arms={arm: [] for arm in arms}, seeds=[1], required=REQUIRED,
                        load_minutes={'1': 0}, binary_sha256=binary_hash)
        (out / 'batch.json').write_text(json.dumps(manifest))
        for arm in arms:
            fixture(out / arm / 's1')
        return manifest

    def test_report_owner_page_and_failed_seed(self):
        out = self.root / 'batch'
        self.batch_fixture(out)
        (out / 'trial/s1/run/FAILED').touch()
        report = score.build_report(out, self.cfg, 'base')
        self.assertEqual(report['ranking'][-1], 'trial')
        self.assertEqual(report['paired']['trial']['invalid'], 1)
        page = score.markdown(report)
        for text in ('Per-seed required losses', 'Worst seed', '**frondgrazer** lifecycle', 'Plant health', self.cfg['profile_hash']):
            self.assertIn(text, page)
        json.dumps(report, allow_nan=False)
        self.assertEqual(next(score.records(report))[0], 'profile')
        with redirect_stdout(io.StringIO()) as stream:
            score.plain(report)
        self.assertIn('wins 0/1', stream.getvalue())

    def test_cross_batch_binary_identity_guard(self):
        a, b = self.root / 'a', self.root / 'b'
        self.batch_fixture(a, 'one', ('base',))
        self.batch_fixture(b, 'two', ('trial',))
        with self.assertRaisesRegex(ValueError, 'binary hashes differ'):
            score.build_report([a, b], self.cfg, 'a:base')
        report = score.build_report([a, b], self.cfg, 'a:base', allow_mixed_binaries=True)
        self.assertTrue(report['mixed_binaries'])
        self.assertEqual(report['paired']['b:trial']['wins'], 0)

    def test_seed_parser_and_required_validation(self):
        self.assertEqual(batch.seed_list('2-4,7'), [2, 3, 4, 7])
        for value in ('3-1', '1,1', 'bad'):
            with self.assertRaises(ValueError):
                batch.seed_list(value)
        path = self.root / 'required.toml'
        path.write_text('[required]\nanimals=["browser"]\nplants=["turf"]\n[required.N_ref]\nbrowser=3\n')
        self.assertEqual(read_required(path)['N_ref']['browser'], 3)
        path.write_text('[required]\nanimals=["browser"]\nplants=[]\n')
        with self.assertRaises(ValueError):
            read_required(path)
    def test_loaded_start_manifest_hashes_and_memory_cap(self):
        source = self.root / 's7'
        fixture(source, seed=7)
        start = source / 'run/ckpt/m000006'
        start.mkdir(parents=True)
        (start / 'minute').write_text('6')
        for name in ('world.bin', 'flora.bin', 'fauna.bin'):
            (start / name).write_bytes(name.encode())
        binary, policy = self.root / 'census', self.root / 'policy.bin'
        binary.write_bytes(b'fake binary')
        policy.write_bytes(b'fake policy')
        out = self.root / 'batch'
        with patch.object(batch, 'execute') as execute, patch.object(batch, 'topology', return_value=[list(range(16)), list(range(16, 32))]), patch.object(batch.os, 'sched_getaffinity', return_value=set(range(32))), patch.object(batch, 'mem_available', return_value=10 * 1024**3), redirect_stdout(io.StringIO()) as stream:
            batch.main([str(out), '--bin', str(binary), '--starts', str(start), '--hours', '0.1',
                        '--arm', 'base=', '--arm', f'trial=policy=frondgrazer={policy}',
                        '--max-parallel', '12', '--mem-per-run-gb', '3'])
        manifest = json.loads((out / 'batch.json').read_text())
        self.assertEqual(manifest['max_parallel'], 2)  # floor(80% of authored 10 GiB / 3 GiB)
        self.assertEqual(execute.call_args.args[2], 2)
        self.assertIn('memory cap 2', stream.getvalue())
        self.assertEqual(manifest['binary_sha256'], sha256(binary))
        for name in ('world.bin', 'flora.bin', 'fauna.bin'):
            self.assertEqual(manifest['identity']['starts']['7'][name], sha256(start / name))
        self.assertEqual(manifest['identity']['policies']['trial']['frondgrazer']['sha256'], sha256(policy))
        self.assertEqual(manifest['load_minutes']['7'], 6)
        self.assertIn('flower', manifest['required']['by_seed']['7']['plants'])
        self.assertEqual(manifest['required']['by_seed']['7']['N_ref']['lanternjaw'], 2)

    def test_legacy_loaded_boundary_recovered_exactly(self):
        fixture(self.root, load=60)
        census = score.read_csv(self.root / 'census.csv', [])
        roster = dict(REQUIRED, load_census=census[0])
        write_csv(self.root / 'census.csv', census[1:])
        world = score.score_world(self.root, 'base', 1, self.cfg, required=roster, load_minute=60)
        self.assertTrue(world['rankable'], world['warnings'])
        self.assertEqual(world['components']['H']['raw'], 6 / (6 / 60))
        self.assertIn('recovered', ' '.join(world['warnings']))
        missing = score.score_world(self.root, 'base', 1, self.cfg, required=REQUIRED, load_minute=60)
        self.assertFalse(missing['rankable'])  # Never estimate a lost counter interval.

    def test_sparse_plant_vitals_invalid_not_a_trend_from_survivors(self):
        fixture(self.root)
        self.mutate('vitals/plants_vitals.csv', lambda rows: [r for r in rows if r['minute'] != '4'])
        self.assertFalse(self.rescore()['rankable'])

    def test_stage_condition_readouts_leave_F_pooled(self):
        fixture(self.root)
        pooled = self.rescore()['components']['F']
        def stages(rows):
            for r in rows:
                alive = int(r['n']) > 0
                for stage in ('adult', 'juvenile', 'pup', 'rider'):
                    present = alive and stage in ('adult', 'juvenile')
                    r[f'n_fullness_{stage}'] = 2 if present else ''
                    r[f'fullness_p10_{stage}'] = (1 if stage == 'adult' else 0) if present else ''
                    r[f'fullness_p50_{stage}'] = r[f'fullness_p10_{stage}']
                    r[f'starving_n_{stage}'] = (0 if stage == 'adult' else 2) if present else ''
                    r[f'n_{stage}'] = 2 if present else 0
            return rows
        self.mutate('vitals/vitals.csv', stages)
        world = self.rescore()
        self.assertTrue(world['rankable'], world['warnings'])
        for key in ('penalty', 'contribution', 'per_lineage_penalty'):
            self.assertEqual(world['components']['F'][key], pooled[key])
        health = world['health']['frondgrazer']
        self.assertEqual(health['stage_fullness']['adult']['fullness_p10'], 1)
        self.assertEqual(health['stage_fullness']['juvenile']['fullness_p50'], 0)
        self.assertIsNone(health['stage_fullness']['pup']['fullness_p10'])
        self.assertEqual(health['stage_fullness']['juvenile']['survivors_by_minute'][0]['starving_n'], 2)
        report = '\n'.join(score.health_tables([world]))
        self.assertIn('Starving n', report)
        self.assertIn('low-p10 driver (lowest late-stage p10): juvenile', report)
        self.assertIn('| frondgrazer | adult | 1 [1,1] | 1 [1,1] | 2–2 | 0–0 |', report)
        # Stage quantiles use their own survivor counts, not pooled population weights.
        self.mutate('vitals/vitals.csv', lambda rows: [dict(r, fullness_p10_adult=r['fullness_p10']) for r in rows])
        stage_mean = self.rescore()['health']['frondgrazer']['stage_fullness']['adult']['fullness_p10']
        self.assertAlmostEqual(stage_mean, (0.1 + 0.8 + 0.8) / 3)

    def test_profile_mask_controls_required_telemetry(self):
        document = copy.deepcopy(self.document)
        spec = document['profiles']['screen-2h']
        spec['active'] = ['L', 'A', 'K']
        spec['disabled'] = {term: 'fixture deliberately measures stocks only' for term in score.COMPONENTS if term not in spec['active']}
        cfg = score.profile_config(document)
        fixture(self.root)
        (self.root / 'vitals/vitals.csv').unlink()
        (self.root / 'vitals/life.csv').unlink()
        world = score.score_world(self.root, 'base', 1, cfg, required=REQUIRED)
        self.assertTrue(world['rankable'], world['warnings'])
        self.assertEqual(world['components']['F']['weight'], 0)

    def test_mismatched_seed_sets_cannot_drop_a_baseline_seed(self):
        world = self.world()
        second = copy.deepcopy(world)
        second['seed'] = 2
        summary = score.arm_summary([world], self.cfg, {1: world, 2: second})
        self.assertFalse(summary['rankable'])
        self.assertFalse(summary['feasible'])
        self.assertEqual(summary['invalid_seeds'], [2])

    def test_extinction_arm_is_ranked_after_all_feasible_arms(self):
        out = self.root / 'batch'
        self.batch_fixture(out)
        path = out / 'trial/s1/census.csv'
        rows = score.read_csv(path, [])
        write_csv(path, [dict(row, stands_flower=0) for row in rows])
        report = score.build_report(out, self.cfg, 'base')
        self.assertEqual(report['ranking'], ['base', 'trial'])
        self.assertTrue(report['arms']['base']['feasible'])
        self.assertFalse(report['arms']['trial']['feasible'])
        self.assertEqual(report['arms']['trial']['loss_vector'], {'1': ['plant:flower']})

    def test_absent_boundary_without_counter_components_can_score_old_data(self):
        document = copy.deepcopy(self.document)
        spec = document['profiles']['screen-2h']
        spec['active'] = ['L', 'A', 'K']
        spec['disabled'] = {term: 'fixture' for term in score.COMPONENTS if term not in spec['active']}
        cfg = score.profile_config(document)
        fixture(self.root, load=60)
        rows = score.read_csv(self.root / 'census.csv', [])
        write_csv(self.root / 'census.csv', rows[1:])
        world = score.score_world(self.root, 'base', 1, cfg, required=REQUIRED, load_minute=60)
        self.assertTrue(world['rankable'], world['warnings'])
        self.assertIn('legacy census', ' '.join(world['warnings']))

    def test_stock_deltas_name_the_answering_role(self):
        fixture(self.root)
        self.mutate('census.csv', lambda rows: [dict(r, litter=float(r['sim_min'])) for r in rows])
        world = self.rescore()
        self.assertEqual(world['stocks']['litter']['change'], 6)
        self.assertIn('shredders', world['stocks']['litter']['answering_role'])

    def test_cross_batch_start_content_difference_refuses_pair(self):
        a, b = self.root / 'a', self.root / 'b'
        self.batch_fixture(a, arms=('base',))
        manifest = self.batch_fixture(b, arms=('trial',))
        manifest['identity'] = dict(starts={'1': {'world.bin': 'changed'}})
        (b / 'batch.json').write_text(json.dumps(manifest))
        with self.assertRaisesRegex(ValueError, 'different start/config contents'):
            score.build_report([a, b], self.cfg, 'a:base')

    def test_standalone_batch_queues_and_retains_failed_and_paused_runs(self):
        if len(os.sched_getaffinity(0)) < 2:
            self.skipTest("batch requires one CPU reserved")
        scripts = self.root / "scripts"
        (scripts / "studies").mkdir(parents=True)
        shutil.copyfile(batch.__file__, scripts / "studies/batch.py")
        shutil.copyfile(Path(batch.__file__).with_name("study_contract.py"), scripts / "studies/study_contract.py")
        shutil.copyfile(Path(batch.__file__).resolve().parents[1] / "run-study.py", scripts / "run-study.py")
        fake = self.root / "census"
        fake.write_text("""#!/usr/bin/env python3
import json, os, pathlib, sys
run = pathlib.Path(next(a[4:] for a in sys.argv if a.startswith('run=')))
run.mkdir(parents=True)
minute = 3
state = 'FAILED' if 'fail' in sys.argv else 'PAUSED' if 'pause' in sys.argv else 'DONE'
(run / state).write_text(str(minute))
(run / 'status').write_text(f'minute={minute} state={state.lower()}')
print(json.dumps({'cpus': sorted(os.sched_getaffinity(0))}))
sys.exit(2 if state == 'FAILED' else 0)
""")
        fake.chmod(0o755)
        config = self.root / "input.toml"
        config.touch()
        required = self.root / "required.toml"
        required.write_text('[required]\nanimals = ["frondgrazer"]\nplants = ["turf"]\n[required.N_ref]\nfrondgrazer = 2\n')
        out = self.root / "out"
        completed = subprocess.run([sys.executable, str(scripts / "studies/batch.py"), str(out), "--bin", str(fake), "--arm", "base=", "--arm", "bad=fail", "--arm", "stopped=pause", "--config", str(config), "--required", str(required), "--seeds", "1-2", "--hours", "0.1", "--max-parallel", "1"], cwd=self.root, capture_output=True, text=True, timeout=10)
        self.assertEqual(completed.returncode, 0, completed.stderr)
        self.assertTrue((out / "DONE").exists())
        with (out / "STATE.tsv").open() as stream:
            states = list(csv.DictReader(stream, delimiter="\t"))
        self.assertEqual(len(states), 6)
        self.assertEqual([row["state"] for row in states], ["done", "failed", "paused"] * 2)
        manifest = json.loads((out / "batch.json").read_text())
        self.assertIsNone(manifest["commit"])
        for job in manifest["runs"]:
            observed = json.loads((out / job["arm"] / f's{job["seed"]}' / "census.csv").read_text())["cpus"]
            self.assertEqual(observed, manifest["l3_groups"][job["slot"] % len(manifest["l3_groups"])])
            self.assertEqual(job["command"][job["command"].index("chrt") + 1:job["command"].index("nice")], ["-i", "0"])

        self.assertEqual(manifest["identity"]["config"]["sha256"], batch.sha256(config))
        self.assertEqual(manifest["identity"]["required"]["sha256"], batch.sha256(required))
        self.assertEqual(manifest["required"]["by_seed"]["1"]["N_ref"]["frondgrazer"], 2)
        self.assertIn("parallel cap:", completed.stdout)


if __name__ == '__main__':
    unittest.main()
