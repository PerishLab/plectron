import hashlib
import json
import subprocess
import time

import application
import frames
import ipc


def cputime(value):
    days = 0
    if '-' in value:
        day, value = value.split('-', 1)
        days = int(day)
    parts = value.split(':')
    seconds = float(parts.pop())
    minutes = int(parts.pop()) if parts else 0
    hours = int(parts.pop()) if parts else 0
    return round((days * 86400 + hours * 3600 + minutes * 60 + seconds) * 1_000_000_000)


def usage(pid):
    result = subprocess.run(
        ['ps', '-o', 'rss=', '-o', 'time=', '-p', str(pid)],
        capture_output=True, text=True, timeout=5,
    )
    values = result.stdout.split()
    if result.returncode or len(values) != 2:
        raise RuntimeError(f'process usage unavailable for {pid}: {result.stderr.strip()}')
    return {'rss': int(values[0]) * 1024, 'cpu': cputime(values[1])}


def counters(snapshot):
    return {
        'frame': snapshot['frame'],
        'computation': snapshot['computation'],
        'cache': sum(entry['stats']['bytes'] for entry in snapshot['caches']),
    }


def phase(app, seconds):
    before = counters(frames.page(app))
    start = usage(app.child.pid)
    clock = time.perf_counter_ns()
    time.sleep(seconds)
    end = usage(app.child.pid)
    wall = time.perf_counter_ns() - clock
    after = counters(frames.page(app))
    computation = {
        key: value if key == 'mailbox' else value - before['computation'][key]
        for key, value in after['computation'].items()
    }
    cpu = end['cpu'] - start['cpu']
    return {
        'wall': wall,
        'cpu': cpu,
        'utilization': cpu * 100 / wall,
        'rss': {'start': start['rss'], 'end': end['rss'], 'delta': end['rss'] - start['rss']},
        'cache': {'start': before['cache'], 'end': after['cache'], 'delta': after['cache'] - before['cache']},
        'frames': after['frame'] - before['frame'],
        'computation': computation,
    }


def focus(app, previous, identifier):
    source = f'document.getElementById("{identifier}").focus(); 1;'
    frames.action(app, previous, source)
    time.sleep(0.25)
    return frames.page(app)


def capture(args, directory, backend, staticfirst):
    app = application.Application(args.binary, directory, backend, args.application)
    samples = {}
    try:
        app.ready()
        current, _, _ = frames.wait(app)
        order = ('static', 'animated') if staticfirst else ('animated', 'static')
        for name in order:
            identifier = 'task-input' if name == 'animated' else 'add'
            snapshot = frames.page(app)
            expected = next(node['id'] for node in snapshot['nodes'] if node['attributes'].get('id') == identifier)
            if snapshot['focused'] != expected:
                current = focus(app, current, identifier)
            samples[name] = phase(app, args.seconds)
            current = frames.page(app)
    finally:
        app.close()
    if samples['animated']['computation']['animations'] == 0:
        raise RuntimeError('focused input did not produce animation samples')
    if samples['static']['computation']['animations'] != 0:
        raise RuntimeError('unfocused input continued producing animation samples')
    for sample in samples.values():
        if sample['computation']['layouts'] or sample['computation']['paints']:
            raise RuntimeError('visual-only activity reconstructed layout or paint')
    result = {'backend': backend, 'order': order, 'phases': samples}
    (directory / 'result.json').write_text(json.dumps(result, indent=2) + '\n')
    return result


def distribution(samples, path):
    values = []
    for sample in samples:
        value = sample
        for key in path:
            value = value[key]
        values.append(value)
    return ipc.distribution(values)


def run(args):
    if args.rounds < 1 or args.seconds <= 0:
        raise ValueError('rounds and seconds must be positive')
    if len(set(args.backends)) != len(args.backends):
        raise ValueError('backends must be unique')
    root = args.output.resolve()
    root.mkdir(parents=True, exist_ok=False)
    binary = args.binary.resolve(strict=True)
    applicationroot = args.application.resolve(strict=True)
    rows = []
    for index in range(args.rounds):
        variants = list(args.backends)
        offset = index % len(variants)
        for backend in variants[offset:] + variants[:offset]:
            print(f'[{index + 1}/{args.rounds}] {backend}', flush=True)
            rows.append(capture(args, root / f'{index}-{backend}', backend, index % 2 == 1))
    summary = {}
    metrics = {
        'cpu': ('cpu',), 'utilization': ('utilization',), 'rss': ('rss', 'end'),
        'rss_delta': ('rss', 'delta'), 'cache': ('cache', 'end'), 'cache_delta': ('cache', 'delta'),
        'frames': ('frames',), 'animations': ('computation', 'animations'),
        'effects': ('computation', 'effects'), 'coalesced': ('computation', 'coalesced'),
        'discarded': ('computation', 'discarded'), 'submitted': ('computation', 'submitted'),
        'mailbox_peak': ('computation', 'mailbox'),
        'sampling': ('computation', 'sampling'), 'layouts': ('computation', 'layouts'),
        'paints': ('computation', 'paints'),
    }
    for backend in args.backends:
        selected = [row for row in rows if row['backend'] == backend]
        summary[backend] = {
            phase: {name: distribution([row['phases'][phase] for row in selected], path)
                    for name, path in metrics.items()}
            for phase in ('animated', 'static')
        }
    result = {
        'unit': {'cpu': 'nanoseconds', 'utilization': 'percent', 'rss': 'bytes',
                 'cache': 'bytes', 'sampling': 'microseconds'},
        'binary': str(binary), 'sha256': hashlib.sha256(binary.read_bytes()).hexdigest(),
        'application': str(applicationroot), 'rounds': args.rounds,
        'phase_seconds': args.seconds, 'summary': summary, 'rows': rows,
        'endpoint': 'same-process quiet interval bounded by process CPU/RSS and page snapshots; no IPC during the interval',
        'limits': ['ps reports current resident memory and cumulative process CPU; endpoint samples are not peak RSS.',
                   'Engine cache bytes cover registered acceleration caches and are not process RSS.',
                   'Animated is the focused Todo caret; static moves focus to the Add button.',
                   'Backend completion does not claim hardware scanout.'],
    }
    (root / 'summary.json').write_text(json.dumps(result, indent=2) + '\n')
    return result
