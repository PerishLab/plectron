import collections
import json
import pathlib


def union(intervals):
    end = None
    total = 0
    for start, stop in sorted(intervals):
        start = start if end is None else max(start, end)
        total += max(0, stop - start)
        end = stop if end is None else max(end, stop)
    return total


def analyze(document):
    if document['version'] != 1 or document['unit'] != 'nanoseconds':
        raise ValueError('unsupported observation schema')
    records = document['records']
    by_id = {r['id']: r for r in records}
    if len(by_id) != len(records):
        raise ValueError('duplicate span identity')
    children = collections.defaultdict(list)
    issues = []
    for record in records:
        if record['end'] < record['start']:
            raise ValueError('inverted interval')
        if record['kind']['type'] == 'receive' and record['kind']['captured'] > record['start']:
            raise ValueError('inverted handoff')
        parent = record['parent']
        if parent is not None:
            if parent not in by_id:
                issues.append(f'missing parent {parent}')
        enclosing = record.get('enclosing', parent)
        if enclosing is not None:
            children[enclosing].append(record)
            if enclosing not in by_id:
                issues.append(f'missing enclosing interval {enclosing}')
    complete = document['stopped'] and not document['open'] and not document['dropped'] and not issues
    roots = [r for r in records if r['parent'] is None]
    successful = all(r['outcome'] == 'success' for r in roots) if roots else None
    groups = collections.defaultdict(list)
    chrome = []
    for record in records:
        start, end = record['start'], record['end']
        nested = [(max(start, child['start']), min(end, child['end'])) for child in children[record['id']] if child['thread'] == record['thread'] and child['end'] > start and child['start'] < end]
        handoff = start - record['kind']['captured'] if record['kind']['type'] == 'receive' else None
        groups[record['name']].append({'wall': end - start, 'self': end - start - union(nested) if complete else None, 'cpu': record['cpu'], 'handoff': handoff, 'outcome': record['outcome']})
        if handoff is not None:
            flow = {'name': record['name'], 'cat': 'handoff', 'id': str(record['id']), 'pid': document['process']}
            chrome.append({**flow, 'ph': 's', 'ts': record['kind']['captured'] / 1000, 'tid': record['kind']['thread']})
            chrome.append({**flow, 'ph': 'f', 'ts': start / 1000, 'tid': record['thread'], 'bp': 'e'})
        chrome.append({'name': record['name'], 'cat': record['kind']['type'], 'ph': 'X', 'ts': start / 1000, 'dur': (end - start) / 1000, 'pid': document['process'], 'tid': record['thread'], 'args': {'id': record['id'], 'parent': record['parent'], 'enclosing': record.get('enclosing'), 'cpu_ns': record['cpu'], 'outcome': record['outcome']}})
    def distribution(values):
        values = sorted(v for v in values if v is not None)
        if not values:
            return None
        count = len(values)
        return {'samples': count, 'total': sum(values), 'median': (values[(count - 1) // 2] + values[count // 2]) / 2, 'maximum': values[-1]}
    summary = {name: {**{key: distribution([v[key] for v in values]) for key in ('wall', 'self', 'cpu', 'handoff')}, 'outcomes': dict(collections.Counter(v['outcome'] for v in values))} for name, values in sorted(groups.items())}
    return {'version': 1, 'unit': 'nanoseconds', 'nesting': 'explicit' if all('enclosing' in r for r in records) else 'legacy or mixed: missing enclosing fields fall back to parent', 'complete': complete, 'successful': successful, 'dropped': document['dropped'], 'open': document['open'], 'issues': issues, 'groups': summary, 'accounting': 'Wall and CPU inclusive; self wall excludes the union of recorded same-thread execution children, not causal descendants. Nesting is session-local; other sessions and unobserved work cannot be subtracted. Group totals overlap across nesting; do not sum them. Handoff is capture-to-handler latency, including intervening sender work, blocking, queueing and scheduling; it is not CPU or pure queue time. It may overlap enclosing work. Completeness covers recorded intervals, not undelivered envelopes or all work. Instrumentation overhead is included. Dropped counts rejected scope entries; suppressed descendants are uncounted.'}, {'traceEvents': chrome, 'displayTimeUnit': 'ms'}


def write(source, output):
    summary, chrome = analyze(json.loads(pathlib.Path(source).read_text()))
    target = pathlib.Path(output)
    target.mkdir(parents=True, exist_ok=True)
    for name, value in [('summary.json', summary), ('timeline.json', chrome)]:
        with (target / name).open('x') as stream:
            json.dump(value, stream)
    return summary
