import hashlib
import json
import statistics
import time

import application
import ipc
import report

CASES = {'cards': 'width', 'layers': 'fade'}
WARMUP = 4


def page(app):
    snapshot = app.request({'type': 'page', 'unit': 1})['snapshot']
    if snapshot['fault'] or snapshot['output'] or snapshot['state'] == 'failed':
        raise RuntimeError(f'page failed: {snapshot["fault"]} {snapshot["output"]}')
    if snapshot['execution'] and snapshot['execution']['fault']:
        raise RuntimeError(snapshot['execution']['fault'])
    return snapshot


def settled(snapshot):
    return (snapshot['state'] == 'ready' and snapshot['scene'] is not None
            and snapshot['displayed'] == snapshot['scene'] and snapshot['presented'] == snapshot['revision']
            and snapshot['issued'] is None and snapshot['pending'] is None)


def signature(snapshot):
    shape = {key: snapshot[key] for key in ('nodes', 'visible', 'visual', 'shown', 'items')}
    return hashlib.sha256(json.dumps(shape, sort_keys=True, separators=(',', ':')).encode()).hexdigest()


def wait(app, previous=None, job=None, interval=0):
    deadline = time.monotonic() + 15
    queries = 0
    query_time = 0
    while time.monotonic() < deadline:
        start = time.perf_counter_ns()
        snapshot = page(app)
        query_time += time.perf_counter_ns() - start
        queries += 1
        ready = settled(snapshot)
        if job is not None:
            result = next(j for j in snapshot['execution']['jobs'] if j['id'] == job)
            if result['phase'] == 'complete':
                if 'Ok' not in result['result']:
                    raise RuntimeError(f'content job failed: {result}')
            else:
                ready = False
        if previous is not None:
            ready = ready and snapshot['revision'] > previous['revision'] and snapshot['frame'] != previous['frame']
        if ready:
            return snapshot, queries, query_time
        if interval:
            time.sleep(interval)
    raise RuntimeError('new frame receipt timed out')


def action(app, previous, source, interval=0):
    start = time.perf_counter_ns()
    accepted = app.request({'type': 'evaluate', 'unit': 1, 'source': source})
    admission = time.perf_counter_ns() - start
    snapshot, queries, inspection = wait(app, previous, accepted['job'], interval)
    wall = time.perf_counter_ns() - start
    computation = {key: value - previous['computation'][key] for key, value in snapshot['computation'].items()}
    return snapshot, {'wall': wall, 'admission': admission, 'inspection': inspection, 'queries': queries,
                      'computation': computation, 'signature': signature(snapshot),
                      'frame': snapshot['frame'], 'revision': snapshot['revision'], 'displayed': snapshot['displayed']}


def capture(args, binary, directory, case, traced):
    app = application.Application(binary, directory, 'metal')
    samples = []
    try:
        app.ready()
        root = json.dumps(str(args.apps.resolve()))
        source = f'const server = platform.serve({root}); server.scheme("app://frames/"); const win = platform.window({{title: "Frame receipt probe", width: 720, height: 640}}); win.load("app://frames/{case}/index.html"); 1;'
        if app.evaluate(source)['result'] != '1':
            raise RuntimeError('launcher failed')
        previous, _, _ = wait(app)
        expected = next(item for item in json.loads((args.apps / 'catalog.json').read_text()) if item['id'] == case)['sizes']['small']
        ids = json.dumps(list(range(expected)))
        previous, _ = action(app, previous, f'scenario.seed({ids}); 1;')
        identity = app.observation({'type': 'start', 'options': {'cpu': True}})['session'] if traced else None
        for index in range(args.runs + WARMUP):
            previous, sample = action(app, previous, f'scenario.run("{CASES[case]}", []); 1;', args.interval / 1000)
            if index >= WARMUP:
                samples.append(sample)
        (directory / 'snapshot.json').write_text(json.dumps(previous))
        if traced:
            summary = report.write(app.export(identity), directory / 'analysis')
            if not summary['complete'] or not summary['successful']:
                raise RuntimeError('incomplete frame observation')
        result = {'case': case, 'action': CASES[case], 'count': expected, 'traced': traced, 'samples': samples,
                  'timing': {key: ipc.distribution([s[key] for s in samples]) for key in ('wall', 'admission', 'inspection', 'queries')},
                  'computation': {key: ipc.distribution([s['computation'][key] for s in samples]) for key in previous['computation']}}
    finally:
        app.close()
    (directory / 'result.json').write_text(json.dumps(result, indent=2))
    return result


def compare(batches):
    rows = []
    for case in CASES:
        for traced in (False, True):
            for metric in ('wall', 'admission', 'inspection'):
                baseline = [b[(case, traced, 'baseline')]['timing'][metric]['median'] / 1_000_000 for b in batches]
                candidate = [b[(case, traced, 'candidate')]['timing'][metric]['median'] / 1_000_000 for b in batches]
                rows.append({'case': case, 'traced': traced, 'metric': metric, 'baseline': baseline, 'candidate': candidate,
                             'paired_percent': statistics.median((c / b - 1) * 100 for b, c in zip(baseline, candidate))})
    return rows


def verify(left, right):
    if [s['signature'] for s in left['samples']] != [s['signature'] for s in right['samples']]:
        raise RuntimeError('frame semantic signatures differ')
    counts = ('layouts', 'paints', 'checks', 'changes', 'queries')
    if [[s['computation'][k] for k in counts] for s in left['samples']] != [[s['computation'][k] for k in counts] for s in right['samples']]:
        raise RuntimeError('frame computation counts differ')


def run(args):
    if args.runs < 1 or args.rounds < 1 or args.interval < 0:
        raise ValueError('runs and rounds must be positive; interval must be nonnegative')
    root = args.output.resolve()
    root.mkdir(parents=True, exist_ok=False)
    binaries = {name: getattr(args, name).resolve(strict=True) for name in ('baseline', 'candidate')}
    variants = [(case, traced, mode) for case in CASES for traced in (False, True) for mode in binaries]
    batches = []
    for index in range(args.rounds):
        offset = index % len(variants)
        batch = {}
        for case, traced, mode in variants[offset:] + variants[:offset]:
            print(f'[{index + 1}/{args.rounds}] {case} {mode} traced={traced}', flush=True)
            target = root / f'{index}-{case}-{mode}-{int(traced)}'
            batch[(case, traced, mode)] = capture(args, binaries[mode], target, case, traced)
        for case in CASES:
            reference = batch[(case, False, 'baseline')]
            for traced in (False, True):
                for mode in binaries:
                    verify(reference, batch[(case, traced, mode)])
        batches.append(batch)
    result = {'unit': 'milliseconds', 'rounds': args.rounds, 'runs': args.runs, 'warmup': WARMUP, 'client_interval_ms': args.interval,
              'binaries': {name: {'path': str(path), 'sha256': hashlib.sha256(path.read_bytes()).hexdigest()} for name, path in binaries.items()},
              'endpoint': 'Client eval connection through page response confirming completed content job and changed frame receipt: presented revision matches desired revision, displayed scene matches desired scene, no pending or issued frame',
              'limits': ['Metal completion and drawable-copy receipt is not physical scanout', 'Full page inspection and JSON processing are included and timed separately; inspection can wake the main loop', 'Small catalog scenes, six records each, 720x640 logical viewport', 'No per-action hardware GPU duration exposed by this protocol', 'Semantic scene/geometry hashes and work counters compared, not screenshots or pixel hashes', 'Content/render/Metal algorithms are unchanged by the compared commits'],
              'comparison': compare(batches)}
    (root / 'summary.json').write_text(json.dumps(result, indent=2))
    return result
