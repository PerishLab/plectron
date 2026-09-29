import collections
import heapq
import json
import pathlib
import xml.etree.ElementTree as ET


class Table:
    def __init__(self, path):
        self.root = ET.parse(path).getroot()
        self.ids = {e.attrib['id']: e for e in self.root.iter() if 'id' in e.attrib}

    def resolve(self, element):
        while element is not None and 'ref' in element.attrib:
            element = self.ids[element.attrib['ref']]
        return element

    def text(self, element):
        element = self.resolve(element)
        return None if element is None else element.text

    def child(self, element, name):
        element = self.resolve(element)
        return None if element is None else element.find(name)

    def frames(self, element):
        element = self.resolve(element)
        if element is None:
            return []
        if element.tag == 'frame':
            return [element.attrib.get('name', '?')]
        return [name for child in element for name in self.frames(child)]

    def rows(self):
        for node in self.root.findall('node'):
            keys = [c.findtext('mnemonic') for c in node.find('schema').findall('col')]
            for row in node.findall('row'):
                yield dict(zip(keys, row))


def intervals(source, document):
    table = Table(source)
    records = {r['id']: r for r in document['records']}
    starts = {}
    ends = {}
    for row in table.rows():
        if table.text(row['subsystem']) != 'io.plectron.observe':
            continue
        if int(table.text(table.child(row['process'], 'pid'))) != document['process']:
            continue
        identity = int(table.text(row['identifier']))
        if identity not in records:
            continue
        thread = int(table.text(table.child(row['thread'], 'tid')))
        if records[identity]['native'] != thread:
            raise ValueError('native thread identity differs from observation record')
        time = int(table.text(row['time']))
        kind = table.text(row['event-type'])
        if kind == 'Begin':
            metadata = table.resolve(row['message'])
            numbers = [int(table.text(e)) for e in metadata if e.tag == 'uint64']
            if numbers != [document['session'], records[identity]['parent'] or 0]:
                raise ValueError('native session or parent identity mismatch')
            starts[identity] = (time, thread)
        elif kind == 'End':
            ends[identity] = time
    matched = [{'id': i, 'start': start, 'end': ends[i], 'thread': thread} for i, (start, thread) in starts.items() if i in ends and ends[i] >= start]
    eligible = {r['id'] for r in records.values() if r['signpost']}
    missing = sorted(eligible - {r['id'] for r in matched})
    return matched, missing


def samples(source, process):
    table = Table(source)
    result = []
    for row in table.rows():
        if int(table.text(table.child(row['process'], 'pid'))) != process:
            continue
        result.append({'time': int(table.text(row['timestamp'])), 'thread': int(table.text(table.child(row['thread'], 'tid'))), 'remark': table.text(row['remark']), 'stack': table.frames(row['backtrace'])})
    return result


def assign(windows, samples):
    threads = collections.defaultdict(list)
    for window in windows:
        threads[window['thread']].append(window)
    indexed = collections.defaultdict(list)
    for sample in samples:
        indexed[sample['thread']].append(sample)
    assigned = []
    for thread, values in indexed.items():
        spans = iter(sorted(threads[thread], key=lambda s: s['start']))
        upcoming = next(spans, None)
        active = []
        for sample in sorted(values, key=lambda s: s['time']):
            time = sample['time']
            while upcoming is not None and upcoming['start'] <= time:
                heapq.heappush(active, (upcoming['end'] - upcoming['start'], -upcoming['start'], upcoming['id'], upcoming['end']))
                upcoming = next(spans, None)
            while active and active[0][3] <= time:
                heapq.heappop(active)
            assigned.append({**sample, 'span': active[0][2] if active else None})
    return assigned


def correlate(root):
    root = pathlib.Path(root)
    document = json.loads((root / 'scenario/observation.json').read_text())
    windows, missing = intervals(root / 'os-signpost.xml', document)
    assigned = assign(windows, samples(root / 'CountingModeSamples.xml', document['process']))
    records = {r['id']: r for r in document['records']}
    groups = collections.defaultdict(list)
    for sample in assigned:
        entry = run = action = node = None
        identity = sample['span']
        seen = set()
        while identity in records and identity not in seen:
            seen.add(identity)
            record = records[identity]
            if record['name'].startswith('entry.'):
                entry = record['name']
            if record['name'].startswith('run.') or record['name'] == 'warmup':
                run = record['name']
            if record['name'].startswith('action.'):
                action = record['name']
            if node is None and record['kind']['type'] != 'graph':
                node = record['name']
            identity = record['parent']
        groups[(entry, run, action, node)].append(sample)
    summary = [{'entry': key[0], 'run': key[1], 'action': key[2], 'node': key[3], 'samples': len(rows), 'remarks': dict(collections.Counter(r['remark'] for r in rows)), 'leaves': collections.Counter(r['stack'][0] for r in rows if r['stack']).most_common(8)} for key, rows in groups.items()]
    result = {'version': 1, 'matched': len(windows), 'missing': missing, 'samples': len(assigned), 'unassigned': sum(r['span'] is None for r in assigned), 'groups': summary, 'accounting': 'Sample counts and native tool classifications, not exact per-node cycles, cache-miss rates or exclusive CPU time. Native collection loss is separate from observation capacity loss.'}
    (root / 'analysis/native.json').write_text(json.dumps(result, indent=2))
    (root / 'analysis/samples.json').write_text(json.dumps(assigned))
    return result
