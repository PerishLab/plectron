import unittest
import report
import bench
import handoff
import ipc
import frames
import passive
import activity
import feel


class Frames(unittest.TestCase):
    def test_receipt(self):
        snapshot = {'state': 'ready', 'scene': 2, 'displayed': 2, 'revision': 4, 'presented': 4,
                    'issued': None, 'pending': None}
        self.assertTrue(frames.settled(snapshot))
        for key, value in [('scene', None), ('displayed', 1), ('presented', 3), ('issued', 9), ('pending', 10)]:
            self.assertFalse(frames.settled({**snapshot, key: value}))

    def test_equivalence(self):
        counts = dict.fromkeys(('layouts', 'paints', 'checks', 'changes', 'queries'), 0)
        left = {'samples': [{'signature': 'same', 'computation': counts}]}
        frames.verify(left, left)
        with self.assertRaises(RuntimeError):
            frames.verify(left, {'samples': [{'signature': 'different', 'computation': counts}]})
        with self.assertRaises(RuntimeError):
            frames.verify(left, {'samples': [{'signature': 'same', 'computation': {**counts, 'layouts': 1}}]})


class Application(unittest.TestCase):
    def test_endpoints(self):
        def result(wall, admission):
            return {'actions': {'probe': {'timing': {'wall': {'median': wall}, 'admission': {'median': admission}}}}}
        modes = {'baseline': result(20_000_000, 10_000_000), 'internal': result(12_000_000, 10_000_000), 'candidate': result(1_000_000, 400_000)}
        rows = ipc.aggregate([{False: modes, True: modes}])
        actual = next(r for r in rows if r['mode'] == 'candidate' and r['metric'] == 'wall' and not r['traced'])
        self.assertEqual(actual['median'], 1)
        self.assertEqual(actual['paired_percent'], -95)
        admission = next(r for r in rows if r['mode'] == 'candidate' and r['metric'] == 'admission' and r['traced'])
        self.assertEqual(admission['median'], 0.4)

    def test_tail(self):
        self.assertEqual(ipc.distribution(list(range(1, 21))), {'median': 10.5, 'p95': 19, 'maximum': 20})


class Activity(unittest.TestCase):
    def test_cputime(self):
        self.assertEqual(activity.cputime('1:02.50'), 62_500_000_000)
        self.assertEqual(activity.cputime('2-03:04:05.25'), 183_845_250_000_000)


class Feel(unittest.TestCase):
    def test_evaluate(self):
        class App:
            def request(self, command):
                return {'type': 'evaluation', 'display': command['source']}
        self.assertEqual(feel.evaluate(App(), 'value;'), 'value;')

    def test_summary(self):
        def frame(identity, admitted, produced, visible, drops=0):
            return {
                'id': identity,
                'target': {'type': 'frame', 'unit': 1, 'frame': identity, 'scene': 1},
                'state': {'type': 'complete', 'result': {'Ok': ''}},
                'admitted': admitted,
                'produced': produced,
                'visible': visible,
                'detected': produced + 2,
                'drops': drops,
            }
        result = feel.summarize([
            frame(1, 0, 10, 20),
            frame(2, 30, 40, 55, 1),
            frame(3, 200_000_000, 200_000_010, None),
        ], [9])
        self.assertEqual(result['frames'], 3)
        self.assertEqual(result['visible'], 2)
        self.assertEqual(result['missing_visibility'], 1)
        self.assertEqual(result['drops'], {'total': 1, 'maximum': 1})
        self.assertEqual(result['timing']['admission_to_visibility']['median'], 22.5)
        self.assertEqual(result['timing']['active_visibility_cadence']['median'], 35)
        self.assertEqual(result['visible_span'], 35)
        self.assertEqual(result['evicted'], [9])

    def test_assessment(self):
        summary = {'frames': 10, 'evicted': [], 'missing_visibility': 0, 'drops': {'total': 0},
                   'visible_span': 900_000_000}
        final = {'settled': True}
        observation = {'complete': True, 'successful': True}
        self.assertTrue(feel.assess(summary, final, observation, 'forward', 1)['valid'])
        interrupted = {**summary, 'missing_visibility': 1, 'drops': {'total': 1},
                       'visible_span': 100_000_000}
        result = feel.assess(interrupted, {**final, 'settled': False}, observation, 'forward', 1)
        self.assertFalse(result['valid'])
        self.assertEqual(len(result['reasons']), 3)
        self.assertEqual(result['findings'], ['presentation reported dropped frames'])
        empty = {**summary, 'frames': 0}
        self.assertFalse(feel.assess(empty, final, observation, 'manual', 1)['valid'])

    def test_empty(self):
        result = feel.summarize([], [])
        self.assertEqual(result['frames'], 0)
        self.assertIsNone(result['timing']['admission_to_visibility'])

class Handoff(unittest.TestCase):
    def test_warmup(self):
        warm = record(1, None, 0, 10)
        warm['name'] = 'warmup'
        live = record(3, None, 20, 30)
        source = document([warm, record(2, 1, 1, 9), live])
        self.assertEqual(handoff.measured(source)['records'], [live])

    def test_comparison(self):
        def batch(round, notified, value):
            return {'round': round, 'run': {'case': 'probe', 'traced': False, 'notified': notified,
                    'samples': [{'wall': value, 'waits': 1}], 'report': None}}
        source = {'unit': 'nanoseconds', 'runs': [batch(0, False, 10), batch(0, True, 1),
                  batch(1, False, 20), batch(1, True, 4)], 'endpoint': 'probe', 'build': 'test',
                  'samples': 1, 'rounds': 2, 'limits': [], 'idle': []}
        result = handoff.analyze(source)
        wall = next(row for row in result['comparison'] if row['metric'] == 'wall')
        self.assertEqual(wall['paired_delta'], -12.5)
        self.assertEqual(wall['paired_percent'], -85)
        source['runs'].pop()
        with self.assertRaises(ValueError):
            handoff.analyze(source)


class Overhead(unittest.TestCase):
    def test_absent(self):
        entry = [{'id': 'example', 'summary': {'unit': 'milliseconds', 'actions': {'edit': {'timing': {'gpu': None}}}}}]
        self.assertEqual(bench.aggregate([{'off': entry, 'wall': entry}]), [])

    def test_pairs(self):
        def entry(value):
            return [{'id': 'example', 'summary': {'unit': 'milliseconds', 'actions': {'edit': {'timing': {'total': {'median': value}}}}}}]
        values = bench.aggregate([{'off': entry(1), 'wall': entry(3)}, {'off': entry(10), 'wall': entry(11)}])
        wall = next(v for v in values if v['mode'] == 'wall')
        self.assertEqual(wall['paired_delta'], 1.5)
        self.assertAlmostEqual(wall['paired_percent'], 105)
        self.assertEqual(wall['medians'], [3, 11])

    def test_unpaired(self):
        entry = [{'id': 'example', 'summary': {'unit': 'milliseconds', 'actions': {'edit': {'timing': {'total': {'median': 1}}}}}}]
        with self.assertRaises(ValueError):
            bench.aggregate([{'off': entry, 'wall': entry}, {'off': entry}])


def record(identity, parent, start, end, thread=1):
    return {'id': identity, 'parent': parent, 'start': start, 'end': end, 'thread': thread, 'cpu': None, 'outcome': 'success', 'name': str(identity), 'kind': {'type': 'scope'}}


def document(records):
    return {'version': 1, 'unit': 'nanoseconds', 'stopped': True, 'open': 0, 'dropped': 0, 'process': 1, 'records': records}


class Report(unittest.TestCase):
    def test_causality(self):
        source = record(1, None, 0, 10)
        execution = record(2, None, 20, 100)
        child = record(3, 1, 30, 90)
        child['enclosing'] = 2
        summary, _ = report.analyze(document([source, execution, child]))
        self.assertEqual(summary['groups']['1']['self']['total'], 10)
        self.assertEqual(summary['groups']['2']['self']['total'], 20)
        self.assertEqual(summary['groups']['3']['self']['total'], 60)

    def test_handoff(self):
        received = record(2, 1, 100, 140, 2)
        received['kind'] = {'type': 'receive', 'captured': 10, 'thread': 1}
        summary, timeline = report.analyze(document([record(1, None, 0, 20), received]))
        self.assertEqual(summary['groups']['2']['handoff']['total'], 90)
        self.assertEqual(summary['groups']['2']['wall']['total'], 40)
        self.assertEqual(summary['groups']['1']['self']['total'], 20)
        flow = [e for e in timeline['traceEvents'] if e['ph'] in ('s', 'f')]
        self.assertEqual([e['tid'] for e in flow], [1, 2])
        self.assertEqual(flow[0]['id'], flow[1]['id'])
        received['kind']['captured'] = 101
        with self.assertRaises(ValueError):
            report.analyze(document([received]))

    def test_union(self):
        self.assertEqual(report.union([(0, 5), (3, 8), (12, 14)]), 10)

    def test_accounting(self):
        source = document([record(1, None, 0, 100), record(2, 1, 10, 40), record(3, 1, 30, 50), record(4, 1, 20, 90, 2)])
        result, chrome = report.analyze(source)
        self.assertEqual(result['groups']['1']['self']['total'], 60)
        self.assertEqual(len(chrome['traceEvents']), 4)

    def test_loss(self):
        source = document([record(2, 1, 10, 20)])
        result, _ = report.analyze(source)
        self.assertFalse(result['complete'])
        self.assertIsNone(result['groups']['2']['self'])

    def test_duplicates(self):
        with self.assertRaises(ValueError):
            report.analyze(document([record(1, None, 0, 10)] * 2))


class Correlation(unittest.TestCase):
    def test_nested(self):
        import trace
        windows = [{'id': 1, 'start': 0, 'end': 100, 'thread': 7}, {'id': 2, 'start': 10, 'end': 30, 'thread': 7}, {'id': 3, 'start': 15, 'end': 25, 'thread': 8}]
        samples = [{'time': t, 'thread': 7} for t in (5, 20, 30, 100)] + [{'time': 20, 'thread': 8}]
        result = trace.assign(windows, samples)
        self.assertEqual([r['span'] for r in result], [1, 2, 1, None, 3])

    def test_expired(self):
        import trace
        windows = [{'id': 1, 'start': 0, 'end': 100, 'thread': 1}, {'id': 2, 'start': 1, 'end': 2, 'thread': 1}]
        self.assertEqual(trace.assign(windows, [{'time': 99, 'thread': 1}])[0]['span'], 1)

    def test_missing(self):
        import pathlib
        import tempfile
        import trace
        xml = '<trace-query-result><node><schema><col><mnemonic>subsystem</mnemonic></col></schema></node></trace-query-result>'
        with tempfile.TemporaryDirectory() as directory:
            source = pathlib.Path(directory) / 'empty.xml'
            source.write_text(xml)
            value = document([dict(record(1, None, 0, 10), signpost=True, native=1)])
            value['session'] = 1
            self.assertEqual(trace.intervals(source, value), ([], [1]))


class Outcomes(unittest.TestCase):
    def test_handled(self):
        root = record(1, None, 0, 100)
        failed = dict(record(2, 1, 5, 10), outcome='failure')
        summary, _ = report.analyze(document([root, failed]))
        self.assertTrue(summary['successful'])
        self.assertEqual(summary['groups']['2']['outcomes'], {'failure': 1})

    def test_absent(self):
        summary, _ = report.analyze(document([]))
        self.assertIsNone(summary['successful'])


class Native(unittest.TestCase):
    def test_identity(self):
        import pathlib
        import tempfile
        import trace
        keys = ['subsystem', 'process', 'identifier', 'thread', 'time', 'event-type', 'message']
        schema = ''.join(f'<col><mnemonic>{key}</mnemonic></col>' for key in keys)
        begin = '<subsystem id="1">io.plectron.observe</subsystem><process id="2"><pid>10</pid></process><identifier id="3">42</identifier><thread id="4"><tid>11</tid></thread><time>100</time><event>Begin</event><metadata><uint64>9</uint64><uint64>7</uint64></metadata>'
        end = '<subsystem ref="1"/><process ref="2"/><identifier ref="3"/><thread ref="4"/><time>200</time><event>End</event><metadata/>'
        xml = f'<trace-query-result><node><schema>{schema}</schema><row>{begin}</row><row>{end}</row></node></trace-query-result>'
        source = {'session': 9, 'process': 10, 'records': [{'id': 42, 'parent': 7, 'native': 11, 'signpost': True}]}
        with tempfile.TemporaryDirectory() as directory:
            path = pathlib.Path(directory) / 'native.xml'
            path.write_text(xml)
            windows, missing = trace.intervals(path, source)
            self.assertEqual(missing, [])
            self.assertEqual(windows, [{'id': 42, 'start': 100, 'end': 200, 'thread': 11}])
            source['session'] = 8
            with self.assertRaises(ValueError):
                trace.intervals(path, source)


class Passive(unittest.TestCase):
    def test_topology(self):
        task = {'admitted': 10, 'produced': 20, 'detected': 30}
        frame = {'admitted': 25, 'produced': 100, 'detected': 110}
        result = passive.durations(task, frame)
        self.assertEqual(result['owner_completion'], 100)
        self.assertEqual(result['task_publication'], 10)
        self.assertEqual(result['frame_production'], 75)
        for item in ({**frame, 'produced': None}, {**frame, 'detected': 99}):
            with self.assertRaises(RuntimeError):
                passive.durations(task, item)

    def test_terminal(self):
        class App:
            def request(self, command):
                return {'reply': {'status': {'type': 'record', 'record': {'state': {'type': 'pending'}}}}}
        with self.assertRaises(RuntimeError):
            passive.terminal(App(), 1, 'zero')
        with self.assertRaises(RuntimeError):
            passive.record({'status': {'type': 'evicted'}})
        with self.assertRaises(RuntimeError):
            passive.record({'status': {'type': 'record', 'record': {'state': {'type': 'complete', 'result': {'Err': 'failure'}}}}})


if __name__ == '__main__':
    unittest.main()
