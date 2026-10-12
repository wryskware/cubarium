#!/usr/bin/env python3
"""Write an ENGAGE batch spec from the harness arm list and episode count.
Usage: engagement-spec.py --probe target/release/examples/engagement_probe --film-seed N --out SPEC
The chosen film seed must come from a successful quick run's family-3 rows.
"""
import argparse
import hashlib
import json
from pathlib import Path
import shlex
import subprocess
import tomllib


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--probe', type=Path, required=True)
    parser.add_argument('--config', type=Path, default=Path('config/engagement.toml'))
    parser.add_argument('--film-seed', type=int, required=True)
    parser.add_argument('--out', type=Path, required=True)
    parser.add_argument('--name', default='engage-baseline', help='the labd job name')
    parser.add_argument('--runs', type=Path,
                        default=Path('/home/wrysk/wryskware/cubarium/art/gen/runs/engage-2026-10-12'),
                        help="the run folder under main's art/gen/runs (the batch writes <runs>/batch-dir)")
    parser.add_argument('--batch-dir', default='baseline', help='the batch output folder under --runs')
    parser.add_argument('--quick', action='store_true', help='the labd quick lane (a job within 15 minutes)')
    options = parser.parse_args()
    repo = Path(__file__).resolve().parents[2]
    cfg_path = options.config.resolve(strict=True)
    cfg = tomllib.loads(cfg_path.read_text())
    probe = options.probe.resolve(strict=True)
    digest = hashlib.sha256(probe.read_bytes()).hexdigest()
    commit = subprocess.check_output(['git', '-C', str(repo), 'rev-parse', 'HEAD'], text=True).strip()
    movie = options.runs / f"family-3-seed{options.film_seed}.mp4"
    args = ['--config', str(cfg_path), '--required', str(repo / 'config/engagement-required.toml'),
            '--seeds', f"1-{cfg['episodes']}", '--hours', str(cfg['duration_s'] / 3600), '--threads', '1']
    for arm in cfg['arms']:
        extra = shlex.join([f"arm={arm['name']}", f'probe={probe}', f'probe_sha256={digest}',
                            f'film_seed={options.film_seed}', f'film_out={movie}',
                            'render_tree=/home/wrysk/wryskware/cubarium'])
        args += ['--arm', f"{arm['name']}={extra}"]
    quick = '\nquick = true' if options.quick else ''
    text = f'''# ENGAGE source commit {commit}; binary sha256 {digest}
kind = "batch"
name = {json.dumps(options.name)}
out = {json.dumps(str(options.runs / options.batch_dir))}
hosts = ["desktop"]{quick}
parallel = 2
mem_gb = 4
bin = {json.dumps(str(repo / 'scripts/studies/engagement-task.py'))}
args = {json.dumps(args, indent=2)}
'''
    options.out.write_text(text)
    print(options.out.resolve())
    print(f'video {movie}')


if __name__ == '__main__':
    main()
