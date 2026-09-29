import collections
import json
import statistics
import time
from types import SimpleNamespace

import native
import report


def aggregate(batches):
    groups = collections.defaultdict(lambda: collections.defaultdict(list))
    for batch in batches:
        for mode, entries in batch.items():
            for entry in entries:
                summary = entry['summary']
                if summary['unit'] != 'milliseconds':
                    raise ValueError('unexpected summary unit')
                for action, result in summary['actions'].items():
                    for metric, timing in result['timing'].items():
                        if timing is not None:
                            groups[(entry['id'], action, metric)][mode].append(timing['median'])
    output = []
    for (entry, action, metric), modes in sorted(groups.items()):
        baseline = modes['off']
        for mode, values in sorted(modes.items()):
            if len(values) != len(baseline):
                raise ValueError('unpaired observation batches')
            delta = [v - b for v, b in zip(values, baseline)]
            ratios = [(v / b - 1) * 100 for v, b in zip(values, baseline) if b > 0]
            output.append({'entry': entry, 'action': action, 'metric': metric, 'mode': mode, 'batches': len(values), 'medians': values, 'median': statistics.median(values), 'paired_delta': statistics.median(delta), 'paired_percent': statistics.median(ratios) if ratios else None})
    return output


def run(args):
    if args.rounds < 1 or (args.runs is not None and args.runs < 1):
        raise ValueError('rounds and runs must be positive')
    if 'off' not in args.modes or len(set(args.modes)) != len(args.modes):
        raise ValueError('modes require off and must be unique')
    root = args.output.resolve()
    root.mkdir(parents=True, exist_ok=False)
    plan = json.loads(args.plan.read_text())
    for entry in plan['entries']:
        if entry.get('baseline'):
            entry['baseline'] = str((args.plan.parent / entry['baseline']).resolve())
        if args.runs is not None:
            entry['runs'] = args.runs
    source = root / 'plan.json'
    source.write_text(json.dumps(plan, indent=2))
    rules = root / 'rules.json'
    rules.write_text(json.dumps({'same': ['images'], 'vary': []}))
    binary = args.binary.resolve(strict=True)
    batches, observations = [], []
    for index in range(args.rounds):
        batch, paths = {}, {}
        offset = index % len(args.modes)
        order = args.modes[offset:] + args.modes[:offset]
        for mode in order:
            directory = root / f'{index}-{mode}'
            print(f'[{index + 1}/{args.rounds}] {mode}', flush=True)
            started = time.monotonic()
            capture = execute(args, (source, directory), mode, binary)
            target = directory / 'scenario' if mode == 'native' else directory
            paths[mode] = target
            manifest = json.loads((target / 'manifest.json').read_text())
            if any(e['error'] or e['summary'] is None or e['summary']['issues'] for e in manifest['entries']):
                raise ValueError(f'invalid experiment: {target}')
            batch[mode] = manifest['entries']
            observations.append({'round': index, 'mode': mode, 'harness_seconds': time.monotonic() - started, 'capture': capture})
        for mode, target in paths.items():
            if mode != 'off':
                for entry in plan['entries']:
                    name = entry['id'] + '.json'
                    native.invoke([str(binary), 'compare', str(paths['off'] / name), str(target / name), '--rules', str(rules)], root / f'{index}-{mode}-{entry["id"]}-compare.json')
        batches.append(batch)
        result = {'version': 1, 'unit': 'milliseconds', 'observations': observations, 'timings': aggregate(batches), 'accounting': 'Paired differences of per-batch action medians; no summing phase medians. Metrics retain spec summary semantics: cpu sums evaluation/publication/preparation wall intervals, not thread CPU time. Thread CPU observations live in observation.json. Off retains compiled probes but opens no session. Native includes CPU Counters and signposts. Harness seconds include reporting and native export and are not action timing. This is workload-specific probe perturbation, not an engine speedup.'}
        (root / 'overhead.json').write_text(json.dumps(result, indent=2))
    return {'report': str(root / 'overhead.json'), 'rounds': len(batches), 'modes': args.modes}


def execute(args, paths, mode, binary):
    source, directory = paths
    if mode == 'native':
        result = native.record(SimpleNamespace(plan=source, output=directory, binary=binary, seconds=args.seconds, limit=args.limit))
        if not result['complete'] or not result['successful']:
            raise ValueError(f'incomplete observation: {directory}')
        return result
    command = [str(binary), 'experiment' if mode == 'off' else 'trace', str(source), '--output', str(directory)]
    if mode != 'off':
        options = {'cpu': mode == 'cpu'}
        if args.limit is not None:
            options['limit'] = args.limit
        command += ['--options', json.dumps(options)]
    native.invoke(command, directory.with_suffix('.log'), args.seconds + 60)
    if mode == 'off':
        return None
    summary = report.write(directory / 'observation.json', directory / 'analysis')
    if not summary['complete'] or not summary['successful']:
        raise ValueError(f'incomplete observation: {directory}')
    return {key: summary[key] for key in ('complete', 'successful', 'dropped', 'open')}
