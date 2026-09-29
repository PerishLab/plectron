import hashlib
import json
import time

import application
import frames
import ipc

MODES = ('zero', 'low', 'high', 'wait')
QUIET = 0.15
INTERVAL = 0.005


def query(app, command):
    return app.request({'type': 'query', 'command': command})['reply']


def record(reply):
    status = reply['status']
    if status['type'] != 'record':
        raise RuntimeError(f'completion record unavailable: {reply}')
    result = status['record']
    if result['state']['type'] not in ('pending', 'complete'):
        raise RuntimeError(f'completion unsuccessful: {result}')
    if result['state']['type'] == 'complete' and 'Ok' not in result['state']['result']:
        raise RuntimeError(f'completion failed: {result}')
    return result


def terminal(app, identity, mode):
    start = time.perf_counter_ns()
    count = 0
    deadline = time.monotonic() + 10
    while time.monotonic() < deadline:
        command = {'type': 'wait', 'record': identity, 'timeout': 2000} if mode == 'wait' else {'type': 'read', 'record': identity}
        reply = query(app, command)
        current = record(reply)
        count += 1
        if current['state']['type'] == 'complete':
            return current, count, time.perf_counter_ns() - start
        if mode == 'zero':
            raise RuntimeError('execution did not complete during the query-free interval')
        if mode == 'low':
            time.sleep(INTERVAL)
    raise RuntimeError('passive completion timed out')


def durations(task, frame):
    for item in (task, frame):
        if item['produced'] is None or not item['admitted'] <= item['produced'] <= item['detected']:
            raise RuntimeError(f'invalid host timestamp topology: {item}')
    return {
        'owner_completion': frame['detected'] - task['admitted'],
        'task_production': task['produced'] - task['admitted'],
        'task_publication': task['detected'] - task['produced'],
        'frame_production': frame['produced'] - frame['admitted'],
        'frame_publication': frame['detected'] - frame['produced'],
    }


def action(app, previous, source, mode):
    start = time.perf_counter_ns()
    accepted = app.request({'type': 'evaluate', 'unit': 1, 'source': source})
    admission = time.perf_counter_ns() - start
    if mode == 'zero':
        time.sleep(QUIET)
    task, queries, inquiry = terminal(app, accepted['record'], mode)
    latest = record(query(app, {'type': 'latest', 'unit': 1}))
    if latest['target']['frame'] == previous['frame']:
        raise RuntimeError('new task did not publish a new frame')
    frame, count, elapsed = terminal(app, latest['id'], mode)
    wall = time.perf_counter_ns() - start
    snapshot = frames.page(app)
    if not frames.settled(snapshot) or snapshot['frame'] != frame['target']['frame']:
        raise RuntimeError('published frame differs from explicit page inspection')
    counts = {key: value - previous['computation'][key] for key, value in snapshot['computation'].items()}
    return snapshot, {
        'task': task, 'frame': frame, 'queries': queries + count + 1,
        'wall': wall, 'admission': admission, 'inquiry': inquiry + elapsed,
        'computation': counts, 'signature': frames.signature(snapshot), **durations(task, frame),
    }


def capture(args, directory, case, mode):
    app = application.Application(args.binary, directory, 'metal')
    samples = []
    try:
        app.ready()
        root = json.dumps(str(args.apps.resolve()))
        app.evaluate(f'const server = platform.serve({root}); server.scheme("app://passive/"); const win = platform.window({{title:"Passive frame receipt",width:720,height:640}}); win.load("app://passive/{case}/index.html"); 1;')
        previous, _, _ = frames.wait(app)
        size = next(item for item in json.loads((args.apps / 'catalog.json').read_text()) if item['id'] == case)['sizes']['small']
        previous, _ = frames.action(app, previous, f'scenario.seed({json.dumps(list(range(size)))}); 1;')
        for index in range(args.runs + frames.WARMUP):
            previous, sample = action(app, previous, f'scenario.run("{frames.CASES[case]}", []); 1;', mode)
            if index >= frames.WARMUP:
                samples.append(sample)
        (directory / 'snapshot.json').write_text(json.dumps(previous))
    finally:
        app.close()
    metrics = ('owner_completion', 'task_production', 'task_publication', 'frame_production', 'frame_publication', 'wall', 'queries')
    result = {'case': case, 'mode': mode, 'count': size, 'samples': samples,
              'timing': {key: ipc.distribution([s[key] for s in samples]) for key in metrics}}
    (directory / 'result.json').write_text(json.dumps(result, indent=2))
    return result


def run(args):
    if args.rounds < 1 or args.runs < 1:
        raise ValueError('rounds and runs must be positive')
    root = args.output.resolve()
    root.mkdir(parents=True, exist_ok=False)
    variants = [(case, mode) for case in frames.CASES for mode in MODES]
    rows = []
    for index in range(args.rounds):
        offset = index % len(variants)
        batch = {}
        for case, mode in variants[offset:] + variants[:offset]:
            print(f'[{index + 1}/{args.rounds}] {case} {mode}', flush=True)
            result = capture(args, root / f'{index}-{case}-{mode}', case, mode)
            batch[(case, mode)] = result
            rows.append({'round': index, 'case': case, 'mode': mode, 'timing': result['timing']})
        for case in frames.CASES:
            for mode in MODES:
                frames.verify(batch[(case, 'zero')], batch[(case, mode)])
    result = {
        'binary': str(args.binary.resolve()), 'sha256': hashlib.sha256(args.binary.read_bytes()).hexdigest(),
        'rounds': args.rounds, 'runs': args.runs, 'warmup': frames.WARMUP,
        'unit': 'nanoseconds', 'zero_interval_seconds': QUIET, 'low_interval_seconds': INTERVAL,
        'endpoint': 'owner-published successful presentation receipt; host timestamps share publication-store epoch',
        'caveats': ['Frame production includes render, drawable acquisition and presentation-copy completion; not hardware GPU duration or scanout.',
                    'Task publication includes owner processing after receipt; component distributions overlap and must not be summed.',
                    'Zero mode performs no requests after submission until the quiet interval ends; setup and post-completion inspection are explicit.',
                    'Client wall includes deliberate quiet or polling delay; semantic signatures are not pixel comparison.'],
        'rows': rows,
    }
    (root / 'summary.json').write_text(json.dumps(result, indent=2))
    return result
