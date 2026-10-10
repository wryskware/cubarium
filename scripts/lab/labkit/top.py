"""Read-only terminal view of labd; no probes, control messages or kind hooks."""

from dataclasses import dataclass
import json
import math
import os
from pathlib import Path
import shutil
import time

from .hosts import Host, Inventory
from .store import Store, TERMINAL


def number(value):
    try:
        result = float(str(value).rstrip('x'))
        return result if math.isfinite(result) else None
    except (ValueError, TypeError):
        return None


def read_json(path):
    try:
        return json.loads(path.read_text())
    except (OSError, ValueError):
        return {}


def duration(seconds):
    if seconds is None:
        return '?'
    minutes = max(0, math.ceil(seconds / 60))
    return f'{minutes // 60}h{minutes % 60:02}m' if minutes >= 60 else f'{minutes}m'


@dataclass
class Progress:
    done: float | None = 0
    total: float | None = None
    rate: float | None = None       # work units per second
    estimate: bool = False

    @property
    def fraction(self):
        return min(1, max(0, self.done / self.total)) if self.total and self.done is not None else None

    @property
    def eta(self):
        if self.total is None or self.done is None:
            return None
        remaining = max(0, self.total - self.done)
        return 0 if remaining == 0 else remaining / self.rate if self.rate else None

    def label(self):
        fraction = self.fraction
        bar = '?' if fraction is None else '█' * int(fraction * 8) + '░' * (8 - int(fraction * 8))
        percent = '?' if fraction is None else f'{fraction * 100:.0f}%'
        mark = '~' if self.estimate else ''
        return f'[{bar}] {mark}{percent} ETA {mark}{duration(self.eta)}'


class Dashboard:
    def __init__(self, paths):
        self.paths = paths
        self.store = Store(paths, readonly=True)
        self.inventory = Inventory.load(paths.hosts)   # no topology subprocess or remote probes
        self.samples = {}
        self.cpu_sample = None

    def close(self):
        self.store.db.close()

    def cpu(self):
        try:
            values = list(map(int, Path('/proc/stat').read_text().splitlines()[0].split()[1:9]))
            sample = sum(values), values[3] + values[4]
        except (OSError, ValueError, IndexError):
            return None
        previous, self.cpu_sample = self.cpu_sample, sample
        if previous is None or sample[0] <= previous[0]:
            return None
        return max(0, min(100, 100 * (1 - (sample[1] - previous[1]) / (sample[0] - previous[0]))))

    def sim(self, root, start, total, state, now):
        """Absolute status minutes become work since this stage's checkpoint."""
        try:
            fields = dict(token.split('=', 1) for token in (root / 'run/status').read_text().split() if '=' in token)
        except OSError:
            fields = {}
        minute, end, speed = (number(fields.get(key)) for key in ('minute', 'end', 'speed'))
        if total is None and end is not None:
            total = max(0, end - start)
        if minute is None and state not in ('done', 'pending', 'queued'):
            return Progress(None, total)
        done = max(0, minute - start) if minute is not None else 0
        if state == 'done' and total is not None:
            done = total
        rate = speed / 60 if speed is not None and speed > 0 else None
        key = str(root)
        previous = self.samples.get(key)
        if minute is not None:
            if previous and minute > previous[1] and now > previous[0]:
                rate = (minute - previous[1]) / (now - previous[0])
                self.samples[key] = (now, minute, rate)
            elif previous and minute == previous[1]:
                rate = previous[2] or rate
            else:
                self.samples[key] = (now, minute, rate)
        return Progress(min(done, total) if total else done, total, rate)

    def batch(self, root, state, now):
        manifest = read_json(root / 'batch/batch.json')
        runs = manifest.get('runs', [])
        hours = number(manifest.get('args', {}).get('hours'))
        if not runs or not hours:
            return Progress()
        pieces = []
        for run in runs:
            start = number(manifest.get('load_minutes', {}).get(str(run['seed']), 0)) or 0
            child = root / 'batch' / run['arm'] / f"s{run['seed']}"
            # batch.json is written at stage boundaries; DONE is the current child's truth.
            child_state = 'done' if (child / 'run/DONE').exists() else 'pending'
            pieces.append(self.sim(child, start, hours * 60, child_state, now))
        total = sum(p.total for p in pieces)
        done = total if state == 'done' else sum(p.done for p in pieces)
        parallel = manifest.get('max_parallel', 1)
        rates = [p.rate for p in pieces if p.rate and p.fraction != 1]
        rate = sum(rates[:parallel]) if rates else None
        etas = [p.eta for p in pieces if p.rate and p.fraction != 1]
        if rate and etas:
            # Aggregate throughput cannot finish before the slowest active run.
            eta = max((total - done) / rate, max(etas))
            rate = (total - done) / eta if eta else rate
        return Progress(done, total, rate)

    def task(self, job, row, now):
        spec = json.loads(row['spec'])
        root = Path(spec.get('outputs') or Path(job['out']) / row['name'])
        if job['kind'] == 'batch':
            result = self.batch(root, row['state'], now)
            if result.total is not None:
                return result
        if job['kind'] == 'ab':
            job_spec = json.loads(job['spec'])
            stage = spec.get('meta', {}).get('stage')
            early = job_spec.get('early_read', True) and bool(job_spec.get('expect_metric'))
            start = job_spec.get('early_minute', 20) if stage == 'continuation' else 0
            total = (job_spec.get('early_minute', 20) if stage == 'early' and early
                     else job_spec['hours'] * 60 - start)
            return self.sim(root, start, total, row['state'], now)
        est = number(spec.get('est_h'))
        if est and est > 0:
            total = est * 3600
            elapsed = max(0, now - row['started']) if row['started'] is not None else 0
            done = total if row['state'] == 'done' else min(total * .99, elapsed)
            return Progress(done, total, 1, True)
        return Progress()

    def job(self, job, tasks, now):
        pieces = [self.task(job, row, now) for row in tasks]
        if job['kind'] == 'ab':
            spec = json.loads(job['spec'])
            if spec.get('preflight_only'):
                return Progress()
            total = len(spec['seed_list']) * len(spec['arms']) * spec['hours'] * 60
        elif pieces and all(p.total is not None for p in pieces):
            total = sum(p.total for p in pieces)
        else:
            return Progress()
        if any(p.done is None for p in pieces):
            return Progress(None, total)
        done = sum(p.done for p in pieces)
        rates = [p.rate for p, row in zip(pieces, tasks) if row['state'] == 'running' and p.rate]
        # Throughput ETA includes queued work and not-yet-spawned continuation stages.
        active = [p for p, row in zip(pieces, tasks) if row['state'] == 'running']
        rate = sum(rates) if rates and all(p.rate for p in active) else None
        if rate:
            eta = max((total - done) / rate, max((p.eta or 0 for p in active), default=0))
            rate = (total - done) / eta if eta else rate
        if job['state'] == 'done':
            done = total
        return Progress(done, total, rate, any(p.estimate for p in pieces))

    def hosts(self, tasks, cpu):
        lines = ['HOSTS (mem = labd allocated/cap GB; remote load is cached)']
        hosts = dict(self.inventory.hosts)
        ec2 = self.store.kv_get('ec2', {})
        if ec2.get('state') == 'up':
            info = self.inventory.instance_type(ec2.get('type'))
            cfg = self.inventory.ec2
            hosts['ec2'] = Host(name='ec2', kind='ec2', cpus=info.get('cpus', 64),
                                max_mem_gb=cfg.get('max_mem_gb', 64),
                                max_cpu_fraction=cfg.get('max_cpu_fraction', .9),
                                reserve_cpus=cfg.get('reserve_cpus', 1))
        for name, host in hosts.items():
            probe = self.store.kv_get(f'host:{name}', {})
            running = [json.loads(t['spec']) for t in tasks if t['state'] == 'running' and t['host'] == name]
            used = sum(t.get('threads', 1) for t in running)
            mem = sum(t.get('mem_gb', 1) for t in running)
            load = os.getloadavg()[0] if host.kind == 'local' else probe.get('load')
            up = host.kind == 'local' or probe.get('up') or (name == 'ec2' and ec2.get('state') == 'up')
            load_label = f'{load:.1f}' if isinstance(load, (int, float)) else '?'
            if probe.get('nproc'):
                host.cpus = probe['nproc']
            if host.kind == 'local':
                fraction = cpu / 100 if cpu is not None else None
                meter = Progress(fraction or 0, 1 if fraction is not None else None).label().split(' ETA')[0]
                metric = f'CPU {meter}'
            else:
                metric = f"load {load_label}/{host.cpus} cpus"
            lines.append(f"{name} {'up' if up else 'down'} {metric} thr {used}/{host.usable} mem {mem:g}/{host.max_mem_gb:g} load {load_label}")
        return lines

    def frame(self, width=80, now=None):
        now = time.time() if now is None else now
        cpu = self.cpu()
        jobs = self.store.q('SELECT * FROM jobs ORDER BY submitted DESC')
        tasks = self.store.q('SELECT * FROM tasks ORDER BY seq, name')
        lines = ['lab top | q quit | arrows scroll | refresh 2s | ~ estimate', *self.hosts(tasks, cpu),
                 'JOBS id kind state run/pend/done/bad age progress ETA']
        for job in jobs:
            if job['state'] in TERMINAL:
                continue
            mine = [t for t in tasks if t['job'] == job['id']]
            counts = [sum(t['state'] == s for t in mine) for s in ('running', 'pending', 'done')]
            counts.append(sum(t['state'] not in ('running', 'pending', 'done', 'skipped') for t in mine))
            age = duration(now - (job['submitted'] or now))
            label = self.job(job, mine, now).label()
            details = f" {job['kind']} {job['state']} {'/'.join(map(str, counts))} {age}"
            room = max(5, width - len(details) - len(label) - 1)
            identifier = job['id']
            if len(identifier) > room:
                identifier = identifier[:room - 5] + '…' + identifier[-4:]
            lines.append(f'{identifier}{details} {label}')
            for row in mine:
                if row['state'] == 'running':
                    label = self.task(job, row, now).label()
                    location = f"{row['host']}:g{row['grp'] if row['grp'] is not None and row['grp'] >= 0 else '-'}"
                    prefix = f"  {location} {row['name']}"
                    lines.append(f'{prefix[:max(10, width - len(label) - 1)]} {label}')
        incoming = self.paths.state / 'incoming'
        for spec in sorted(incoming.glob('*.json')):
            lines.append(f'{spec.stem} spooled (waiting for labd) progress ? ETA ?')
        lines.append('RECENTLY FINISHED (DONE reason)')
        recent = sorted((j for j in jobs if j['state'] in TERMINAL), key=lambda j: j['finished'] or 0, reverse=True)[:3]
        for job in recent:
            try:
                reason = (Path(job['out']) / 'DONE').read_text().strip()
            except OSError:
                reason = job['reason'] or '?'
            lines.append(f"{job['id']} {job['state']} {reason}")
        # Keep control characters from subprocess reasons out of the terminal.
        return [''.join(c if c.isprintable() else ' ' for c in line)[:width] for line in lines]


def cmd_top(args, paths):
    dashboard = Dashboard(paths)
    try:
        if args.once:
            dashboard.cpu()
            time.sleep(.1)  # first frame needs two /proc/stat samples
            print('\n'.join(dashboard.frame(width=shutil.get_terminal_size((80, 24)).columns)))
            return
        import curses

        def draw(screen):
            screen.timeout(200)
            offset, next_frame, lines = 0, 0, []
            while True:
                now = time.monotonic()
                height, width = screen.getmaxyx()
                if now >= next_frame:
                    lines = dashboard.frame(width=max(1, width - 1))
                    next_frame = now + 2
                offset = min(offset, max(0, len(lines) - max(1, height - 1)))
                screen.erase()
                for y, line in enumerate(lines[offset:offset + max(1, height - 1)]):
                    try:
                        screen.addstr(y, 0, line[:max(1, width - 1)])
                    except curses.error:
                        pass
                screen.refresh()
                key = screen.getch()
                if key in (ord('q'), ord('Q')):
                    break
                if key in (curses.KEY_DOWN, curses.KEY_NPAGE):
                    offset += 1 if key == curses.KEY_DOWN else max(1, height - 2)
                elif key in (curses.KEY_UP, curses.KEY_PPAGE):
                    offset = max(0, offset - (1 if key == curses.KEY_UP else max(1, height - 2)))
                elif key == curses.KEY_RESIZE:
                    next_frame = 0
        try:
            curses.wrapper(draw)
        except KeyboardInterrupt:
            pass
    finally:
        dashboard.close()
