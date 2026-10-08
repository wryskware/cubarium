#!/usr/bin/env python3
"""Reduce declared census profiles; infeasible or incomplete arms cannot win.

This is an external promotion gate, not the Rust search objective. Stock/event
windows are half-open [from,to); end-stamped interval rows (from,to] cover that
same exposure. Counts at to are used only for terminal health and counter deltas.
"""

import argparse
import csv
import copy
import hashlib
import json
import math
from pathlib import Path
import statistics
import sys
import tomllib

from vitals_summary import average, count_share, flag, read_csv, weighted
from study_contract import validate_required


COMPONENTS = ("L", "J", "Q", "A", "K", "V", "Z", "M", "G_b", "G_v", "H", "F", "S")
DEFAULT_CONFIG = Path(__file__).resolve().parents[2] / "config/training/fitness-v2.toml"


def clip(value):
    return min(1.0, max(0.0, value))


def h(value):
    return clip(1.0 - value)


def d(value, low, high):
    if low <= value <= high:
        return 0.0
    if value < low:
        return clip((low - value) / abs(low)) if low else 1.0
    return clip((value - high) / abs(high)) if high else 1.0


def band_penalty(counts, reference, k_hi):
    return average([d(n, reference, k_hi * reference) for n in counts])


def peak_penalty(counts, reference, k_peak):
    if not counts:
        return None
    ceiling = k_peak * reference
    return clip((max(counts) - ceiling) / ceiling) if ceiling else float(max(counts) > 0)


def reproductive_penalty(late_births, eligible, cfg):
    if late_births:
        return 0.0
    if eligible is None:
        return None
    return cfg["eligible_deficit"] if eligible else cfg["ineligible_deficit"]


def stats(values):
    values = [v for v in values if v is not None]
    if not values:
        return dict(median=None, min=None, max=None, n=0)
    return dict(median=statistics.median(values), min=min(values), max=max(values), n=len(values))


def component(raw=None, penalty=None, weight=0.0, **details):
    return dict(raw=raw, penalty=penalty, weight=weight,
                contribution=weight * penalty if penalty is not None else 0.0, **details)


def validate_config(cfg):
    for term in COMPONENTS:
        if cfg["weights"][term] < 0 or not math.isfinite(cfg["weights"][term]):
            raise ValueError(f"invalid weight: {term}")
    targets = cfg["targets"]
    for key in ("moving_share", "voxel_m", "meadow_m2", "grove_m2", "vaulttree_recruits", "fullness_p10"):
        if not math.isfinite(targets[key]) or targets[key] <= 0:
            raise ValueError(f"target {key} must be positive and finite")
    for lo, hi in (("predator_share_lo", "predator_share_hi"), ("hunts_per_hour_lo", "hunts_per_hour_hi")):
        if not 0 <= targets[lo] <= targets[hi] or not math.isfinite(targets[hi]):
            raise ValueError(f"invalid band: {lo}, {hi}")
    pop = cfg["population"]
    if not 1 <= pop["k_hi"] <= pop["k_peak"] or not math.isfinite(pop["k_peak"]):
        raise ValueError("require 1 <= k_hi <= k_peak, finite")
    if any(v < 0 or not math.isfinite(v) for v in pop.get("references", {}).values()):
        raise ValueError("population references must be finite and nonnegative")
    if any(not 0 <= v <= 1 for v in cfg["reproduction"].values()):
        raise ValueError("reproductive deficits must be in [0,1]")
    summary = cfg["summary"]
    if (not all(math.isfinite(value) for value in summary.values()) or not 0 < summary["consistent_fraction"] <= 1
            or summary["consistent_min_seeds"] < 1 or summary["sd_weight"] < 0):
        raise ValueError("invalid summary settings")


LABELS = {'H': 'kill rate (kills per world-hour)',
          'G_v': 'vaulttree recruitment (not established sites)',
          'F': 'mean lineage h(fullness_p10 / target)',
          'S': 'stuck share with observed exposure'}
TEXT_COLUMNS = {'lineage', 'species', 'origin', 'cause', 'killer_lineage', 'native', 'alive', 'adult', 'mode'}


def number(row, key, default=None):
    value = row.get(key)
    if value is None or value == '':
        return default
    value = float(value)
    if not math.isfinite(value):
        raise ValueError(f'non-finite {key}')
    return value


def profile_config(document, profile='screen-2h', text=None):
    cfg = copy.deepcopy(document)
    if document.get('version') != 2 or profile not in document.get('profiles', {}):
        raise ValueError(f'unknown v2 objective profile: {profile}')
    cfg['profile'] = copy.deepcopy(document['profiles'][profile])
    cfg['profile_id'] = profile
    # Hash the exact supplied profile file, including all shared targets/weights.
    payload = text if text is not None else json.dumps(document, sort_keys=True).encode()
    cfg['profile_hash'] = hashlib.sha256(payload).hexdigest()
    spec = cfg['profile']
    if (not isinstance(spec['from_minute'], int) or not isinstance(spec['to_minute'], int)
            or not 0 <= spec['from_minute'] < spec['to_minute']):
        raise ValueError('profile horizon must be whole minutes, 0 <= from < to')
    if set(spec['active']) - set(COMPONENTS) or 'L' not in spec['active']:
        raise ValueError('profile active mask must name known components and L')
    if set(COMPONENTS) != set(spec['active']) | set(spec['disabled']):
        raise ValueError('every inactive component needs a named reason')
    if set(spec['active']) & set(spec['disabled']):
        raise ValueError('a component cannot be active and disabled')
    if any(not spec['required_data'].get(term) for term in spec['active']):
        raise ValueError('every active component needs required_data')
    if spec['aggregation'] != 'mean + sd_weight * sample_sd of soft P (L excluded); feasibility first':
        raise ValueError('unsupported aggregation')
    if spec['start_kind'] not in ('mature saved start', 'fresh founding'):
        raise ValueError('unsupported start kind')
    if len(set(spec['active'])) != len(spec['active']):
        raise ValueError('duplicate active component')
    if any(value != 0 for value in spec['trend_weights'].values()):
        raise ValueError('trend weights are diagnostic only until a trend penalty is specified')
    validate_config(cfg)
    return cfg


def read_optional(path, warnings):
    if not path.exists():
        warnings.append(f'missing {path.name}')
        return []
    return read_csv(path, [])


def validate_numbers(rows, source):
    for row in rows:
        for key, value in row.items():
            if key is None:
                raise ValueError(f'{source}: malformed CSV row')
            if key in TEXT_COLUMNS or value in (None, ''):
                continue
            value = number(row, key)
            is_count = (key == 'n' or key == 'stands' or key.endswith('_n') or key.startswith(
                ('founders_', 'eggs_', 'stands_', 'kills_', 'births_', 'recruits_', 'n_')))
            if is_count and (value < 0 or not value.is_integer()):
                raise ValueError(f'{source}: negative or fractional count {key}')
            if key in ('awake_body_s', 'energy_body_hours', 'stuck_observed_n') and value < 0:
                raise ValueError(f'{source}: negative exposure {key}')
            if key.endswith('_share') and not 0 <= value <= 1:
                raise ValueError(f'{source}: invalid share {key}')


def minute_rows(rows, time_key, name_key=None):
    """Boundary repeats across segments use the original interval once.

    Repeated keys inside a segment are invalid. Earlier segments own a shared
    end minute: resumed boundary rows contain no preceding interval exposure.
    """
    chosen, seen = {}, set()
    for row in rows:
        minute = number(row, time_key)
        segment = number(row, 'segment', 0)
        if minute is None or minute < 0 or not minute.is_integer() or segment < 0 or not segment.is_integer():
            raise ValueError('minutes and segments must be nonnegative whole numbers')
        name = row.get(name_key) if name_key else None
        if name_key and not name:
            raise ValueError(f'missing {name_key}')
        key = (minute, name)
        if (key, segment) in seen:
            raise ValueError(f'duplicate minute {minute} for {name} in segment {segment}')
        if name_key == 'lineage' and number(row, 'stuck_n', 0) > number(row, 'stuck_observed_n', 0):
            raise ValueError('stuck count exceeds observed exposure')
        seen.add((key, segment))
        if key not in chosen or segment < number(chosen[key], 'segment', 0):
            chosen[key] = row
    return sorted(chosen.values(), key=lambda row: (number(row, time_key), row.get(name_key, '') if name_key else ''))


def life_rows(rows):
    """Merge per-body snapshots, preserving birth attribution across resumes."""
    merged = {}
    for row in sorted(rows, key=lambda r: (number(r, 'segment', 0), number(r, 'last_seen_minute', 0))):
        key = row.get('id') or (row.get('lineage'), row.get('origin'), row.get('born_minute'), row.get('parent_id'))
        previous = merged.get(key)
        if previous is None:
            merged[key] = dict(row)
            continue
        if previous['lineage'] != row['lineage']:
            raise ValueError('life id changed lineage')
        updated = dict(previous)
        updated.update({k: v for k, v in row.items() if v != ''})
        for field in ('born_minute', 'matured_minute', 'independent_minute', 'died_minute'):
            if previous.get(field, '') != '':
                updated[field] = previous[field]
        if previous.get('origin') in ('born', 'hatched'):
            updated['origin'] = previous['origin']
            updated['native'] = previous.get('native', 'true')
        merged[key] = updated
    return list(merged.values())


def within(value, start, end):
    return value is not None and start <= value < end


def born(row):
    return flag(row, 'native') and row.get('origin', '').lower() in ('born', 'hatched')


def slope(rows, time_key, value_key, end, reference=1):
    points = [(number(row, time_key), number(row, value_key)) for row in rows
              if end - 30 <= number(row, time_key) <= end and number(row, value_key) is not None]
    if len(points) < 2 or not reference:
        return None
    x, y = zip(*points)
    xm, ym = statistics.mean(x), statistics.mean(y)
    denominator = sum((value - xm)**2 for value in x)
    return sum((a - xm) * (b - ym) for a, b in points) / denominator / reference if denominator else None


def require_columns(rows, columns, source, allow_empty=False):
    if not rows:
        raise ValueError(f'missing required {source}')
    for row in rows:
        for key in columns:
            if key not in row or (not allow_empty and number(row, key) is None):
                raise ValueError(f'{source}: missing required {key}')


def lifecycle(name, records, opening, closing, begin, end, midpoint):
    births = [row for row in records if born(row) and within(number(row, 'born_minute'), begin, end)]
    surviving = [row for row in births if number(row, 'died_minute') is None or number(row, 'died_minute') > end]
    # Counter deltas include events in the final interval; life timestamps select
    # exact [from,to) events and supply the indispensable surviving-recruit check.
    def delta(prefix):
        key = prefix + '_' + name
        a, b = number(opening, key), number(closing, key)
        return b - a if a is not None and b is not None else None
    hatchings = delta('eggs_hatched')
    live_births = delta('births')
    if live_births is None:
        live_births = sum(row.get('origin') == 'born' for row in births) if any(row.get('origin') == 'hatched' for row in records) else (max(0, len(births) - hatchings) if hatchings is not None else None)
    return dict(live_births=live_births, hatchings=hatchings, eggs_laid=delta('eggs_laid'),
                maturation=sum(within(number(row, 'matured_minute'), begin, end) for row in records),
                recruits=len(births), surviving_recruits=len(surviving),
                late_births=sum(within(number(row, 'born_minute'), midpoint, end) for row in births),
                surviving_late_recruits=sum(within(number(row, 'born_minute'), midpoint, end) for row in surviving))


def score_world(root, arm, seed, cfg, start=None, stop=None, state=None, required=None, load_minute=0):
    spec = cfg['profile']
    begin, end = load_minute + spec['from_minute'], load_minute + spec['to_minute']
    result = dict(arm=arm, seed=seed, state='missing', P=None, soft_P=None, rankable=False,
                  profile_id=cfg['profile_id'], profile_hash=cfg['profile_hash'],
                  horizon=dict(start=spec['from_minute'], end=spec['to_minute']),
                  window=dict(start=begin, end=end, midpoint=(begin + end) / 2),
                  components={}, health={}, plants={}, trends={}, stocks={}, lost=[], warnings=[])
    run = root / 'run'
    markers = [name for name in ('FAILED', 'PAUSED', 'DONE') if (run / name).exists()]
    result['state'] = markers[0].lower() if markers else ('missing' if state == 'done' else state or 'missing')
    if state in {'failed', 'paused'}:
        result['state'] = state
    if result['state'] != 'done':
        result['warnings'].append(f"run is {result['state']}; unrankable")
        return result
    try:
        if (start is not None and start != spec['from_minute']) or (stop is not None and stop != spec['to_minute']):
            raise ValueError('requested horizon differs from the objective profile')
        if not isinstance(load_minute, int) or load_minute < 0:
            raise ValueError('load minute must be a nonnegative whole number')
        required = validate_required(required or {})
        rows = read_csv(root / 'census.csv', ['sim_min', 'seed'])
        vitals = read_optional(root / 'vitals/vitals.csv', result['warnings'])
        plants = read_optional(root / 'vitals/plants_vitals.csv', result['warnings'])
        lives = read_optional(root / 'vitals/life.csv', result['warnings'])
        for source, data in (('census', rows), ('vitals', vitals), ('plants', plants), ('life', lives)):
            validate_numbers(data, source)
        rows = minute_rows(rows, 'sim_min')
        vitals = minute_rows(vitals, 'minute', 'lineage')
        plants = minute_rows(plants, 'minute', 'species')
        lives = life_rows(lives)
        if vitals and max(number(row, 'minute') for row in vitals) != end:
            raise ValueError('vitals observed horizon differs from profile')
        if plants and max(number(row, 'minute') for row in plants) != end:
            raise ValueError('plant vitals observed horizon differs from profile')
        # A fresh founding must report the batch seed. A saved start reports the seed its
        # world was founded on, which differs from the start's label when the founding
        # redrew a refused seed (seed + k): it must be one seed throughout, recorded.
        if not rows:
            raise ValueError('empty census')
        world_seeds = {number(row, 'seed') for row in rows}
        if len(world_seeds) != 1:
            raise ValueError('census seed changes within the run')
        if load_minute == 0 and world_seeds != {seed}:
            raise ValueError('census seed differs from batch seed')
        result['world_seed'] = next(iter(world_seeds))
        if result['world_seed'] != seed:
            result['warnings'].append(f"start labelled s{seed} holds world seed {result['world_seed']:.0f} (founding redrew a refused seed)")
        minutes = [number(row, 'sim_min') for row in rows]
        # A later invocation may have appended beyond the profile: it is a
        # different observed horizon, never silently clipped to fit this trial.
        if minutes[-1] != end or minutes[0] not in (load_minute, load_minute + 1):
            raise ValueError('observed horizon differs from profile/load boundary')
        if minutes[0] == load_minute + 1 and required.get('load_census'):
            boundary = {key: str(value) for key, value in required['load_census'].items()}
            if number(boundary, 'sim_min') != load_minute or number(boundary, 'seed') != seed:
                raise ValueError('declared load census has wrong minute or seed')
            validate_numbers([boundary], 'load census')
            rows.insert(0, boundary)
            minutes.insert(0, load_minute)
            result['warnings'].append('legacy load boundary recovered from the start founding run')
        old_boundary = minutes[0] == load_minute + 1
        if old_boundary:
            result['warnings'].append('legacy census omits load boundary; first counter interval unavailable')
        expected = set(range(int(minutes[0]), int(end) + 1))
        if set(minutes) != expected:
            raise ValueError('census has minute gaps')
        # For profiles starting after load, old runs can still have all boundaries.
        by_minute = {number(row, 'sim_min'): row for row in rows}
        opening = by_minute.get(begin)
        closing = by_minute[end]
        midpoint = (begin + end) / 2
        window = [row for row in rows if within(number(row, 'sim_min'), begin, end)]
        late = [row for row in window if within(number(row, 'sim_min'), midpoint, end)]
        if not late:
            raise ValueError('no late census observations')
        lineages = sorted(key.removeprefix('founders_') for key in rows[0] if key.startswith('founders_'))
        species = sorted(key.removeprefix('stands_') for key in rows[0] if key.startswith('stands_'))
        animals, required_plants = set(required['animals']), set(required['plants'])
        if animals - set(lineages) or required_plants - set(species):
            raise ValueError('required names absent from census columns')
        stock_keys = [prefix + name for prefix, names in (('founders_', lineages), ('eggs_', lineages), ('stands_', species)) for name in names]
        require_columns(rows, stock_keys, 'census stocks')
        # Preserve a loss before load, even if an arm subsequently introduces bodies.
        initial = rows[0]
        lost = [f'animal:{name}' for name in sorted(animals) if any(
            number(row, 'founders_' + name) + number(row, 'eggs_' + name) == 0 for row in (initial, closing))]
        lost += [f'plant:{name}' for name in sorted(required_plants)
                 if any(number(row, 'stands_' + name) == 0 for row in (initial, closing))]
        result['lost'] = lost
        weights, targets, pop = cfg['weights'], cfg['targets'], cfg['population']
        components = result['components']

        def put(term, raw, penalty, **details):
            active = term in spec['active']
            components[term] = component(raw, penalty, weights[term] if active else 0,
                active=active, label=LABELS.get(term, term), **details)

        put('L', len(lost), len(lost), lost=lost, required_animals=sorted(animals), required_plants=sorted(required_plants))
        active_names = sorted(animals | {name for name in lineages if any(number(row, 'founders_' + name) > 0 for row in window)})
        per_j, per_a, per_k, fullness, condition, stuck = {}, {}, {}, {}, {}, {}
        stage_penalties = {}
        body_exposed = set()
        for name in active_names:
            reference = required['N_ref'].get(name)
            if reference is None:
                raise ValueError(f'active lineage {name} has no declared N_ref')
            vr = [r for r in vitals if r['lineage'] == name and begin < number(r, 'minute') <= end]
            needs_vitals = any(requirement.startswith('vitals:') for term in spec['active'] for requirement in spec['required_data'][term])
            if (needs_vitals or vr) and set(number(r, 'minute') for r in vr) != set(range(begin + 1, end + 1)):
                raise ValueError(f'vitals minute gaps/missing required lineage {name}')
            vl = [r for r in vr if within(number(r, 'minute'), midpoint, end)]
            intervals = [r for r in vr if midpoint < number(r, 'minute') <= end]
            lr = [r for r in lives if r['lineage'] == name]
            life = lifecycle(name, lr, opening or initial, closing, begin, end, midpoint)
            eligible_values = [number(r, 'breeding_eligible_n') for r in vr]
            eligible = any(value > 0 for value in eligible_values if value is not None) if all(value is not None for value in eligible_values) else None
            slow = name in spec['slow']
            jp = reproductive_penalty(life['surviving_late_recruits'], eligible, cfg['reproduction'])
            per_j[name] = dict(**life, eligible=eligible, penalty=jp, weight=0 if slow else 1,
                               flag='lifecycle untested in this profile' if slow else '')
            counts = [number(row, 'founders_' + name) for row in late]
            peaks = [number(row, 'founders_' + name) for row in window] + [number(closing, 'founders_' + name)]
            per_a[name] = dict(N_ref=reference, raw=average(counts), penalty=band_penalty(counts, reference, pop['k_hi']))
            per_k[name] = dict(N_ref=reference, raw=max(peaks), penalty=peak_penalty(peaks, reference, pop['k_peak']))
            living = [r for r in vl if number(r, 'n', 0) > 0]
            if living:
                body_exposed.add(name)
            vals = [number(r, 'fullness_p10') for r in living]
            fullness[name] = average(vals) if vals and all(v is not None for v in vals) else None
            condition[name] = average([h(v / targets['fullness_p10']) for v in vals]) if vals and all(v is not None for v in vals) else None
            stuck[name] = count_share(living, 'stuck_n', 'stuck_observed_n') if living else None
            # No living bodies is structural NA, shown beside the egg/loss
            # gate. Living bodies without stuck exposure remain unrankable.
            for r in vr:
                if number(r, 'survivor_stock_change_per_body_hour') is None:
                    r['survivor_stock_change_per_body_hour'] = r.get('net_energy_per_body_hour', '')
            for stage in ('adult', 'juvenile', 'pup', 'rider'):
                key = f'fullness_p10_{stage}'
                if living and all(key in r and 'n_' + stage in r for r in living):
                    exposed = [r for r in living if number(r, 'n_' + stage, 0) > 0]
                    values = [number(r, key) for r in exposed]
                    stage_penalties.setdefault(name, {})[stage] = (average([h(v / targets['fullness_p10']) for v in values])
                                                                   if values and all(v is not None for v in values) else None)
            if name in stage_penalties:
                exposed_stages = [stage for stage in stage_penalties[name]
                                  if any(number(r, 'n_' + stage, 0) > 0 for r in living)]
                measured = [stage_penalties[name][stage] for stage in exposed_stages]
                condition[name] = average(measured) if measured and all(v is not None for v in measured) else None
            deaths = [r for r in lr if within(number(r, 'died_minute'), begin, end)]
            energy = weighted(vr, 'cohort_energy_change', 'energy_body_hours')
            survivor = weighted(vr, 'survivor_stock_change_per_body_hour', 'energy_body_hours')
            terminal_n = number(closing, 'founders_' + name)
            trend = dict(count_slope_per_minute_per_N_ref=slope(rows, 'sim_min', 'founders_' + name, end, reference),
                         drawdown_per_N_ref=(max(peaks) - terminal_n) / reference,
                         energy_trend=energy, energy_trend_kind='conserved cohort budget' if energy is not None else 'NA; survivor stock change is separate')
            result['trends'][name] = trend
            health = dict(alive_end=terminal_n, eggs_end=number(closing, 'eggs_' + name),
                          survivors_by_minute=[dict(minute=number(r, 'minute'), n=number(r, 'n')) for r in vl],
                          events=life, deaths={cause: sum(r.get('cause') == cause for r in deaths) for cause in sorted({r.get('cause', '') for r in deaths})},
                          fullness_p10=fullness[name], fullness_p50=weighted(vl, 'fullness_p50'),
                          survivor_stock_change_per_body_hour=survivor, cohort_energy_change=energy,
                          delivered_bl_per_s=weighted(intervals, 'delivered_bl_per_s', 'awake_body_s'),
                          moving_share=weighted(intervals, 'moving_share', 'awake_body_s'),
                          awake_body_s=sum(number(r, 'awake_body_s', 0) for r in vr),
                          intake_organic=sum(number(r, 'intake_organic', 0) for r in vr) if all('intake_organic' in r for r in vr) else None,
                          stuck_observed_n=sum(number(r, 'stuck_observed_n', 0) for r in vl),
                          stuck_share=count_share(vl, 'stuck_n', 'stuck_observed_n'),
                          stages={stage: number(vr[-1] if vr else {}, 'n_' + stage) for stage in ('adult', 'juvenile', 'pup', 'rider')},
                          stage_fullness_penalty=stage_penalties.get(name),
                          stage_note='per-stage fullness measured' if name in stage_penalties else 'F pooled: stage counts alone cannot reconstruct per-stage quantiles',
                          flag=per_j[name]['flag'], **trend)
            result['health'][name] = health
        j_values = [v['penalty'] for v in per_j.values() if v['weight']]
        put('J', per_j, average(j_values) if all(v is not None for v in j_values) else None)
        if not j_values:
            components['J']['penalty'] = 0
            components['J']['contribution'] = 0
        put('Q', None, None, note=spec['disabled'].get('Q', 'cycle books unavailable'))
        put('A', per_a, average([v['penalty'] for v in per_a.values()]))
        put('K', per_k, average([v['penalty'] for v in per_k.values()]))
        moving = result['health'].get(targets['moving_lineage'], {}).get('moving_share')
        put('V', moving, h(moving / targets['moving_share']) if moving is not None else None)
        hunter = targets['predator_lineage']
        shares = []
        for row in late:
            total = sum(number(row, 'founders_' + name) for name in lineages)
            free = max(0, number(row, 'founders_' + hunter, 0) - number(row, 'riders_now', 0))
            shares.append(free / total if total else 0)
        put('Z', average(shares), average([d(value, targets['predator_share_lo'], targets['predator_share_hi']) for value in shares]),
            note='free bodies exclude riders; mean per-minute band penalty')
        meta_path = root / 'vitals/meta.json'
        meta = json.loads(meta_path.read_text()) if meta_path.exists() else {}
        voxel_m = meta.get('voxel_m', targets['voxel_m'])
        if not isinstance(voxel_m, (int, float)) or not math.isfinite(voxel_m) or voxel_m <= 0:
            raise ValueError('invalid meta voxel_m')
        if 'voxel_m' not in meta:
            result['warnings'].append('geometry fallback: voxel_m from scoring config')
        if 'seed' in meta and meta['seed'] != seed:
            raise ValueError('vitals metadata seed differs from batch seed')
        for term, key, target in (('M', 'meadow_patch_max', 'meadow_m2'), ('G_b', 'grove_patch_max', 'grove_m2')):
            values = [number(row, key) for row in late]
            area = statistics.median(values) * voxel_m**2 if values and all(v is not None for v in values) else None
            put(term, area, h(area / targets[target]) if area is not None else None, unit='m2', voxel_m=voxel_m)
        recruits = (number(closing, 'recruits_vaulttree') - number(opening, 'recruits_vaulttree')) if opening and all(number(row, 'recruits_vaulttree') is not None for row in (opening, closing)) else None
        put('G_v', recruits, h(recruits / targets['vaulttree_recruits']) if recruits is not None else None)
        kill_keys = [key for key in initial if any(key.startswith('kills_' + hunter + '_') for hunter in targets['hunt_lineages'])]
        kills = sum(number(closing, key) - number(opening, key) for key in kill_keys) if kill_keys and opening else None
        rate = kills / ((end - begin) / 60) if kills is not None else None
        put('H', rate, d(rate, targets['hunts_per_hour_lo'], targets['hunts_per_hour_hi']) if rate is not None else None, unit='kills/world-hour')
        f_values = [condition[name] for name in sorted(body_exposed)]
        # A lineage observed with no eligible stuck exposure (asleep, fed or voluntarily
        # resting through every trailing window) was not stuck: it counts 0 and is listed.
        # Missing vitals are caught by the required-data check, not here.
        unexposed = sorted(name for name in body_exposed if stuck[name] is None)
        s_values = [0.0 if stuck[name] is None else stuck[name] for name in sorted(body_exposed)]
        structural_na = sorted(set(active_names) - body_exposed)
        put('F', fullness, average(f_values) if all(v is not None for v in f_values) else None,
            per_lineage_penalty=condition, per_stage_penalty=stage_penalties,
            structural_na_lineages=structural_na, note='No living bodies: structural NA; retained in the loss/egg-bank gate')
        put('S', stuck, average(s_values) if s_values else None,
            structural_na_lineages=structural_na, unexposed_lineages=unexposed,
            note='No living bodies: structural NA; a living lineage with no stuck exposure counts 0 (listed as unexposed)')
        for name in species:
            pr = [r for r in plants if r['species'] == name and begin < number(r, 'minute') <= end]
            if plants and name in required_plants and set(number(r, 'minute') for r in pr) != set(range(begin + 1, end + 1)):
                raise ValueError(f'plant vitals minute gaps/missing required species {name}')
            count_key = 'stands_' + name
            values = [number(row, count_key) for row in window] + [number(closing, count_key)]
            result['plants'][name] = dict(alive_end=number(closing, count_key),
                stand_slope_per_minute=slope(rows, 'sim_min', count_key, end),
                drawdown=max(values) - number(closing, count_key),
                vigour_mean_end=number(pr[-1], 'vigour_mean') if pr else None,
                vigour_trend_per_minute=slope(pr, 'minute', 'vigour_mean', end) if pr else None,
                observations=len(pr))
        roles = {'litter': 'shredders: access and intake', 'dead_wood': 'decomposers: recycling',
                 'carrion': 'scavengers: actual meals', 'standing_foliage': 'browser demand, woody competition and habitat',
                 'stored': 'organic stores: recycling pathways'}
        for key, role in roles.items():
            a, b = number(opening or initial, key), number(closing, key)
            if a is not None and b is not None:
                result['stocks'][key] = dict(start=a, end=b, change=b - a,
                    terminal_slope_per_minute=slope(rows, 'sim_min', key, end), answering_role=role)
        # Validate declared sources/columns independently of whether a metric
        # happens to have a computable mean from sparse data.
        check_required_data(spec, rows, vitals, lives, active_names, begin, end, targets)
        missing = [term for term in spec['active'] if components[term]['penalty'] is None]
        result['missing_components'] = missing
        if missing:
            raise ValueError('missing active component exposure/data: ' + ', '.join(missing))
        result['soft_P'] = sum(item['contribution'] for term, item in components.items() if term != 'L')
        result.update(P=result['soft_P'] + components['L']['contribution'], rankable=True)
    except (OSError, ValueError, TypeError, KeyError) as error:
        result.update(P=None, soft_P=None, rankable=False, state='unrankable')
        result['warnings'].append(str(error))
    return result


def check_required_data(spec, rows, vitals, lives, names, begin, end, targets):
    selected = [row for row in rows if begin <= number(row, 'sim_min') <= end]
    vr = [row for row in vitals if begin < number(row, 'minute') <= end and row['lineage'] in names]
    for term in spec['active']:
        for requirement in spec['required_data'][term]:
            source, key = requirement.split(':', 1)
            if source == 'census' and key == 'stocks':
                continue  # Complete stock matrix checked before calculating L.
            if source == 'census' and key == 'kills':
                keys = [key for key in rows[0] if any(key.startswith('kills_' + hunter + '_') for hunter in targets['hunt_lineages'])]
                if not keys:
                    raise ValueError('missing required census kill counters')
                require_columns(selected, keys, 'census')
            elif source == 'census' and key == 'lifecycle':
                for name in names:
                    if name in spec['slow']:
                        continue
                    # Egg laying is distinct; live-bearing lineages legitimately
                    # have no egg counters. Require an actual recruitment counter.
                    keys = [prefix + '_' + name for prefix in ('births', 'eggs_hatched')
                            if prefix + '_' + name in rows[0]]
                    if not keys:
                        raise ValueError(f'missing lifecycle counters for {name}')
                    require_columns(selected, keys, 'census lifecycle')
            elif source == 'life' and key == 'recruits':
                # A header-only life file may have no recruits; the caller must
                # still have read a real file, rather than synthesize births.
                if not lives:
                    raise ValueError('missing life records for surviving-recruit check')
                for row in lives:
                    if not {'lineage', 'native', 'origin', 'born_minute', 'died_minute'} <= row.keys():
                        raise ValueError('incomplete life schema')
            elif source == 'vitals':
                require_columns(vr, [key], 'vitals', allow_empty=key in ('fullness_p10', 'moving_share', 'stuck_n', 'stuck_observed_n'))
                for row in vr:
                    exposed = number(row, 'n', 0) > 0
                    if key == 'moving_share':
                        exposed = number(row, 'awake_body_s', 0) > 0
                    if key in ('stuck_n', 'stuck_observed_n'):
                        # Empty shares at zero observed exposure remain NA.
                        exposed = number(row, 'stuck_observed_n', 0) > 0
                    if exposed and number(row, key) is None:
                        raise ValueError(f'missing {key} for exposed lineage {row["lineage"]}')
            elif source == 'census':
                require_columns(selected, [key], 'census')
            else:
                raise ValueError(f'unsupported required data: {requirement}')

def arm_summary(worlds, cfg, baseline_worlds=None):
    complete = bool(worlds) and all(world['rankable'] for world in worlds)
    if baseline_worlds is not None and {world['seed'] for world in worlds} != set(baseline_worlds):
        complete = False
    values = [world['soft_P'] for world in worlds]
    mean = statistics.mean(values) if complete else None
    sd = statistics.stdev(values) if complete and len(values) > 1 else (0 if complete else None)
    loss_vector = {str(world['seed']): world['lost'] for world in worlds}
    new_losses = {}
    for world in worlds:
        baseline = baseline_worlds.get(world['seed']) if baseline_worlds is not None else None
        if baseline_worlds is not None and (baseline is None or not baseline['rankable']):
            complete = False
        allowed = set(baseline['lost']) if baseline else set()
        new_losses[str(world['seed'])] = sorted(set(world['lost']) - allowed)
    feasible = complete and not any(new_losses.values())
    soft_rank = mean + cfg['summary']['sd_weight'] * sd if complete else None
    total_losses = sum(len(world['lost']) for world in worlds)
    rank_key = [0 if feasible else 1 if complete else 2, 0 if feasible else total_losses,
                soft_rank if soft_rank is not None else None]
    worst = max(worlds, key=lambda world: (len(new_losses[str(world['seed'])]), len(world['lost']),
                                          world['soft_P'] if world['soft_P'] is not None else math.inf)) if worlds else None
    return dict(feasible=feasible, rankable=complete, loss_vector=loss_vector, new_losses=new_losses,
                total_losses=total_losses, mean_P=mean, sd_P=sd, rank=soft_rank, rank_key=rank_key,
                worst_seed=worst['seed'] if worst else None,
                invalid_seeds=sorted({world['seed'] for world in worlds if not world['rankable']}
                                     | (set(baseline_worlds or {}) - {world['seed'] for world in worlds})),
                components={term: dict(stats([world['components'].get(term, {}).get('penalty') for world in worlds]),
                                       raw=stats([world['components'].get(term, {}).get('raw') for world in worlds
                                                  if isinstance(world['components'].get(term, {}).get('raw'), (int, float))]))
                            for term in COMPONENTS})


def pair_worlds(worlds, baseline, cfg):
    by_arm = {}
    for world in worlds:
        by_arm.setdefault(world['arm'], {})[world['seed']] = world
    if baseline not in by_arm:
        raise ValueError(f'unknown baseline arm: {baseline}')
    result = {}
    for arm, candidates in by_arm.items():
        if arm == baseline:
            continue
        pairs = []
        for seed in sorted(set(candidates) | set(by_arm[baseline])):
            a, b = candidates.get(seed), by_arm[baseline].get(seed)
            valid = a is not None and b is not None and a['rankable'] and b['rankable']
            if a and b and (a['profile_id'], a['profile_hash'], a['horizon'], a['window']) != (b['profile_id'], b['profile_hash'], b['horizon'], b['window']):
                raise ValueError(f'seed {seed}: refused pair with different profile or horizon')
            deltas, contributions = {}, {}
            for term in COMPONENTS:
                av = a['components'].get(term, {}) if a else {}
                bv = b['components'].get(term, {}) if b else {}
                deltas[term] = av['penalty'] - bv['penalty'] if valid and av.get('penalty') is not None and bv.get('penalty') is not None else None
                contributions[term] = av['contribution'] - bv['contribution'] if valid and term != 'L' else None
            pairs.append(dict(seed=seed, delta_P=a['soft_P'] - b['soft_P'] if valid else None,
                              components=deltas, contribution_deltas=contributions, valid=valid,
                              new_losses=sorted(set(a['lost']) - set(b['lost'])) if a and b else None))
        deltas = [pair['delta_P'] for pair in pairs if pair['valid']]
        worst = max(pairs, key=lambda pair: (len(pair['new_losses'] or []), pair['delta_P'] if pair['delta_P'] is not None else math.inf)) if pairs else None
        moves = []
        for term in cfg['profile']['active']:
            if term == 'L':
                continue
            values = [pair['contribution_deltas'][term] for pair in pairs if pair['valid']]
            if values and statistics.mean(values) != 0:
                moves.append(dict(component=term, mean_contribution_delta=statistics.mean(values)))
        moves.sort(key=lambda item: abs(item['mean_contribution_delta']), reverse=True)
        result[arm] = dict(baseline=baseline, pairs=pairs,
            wins=sum(pair['valid'] and pair['delta_P'] < 0 for pair in pairs), n=len(pairs),
            invalid=sum(not pair['valid'] for pair in pairs),
            note='Consistency count, not a significance test (P(>=6/8 fair signs) = 14.5%).',
            median_delta=statistics.median(deltas) if deltas else None,
            worst_seed=worst['seed'] if worst else None, movers=moves[:3])
    return result


def manifest_required(manifest, seed):
    block = manifest.get('required', {})
    return block.get('by_seed', {}).get(str(seed), block)


def build_report(out, cfg, baseline=None, start=None, stop=None, allow_mixed_binaries=False):
    paths = [out] if isinstance(out, Path) else list(out)
    manifests = [json.loads((path / 'batch.json').read_text()) for path in paths]
    hashes = {manifest.get('binary_sha256') for manifest in manifests}
    if len(hashes) > 1 and not allow_mixed_binaries:
        raise ValueError('batch binary hashes differ; use --allow-mixed-binaries to compare explicitly')
    if len({path.name for path in paths}) != len(paths):
        raise ValueError('batch directory names must be unique for comparison')
    worlds, inputs, arm_names = [], {}, []
    for out, manifest in zip(paths, manifests):
        if manifest.get('start_kind') == 'fresh founding' and cfg['profile']['start_kind'] == 'mature saved start':
            raise ValueError('this profile requires mature saved starts, not a fresh founding batch')
        if manifest.get('start_kind') == 'saved start' and cfg['profile']['start_kind'] == 'fresh founding':
            raise ValueError('this profile scores fresh foundings, not a saved-start batch')
        states = {}
        if (out / 'STATE.tsv').exists():
            with (out / 'STATE.tsv').open(newline='') as stream:
                for row in csv.DictReader(stream, delimiter='\t'):
                    states[(row['arm'], int(row['seed']))] = row['state']
        for job in manifest.get('runs', []):
            common = [arg for arg in job['command'] if arg.startswith(('load=', 'seed=', 'config='))]
            key = (str(out), job['seed'])
            if key in inputs and inputs[key] != common:
                raise ValueError(f"seed {job['seed']} has different starts across arms")
            inputs[key] = common
        for arm in manifest['arms']:
            label = f'{out.name}:{arm}' if len(paths) > 1 else arm
            arm_names.append(label)
            for seed in manifest['seeds']:
                load = manifest.get('load_minutes', {}).get(str(seed), 0)
                if not isinstance(load, int) or load < 0:
                    raise ValueError('manifest load_minutes must be nonnegative integers')
                world = score_world(out / arm / f's{seed}', label, seed, cfg, start, stop,
                                    states.get((arm, seed)), manifest_required(manifest, seed), load)
                identity = manifest.get('identity', {})
                world['start_identity'] = identity.get('starts', {}).get(str(seed)) or (identity.get('config') or {}).get('sha256')
                roster = manifest_required(manifest, seed)
                world['required_identity'] = {key: roster.get(key) for key in ('animals', 'plants', 'N_ref')}
                worlds.append(world)
    baseline_worlds = {world['seed']: world for world in worlds if world['arm'] == baseline} if baseline else None
    if baseline and not baseline_worlds:
        raise ValueError(f'unknown baseline arm: {baseline}; arms: {arm_names}')
    if baseline:
        for world in worlds:
            reference = baseline_worlds.get(world['seed'])
            if reference and (world['start_identity'] != reference['start_identity'] or world['required_identity'] != reference['required_identity']):
                raise ValueError(f"seed {world['seed']} has different start/config contents or required roster")
    arms = {arm: arm_summary([world for world in worlds if world['arm'] == arm], cfg, baseline_worlds) for arm in arm_names}
    ranking = sorted(arms, key=lambda arm: (arms[arm]['rank_key'][0], arms[arm]['rank_key'][1],
                                         arms[arm]['rank'] if arms[arm]['rank'] is not None else math.inf, arm))
    return dict(profile_id=cfg['profile_id'], profile_hash=cfg['profile_hash'],
                profile=cfg['profile'], objective='external census promotion gate; Rust search objective is separate',
                weights=cfg['weights'], binary_hashes=sorted(str(value) for value in hashes),
                mixed_binaries=len(hashes) > 1, worlds=worlds, arms=arms, ranking=ranking,
                paired=pair_worlds(worlds, baseline, cfg) if baseline else {})


def display(value):
    if value is None:
        return 'NA'
    if isinstance(value, bool):
        return str(value)
    if isinstance(value, (int, float)):
        return f'{value:.6g}'
    if isinstance(value, (dict, list)):
        return json.dumps(value, separators=(',', ':'))
    return str(value)


def records(report):
    yield ('profile', '', '', report['profile_id'], report['profile_hash'], None, None, None)
    for arm in report['ranking']:
        summary = report['arms'][arm]
        yield ('arm', arm, '', 'required_losses', summary['loss_vector'], None, None, None)
        yield ('arm', arm, '', 'feasible', summary['feasible'], None, None, None)
    for world in report['worlds']:
        for term, item in world['components'].items():
            yield ('world', world['arm'], world['seed'], term, item['raw'], item['penalty'], item['weight'], item['contribution'])
        yield ('world', world['arm'], world['seed'], 'soft_P', world['soft_P'], None, None, world['state'])
        for kind in ('health', 'plants', 'trends', 'stocks'):
            for name, data in world[kind].items():
                yield (kind, world['arm'], world['seed'], name, data, None, None, None)
        for warning in world['warnings']:
            yield ('warning', world['arm'], world['seed'], 'note', warning, None, None, None)
    for arm, paired in report['paired'].items():
        for pair in paired['pairs']:
            yield ('pair', arm, pair['seed'], 'delta_soft_P', pair['delta_P'], None, None, paired['baseline'])
        yield ('pair', arm, '', 'median_delta', paired['median_delta'], None, None, None)
        yield ('pair', arm, '', 'wins', paired['wins'], paired['note'], None, paired['n'])


def table(headers, rows):
    lines = ['| ' + ' | '.join(headers) + ' |', '| ' + ' | '.join('---' for _ in headers) + ' |']
    lines.extend('| ' + ' | '.join(map(display, row)) + ' |' for row in rows)
    return lines


def health_tables(worlds):
    """Compact seed medians and ranges; all raw readings remain in JSON/TSV."""
    def spread(values):
        values = [value for value in values if value is not None]
        if not values:
            return 'NA'
        return f'{display(statistics.median(values))} [{display(min(values))},{display(max(values))}]'

    names = sorted({name for world in worlds for name in world['health']})
    lines = ['Animal health: seed median [min,max]. F uses per-minute p10 and shows its survivor count range.', '']
    counts, activity, trends = [], [], []
    for name in names:
        rows = [world['health'][name] for world in worlds if name in world['health']]
        metric = lambda key: spread([row.get(key) for row in rows])
        stage = lambda key: spread([row['stages'].get(key) for row in rows])
        survivor_counts = [item['n'] for row in rows for item in row['survivors_by_minute']]
        survivor_range = f'{display(min(survivor_counts))}–{display(max(survivor_counts))}' if survivor_counts else 'NA'
        counts.append([name, metric('alive_end'), stage('adult'), stage('juvenile'), stage('pup'), metric('eggs_end'), metric('fullness_p10'), survivor_range])
        activity.append([name, metric('awake_body_s'), metric('moving_share'), metric('delivered_bl_per_s'), metric('intake_organic'), metric('stuck_share'), metric('stuck_observed_n')])
        trends.append([name, metric('count_slope_per_minute_per_N_ref'), metric('drawdown_per_N_ref'), metric('cohort_energy_change'), metric('survivor_stock_change_per_body_hour')])
    lines += table(['Lineage', 'Alive end', 'Adult', 'Juvenile', 'Pup', 'Eggs', 'Fullness p10', 'Survivor n'], counts)
    lines += [''] + table(['Lineage', 'Awake body-s', 'Moving share', 'Delivered BL/s', 'Intake organic', 'Stuck share', 'Stuck observations'], activity)
    lines += [''] + table(['Lineage', 'Last 30m count slope/min/N_ref', 'Peak drawdown/N_ref', 'Cohort energy trend/body-hour', 'Survivor stock change/body-hour'], trends)
    for name in names:
        rows = [world['health'][name] for world in worlds if name in world['health']]
        events = {key: spread([row['events'].get(key) for row in rows])
                  for key in ('live_births', 'hatchings', 'eggs_laid', 'maturation', 'surviving_recruits')}
        deaths = {cause: spread([row['deaths'].get(cause, 0) for row in rows])
                  for cause in sorted({cause for row in rows for cause in row['deaths']})}
        notes = sorted({row['flag'] for row in rows if row['flag']} | {row['stage_note'] for row in rows})
        lines += ['', f'**{name}** lifecycle: {display(events)}; deaths: {display(deaths)}. ' + '; '.join(notes) + '.']
        if any(row['stage_fullness_penalty'] for row in rows):
            lines += ['Stage F penalties: ' + display([row['stage_fullness_penalty'] for row in rows])]
    plants = []
    for name in sorted({name for world in worlds for name in world['plants']}):
        rows = [world['plants'][name] for world in worlds if name in world['plants']]
        plants.append([name] + [spread([row.get(key) for row in rows]) for key in
                      ('alive_end', 'stand_slope_per_minute', 'drawdown', 'vigour_mean_end', 'vigour_trend_per_minute')])
    lines += ['', 'Plant health: seed median [min,max]. NA means unavailable.', '']
    lines += table(['Species', 'Stands end', 'Last 30m stand slope/min', 'Drawdown', 'Vigour end', 'Vigour trend/min'], plants)
    stocks = []
    for name in sorted({name for world in worlds for name in world['stocks']}):
        rows = [world['stocks'][name] for world in worlds if name in world['stocks']]
        stocks.append([name, spread([r['change'] for r in rows]), spread([r['terminal_slope_per_minute'] for r in rows]), rows[0]['answering_role']])
    if stocks:
        lines += ['', 'Unchecked stocks and the role to inspect (observed changes, not a tuned verdict):', '']
        lines += table(['Stock', 'Start-to-end change', 'Last 30m slope/min', 'Answering role'], stocks)
    return lines


def markdown(report):
    lines = [f"# Census screen: {report['profile_id']}", '', f"Profile SHA256: `{report['profile_hash']}`.",
             report['objective'] + '.',
             f"Horizon relative to load: [{report['profile']['from_minute']}, {report['profile']['to_minute']}) whole minutes.",
             'Rank: feasibility first; infeasible arms by total required losses, then mean + w × sample SD of soft P (L excluded).',
             'Weights/targets are provisional. Trend weights are zero in this profile.',
             'Active mask/weights: ' + display({term: report['weights'][term] for term in report['profile']['active']}),
             'Disabled: ' + display(report['profile']['disabled']), '']
    if report['mixed_binaries']:
        lines += ['Explicit mixed-binary comparison: ' + display(report['binary_hashes']), '']
    # Loss vectors precede every component score, including the baseline.
    lines += table(['Arm', 'Feasible', 'Rankable', 'Per-seed required losses', 'Worst seed'],
                   [[arm, report['arms'][arm]['feasible'], report['arms'][arm]['rankable'],
                     report['arms'][arm]['loss_vector'], report['arms'][arm]['worst_seed']] for arm in report['ranking']])
    for arm in report['ranking']:
        summary = report['arms'][arm]
        lines += ['', f'## {arm}', '',
                  f"Feasible: **{summary['feasible']}**; rankable: {summary['rankable']}; soft mean: {display(summary['mean_P'])}; SD: {display(summary['sd_P'])}; soft rank: {display(summary['rank'])}; invalid seeds: {summary['invalid_seeds']}.",
                  'New losses relative to baseline: ' + display(summary['new_losses']),
                  f"Worst seed: s{summary['worst_seed']}.", '']
        lines += table(['Component', 'Raw median', 'Median penalty', 'Min', 'Max', 'Seeds'],
                       [[LABELS.get(term, term), item['raw']['median'], item['median'], item['min'], item['max'], item['n']]
                        for term, item in summary['components'].items()])
        paired = report['paired'].get(arm)
        if paired:
            lines += ['', f"Paired with {paired['baseline']}: wins {paired['wins']}/{paired['n']}; invalid {paired['invalid']}; median Δ soft P {display(paired['median_delta'])}; worst seed s{paired['worst_seed']}.",
                      paired['note'], 'Components moving the soft ranking: ' + display(paired['movers'])]
        arm_worlds = [world for world in report['worlds'] if world['arm'] == arm]
        lines += [''] + health_tables(arm_worlds)
        for world in arm_worlds:
            for warning in world['warnings']:
                lines += ['', f"s{world['seed']}: {warning}"]
        if len(arm_worlds) > 1:
            worst = [world for world in arm_worlds if world['seed'] == summary['worst_seed']]
            lines += ['', f"### Worst seed s{summary['worst_seed']}", ''] + health_tables(worst)
    lines += ['', 'Read stock changes with the answering role and accessible intake: litter → shredders; fungal caps → capgnawers; carrion → scavengers; excess prey → hunters; foliage/meadow decline → browser demand, woody competition and habitat; immobilized nutrients → recycling. This screen does not establish seasonal replacement.']
    return '\n'.join(lines) + '\n'


def plain(report):
    print(markdown(report))


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('out', type=Path, nargs='+', help='one or more batches; cross-batch arms are BATCH:ARM')
    parser.add_argument('--config', type=Path, default=DEFAULT_CONFIG)
    parser.add_argument('--profile', default='screen-2h')
    parser.add_argument('--baseline')
    parser.add_argument('--allow-mixed-binaries', action='store_true')
    parser.add_argument('--report', type=Path, metavar='FILE.md')
    parser.add_argument('--from', dest='start', type=int, metavar='MIN', help='must match profile')
    parser.add_argument('--to', dest='stop', type=int, metavar='MIN', help='must match profile')
    formats = parser.add_mutually_exclusive_group()
    formats.add_argument('--tsv', action='store_true')
    formats.add_argument('--json', action='store_true')
    args = parser.parse_args(argv)
    try:
        payload = args.config.read_bytes()
        cfg = profile_config(tomllib.loads(payload.decode()), args.profile, payload)
        report = build_report(args.out, cfg, args.baseline, args.start, args.stop, args.allow_mixed_binaries)
        if args.report:
            args.report.write_text(markdown(report))
        if args.json:
            print(json.dumps(report, indent=2, allow_nan=False))
        elif args.tsv:
            writer = csv.writer(sys.stdout, delimiter='\t', lineterminator='\n')
            writer.writerow(('kind', 'arm', 'seed', 'metric', 'raw_or_median', 'penalty_or_min', 'weight_or_max', 'contribution_or_n'))
            writer.writerows([[display(value) for value in row] for row in records(report)])
        else:
            plain(report)
    except (OSError, ValueError, KeyError, TypeError) as error:
        parser.error(str(error))


if __name__ == '__main__':
    main()
