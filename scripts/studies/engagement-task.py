#!/usr/bin/env python3
"""ENGAGE adapter for the existing labd batch kind.

batch.py supplies seed/config/run per episode and placement through run-study.py.
Each child runs one complete episode; the last successful child reduces all rows
and renders the quick-run-selected seed. No staged tripwire or zero-count abort.
The bin/probe pair must be from the same committed tree. Desktop only.
"""
import fcntl
import hashlib
import json
from pathlib import Path
import statistics
import subprocess
import sys
import tomllib


def atomic(path, text):
    temp = path.with_suffix(path.suffix + '.tmp')
    temp.write_text(text)
    temp.replace(path)


def reduce(root, config):
    arms = config['arms']
    counts = config['episodes']
    rows_by_arm = {}
    for arm in arms:
        rows = []
        for seed in range(1, counts + 1):
            path = root / arm['name'] / f's{seed}' / 'outcome.json'
            if not path.is_file():
                return False
            rows.append(json.loads(path.read_text()))
        rows_by_arm[arm['name']] = rows
    header = ['arm', 'episodes', 'calf_killed_n', 'calf_killed_share', 'adult_killed_n',
              'adult_killed_share', 'pack_gave_up_n', 'pack_gave_up_share', 'timeout_n',
              'timeout_share', 'median_time_s', 'hound_wounds', 'hound_deaths', 'calf_wounds',
              'adult_wounds', 'encounters', 'tests', 'chases', 'kills', 'family_flee_ticks']
    lines = ['\t'.join(header)]
    for name, rows in rows_by_arm.items():
        atomic(root / f'{name}.jsonl', ''.join(json.dumps(r) + '\n' for r in rows))
        values = [name, len(rows)]
        for outcome in ('calf_killed', 'adult_killed', 'pack_gave_up', 'timeout'):
            n = sum(r['outcome'] == outcome for r in rows)
            values += [n, n / len(rows)]
        values += [statistics.median(r['time_s'] for r in rows)]
        values += [sum(r[k] for r in rows) for k in header[11:]]
        lines.append('\t'.join(map(str, values)))
    atomic(root / 'summary.tsv', '\n'.join(lines) + '\n')
    return True


def main(argv=None):
    args = sys.argv[1:] if argv is None else argv
    values = dict(a.split('=', 1) for a in args if '=' in a)
    config_path = Path(values['config'])
    config = tomllib.loads(config_path.read_text())
    run = Path(values['run'])
    episode_out = run.parent
    root = episode_out.parent.parent
    probe = Path(values['probe']).resolve(strict=True)
    expected = values['probe_sha256']
    if hashlib.sha256(probe.read_bytes()).hexdigest() != expected:
        raise ValueError('engagement binary changed since submission')
    command = [str(probe), f'config={config_path}', f"seed={values['seed']}",
               f"arm={values['arm']}", 'episodes=1']
    result = subprocess.run(command, check=True, capture_output=True, text=True)
    sys.stderr.write(result.stderr)
    row = json.loads(result.stdout)
    if row['seed'] != int(values['seed']) or row['arm'] != values['arm']:
        raise ValueError('wrong seed or arm in engagement row')
    atomic(episode_out / 'outcome.json', json.dumps(row) + '\n')
    print(json.dumps(row), flush=True)
    run.mkdir(parents=True, exist_ok=True)
    atomic(run / 'status', f"minute={int(row['time_s'] // 60)} outcome={row['outcome']}\n")
    # Serialize the final reduction/video, including simultaneous last completions.
    with (root / '.reduce.lock').open('w') as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        if not (root / 'summary.tsv').exists() and reduce(root, config):
            if 'film_seed' in values:
                film = Path(values['film_out'])
                film.parent.mkdir(parents=True, exist_ok=True)
                movie = subprocess.run([str(probe), f'config={config_path}',
                    f"seed={values['film_seed']}", 'arm=family-3', 'episodes=1',
                    f'video={film}'], capture_output=True, text=True)
                atomic(root / 'video.log', movie.stdout + movie.stderr)
                if movie.returncode:
                    atomic(root / 'VIDEO_FAILED', f'exit {movie.returncode}; see video.log\n')
                else:
                    atomic(root / 'VIDEO_DONE', str(film) + '\n')
    atomic(run / 'DONE', 'complete engagement episode\n')


if __name__ == '__main__':
    main()
