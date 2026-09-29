import hashlib
import json
import resource
import statistics
import time

import application
import bench
import report

WARMUP = 5
CASES = {
    'compute': ('count = count + 1; 1;', 0),
    'single': ('console.log(1); count = count + 1; 1;', 1),
    'burst': ('for (const value of [1,1,1,1,1,1,1,1]) { console.log(value); } count = count + 1; 1;', 8),
    'promise': ('Promise.resolve(1).then(value => { count = count + value; console.log(value); }); 1;', 1),
}


def distribution(values):
    values = sorted(values)
    return {'median': statistics.median(values), 'p95': values[(len(values) * 95 + 99) // 100 - 1], 'maximum': values[-1]}


def capture(binary, directory, runs, traced):
    before = resource.getrusage(resource.RUSAGE_CHILDREN)
    app = application.Application(binary, directory)
    records = {}
    diagnostics = None
    try:
        app.ready()
        identity = app.observation({'type': 'start', 'options': {'cpu': True}})['session'] if traced else None
        start = time.perf_counter_ns()
        time.sleep(0.5)
        idle = app.request({'type': 'status'})
        duration = time.perf_counter_ns() - start
        if idle['mode'] != 'running':
            raise RuntimeError(f'idle application failed: {idle}')
        for name, (source, _) in CASES.items():
            samples = []
            for index in range(runs + WARMUP):
                sample = app.evaluate(source)
                if sample['result'] != '1':
                    raise RuntimeError(f'unexpected script result: {sample}')
                if index >= WARMUP:
                    samples.append(sample)
            records[name] = {'samples': samples, 'timing': {key: distribution([v[key] for v in samples]) for key in ('wall', 'admission', 'polls')}}
        expected = (runs + WARMUP) * len(CASES)
        if app.evaluate('count;')['result'] != str(expected):
            raise RuntimeError('script state did not match completed actions')
        if traced:
            diagnostics = report.write(app.export(identity), directory / 'analysis')
            if not diagnostics['complete'] or not diagnostics['successful']:
                raise RuntimeError('incomplete application observation')
    finally:
        app.close()
    after = resource.getrusage(resource.RUSAGE_CHILDREN)
    expected = (runs + WARMUP) * sum(count for _, count in CASES.values())
    if (directory / 'stdout.log').read_text().splitlines() != ['1'] * expected:
        raise RuntimeError('platform log output mismatch')
    result = {'actions': records, 'observation': diagnostics, 'idle_wall': duration,
              'children_cpu': {'user': after.ru_utime - before.ru_utime, 'system': after.ru_stime - before.ru_stime},
              'cpu_endpoint': 'Whole application lifecycle plus export helper in traced runs; includes startup and shutdown, not per-action CPU'}
    (directory / 'result.json').write_text(json.dumps(result, indent=2))
    return result


def aggregate(batches):
    output = []
    for traced in (False, True):
        paired = []
        for batch in batches:
            modes = {}
            for mode, result in batch[traced].items():
                modes['off' if mode == 'baseline' else mode] = [{'id': case, 'summary': {'unit': 'milliseconds', 'actions': {'execute': {'timing': {
                    metric: {'median': data['timing'][metric]['median'] / 1_000_000} for metric in ('wall', 'admission')
                }}}}} for case, data in result['actions'].items()]
            paired.append(modes)
        output.extend({**row, 'traced': traced} for row in bench.aggregate(paired))
    return output


def run(args):
    if args.rounds < 1 or args.runs < 1:
        raise ValueError('rounds and runs must be positive')
    root = args.output.resolve()
    root.mkdir(parents=True, exist_ok=False)
    binaries = {name: getattr(args, name).resolve(strict=True) for name in ('baseline', 'internal', 'candidate')}
    variants = [(traced, mode) for traced in (False, True) for mode in binaries]
    batches = []
    for round in range(args.rounds):
        batch = {False: {}, True: {}}
        offset = round % len(variants)
        for traced, mode in variants[offset:] + variants[:offset]:
            print(f'[{round + 1}/{args.rounds}] {mode} traced={traced}', flush=True)
            target = root / f'{round}-{mode}-{int(traced)}'
            batch[traced][mode] = capture(binaries[mode], target, args.runs, traced)
        batches.append(batch)
    result = {'unit': 'milliseconds', 'rounds': args.rounds, 'runs': args.runs, 'warmup': WARMUP,
              'binaries': {name: {'path': str(path), 'sha256': hashlib.sha256(path.read_bytes()).hexdigest()} for name, path in binaries.items()},
              'endpoint': 'Client socket connect through eval acknowledgement and first control reply confirming complete; includes repeated fresh connections and JSON decoding, no CLI process per request',
              'limits': ['Warm headless application, no render claim', 'Client polls status without added sleep; poll counts recorded', 'Traced captures include warmups, final state check and idle phase', 'Action wall excludes startup, observation export and shutdown'],
              'comparison': aggregate(batches)}
    (root / 'summary.json').write_text(json.dumps(result, indent=2))
    return result
