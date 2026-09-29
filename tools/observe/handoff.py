import collections
import json
import statistics

import report


def measured(document):
    by_id = {r['id']: r for r in document['records']}
    def root(record):
        seen = set()
        while record['parent'] is not None:
            if record['id'] in seen:
                raise ValueError('cyclic observation ancestry')
            seen.add(record['id'])
            record = by_id[record['parent']]
        return record['name']
    records = [r for r in document['records'] if root(r) != 'warmup']
    return {**document, 'records': records}


def analyze(document):
    if document['unit'] != 'nanoseconds':
        raise ValueError('unexpected handoff unit')
    groups = collections.defaultdict(dict)
    diagnostics = []
    for batch in document['runs']:
        run = batch['run']
        mode = run.get('mode') or ('both' if run['notified'] else 'poll')
        key = (run['case'], run['traced'], mode)
        if batch['round'] in groups[key]:
            raise ValueError('duplicate handoff batch')
        samples = run['samples']
        result = {name: statistics.median(s[name] for s in samples) for name in ('wall', 'waits')}
        result['p95'] = sorted(s['wall'] for s in samples)[(len(samples) * 95 + 99) // 100 - 1]
        if run['report'] is not None:
            summary, _ = report.analyze(measured(run['report']))
            if not summary['complete'] or not summary['successful']:
                raise ValueError('incomplete handoff capture')
            spans = summary['groups']
            result['host_cpu'] = spans['probe.action']['cpu']['median']
            result['worker_cpu'] = spans['probe.execute']['cpu']['median']
            result['handoff'] = spans['platform.request']['handoff']['median']
            diagnostics.append({'case': run['case'], 'round': batch['round'], 'mode': mode, 'summary': summary})
        groups[key][batch['round']] = result
    rows = []
    for (case, traced, mode), candidate in sorted(groups.items()):
        if mode == 'poll':
            continue
        batches = groups[(case, traced, 'poll')]
        if set(candidate) != set(batches):
            raise ValueError('unpaired handoff batches')
        for metric in next(iter(batches.values())):
            baseline = [batches[r][metric] for r in sorted(batches)]
            values = [candidate[r][metric] for r in sorted(batches)]
            ratios = [(v / b - 1) * 100 for b, v in zip(baseline, values) if b]
            rows.append({'case': case, 'traced': traced, 'mode': mode, 'metric': metric, 'poll': baseline, 'notify': values,
                         'paired_delta': statistics.median(v - b for b, v in zip(baseline, values)),
                         'paired_percent': statistics.median(ratios) if ratios else None})
    return {'unit': 'nanoseconds except waits/count and percentages', 'endpoint': document['endpoint'],
            'build': document['build'], 'samples': document['samples'], 'rounds': document['rounds'],
            'limits': document['limits'], 'comparison': rows, 'diagnostics': diagnostics, 'idle': document['idle']}


def write(source, output):
    result = analyze(json.loads(source.read_text()))
    output.mkdir(parents=True, exist_ok=False)
    (output / 'summary.json').write_text(json.dumps(result, indent=2))
    return {key: value for key, value in result.items() if key not in ('diagnostics', 'idle')}
