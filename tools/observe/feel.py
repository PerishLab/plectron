import hashlib
import json
import statistics
import sys
import time

import application
import frames
import report


def distribution(values):
    values = sorted(values)
    if not values:
        return None
    return {
        'median': statistics.median(values),
        'p95': values[(len(values) * 95 + 99) // 100 - 1],
        'p99': values[(len(values) * 99 + 99) // 100 - 1],
        'maximum': values[-1],
    }


def query(app, command):
    return app.request({'type': 'query', 'command': command})['reply']


def selected(reply):
    status = reply['status']
    if status['type'] == 'record':
        return status['record']
    return None


def evaluate(app, source):
    result = app.request({'type': 'evaluate', 'unit': 1, 'source': source})
    if result['type'] == 'evaluation':
        return result['display']
    identity = result['job']
    deadline = time.monotonic() + 10
    while time.monotonic() < deadline:
        snapshot = frames.page(app)
        job = next(job for job in snapshot['execution']['jobs'] if job['id'] == identity)
        if job['phase'] == 'complete':
            if 'Ok' not in job['result']:
                raise RuntimeError(f'content evaluation failed: {job}')
            return job['result']['Ok']
        time.sleep(0.002)
    raise RuntimeError('content evaluation timed out')


def initial(app):
    started = time.perf_counter_ns()
    deadline = time.monotonic() + 30
    while True:
        try:
            snapshot, queries, inspection = frames.wait(app, interval=0.002)
            return snapshot, time.perf_counter_ns() - started, queries, inspection
        except RuntimeError as error:
            if ('request handler did not reply' not in str(error)
                    or time.monotonic() >= deadline):
                raise


def collect(app, seconds, interval, drive='manual'):
    initial = selected(query(app, {'type': 'latest', 'unit': 1}))
    if initial is None:
        raise RuntimeError('initial frame record is absent')
    cursor = initial['id']
    records = {}
    evicted = []
    deadline = time.monotonic() + seconds
    nextdrive = time.monotonic()
    direction = 1
    while time.monotonic() < deadline:
        now = time.monotonic()
        if drive != 'manual' and now >= nextdrive:
            if drive == 'reversal' and now >= deadline - seconds / 2:
                direction = -1
            app.request({
                'type': 'wheel',
                'unit': 1,
                'x': 360,
                'y': 360,
                'delta': direction * 80,
            })
            nextdrive += 1 / 60
        latest = selected(query(app, {'type': 'latest', 'unit': 1}))
        if latest is not None:
            for identity in range(cursor + 1, latest['id'] + 1):
                reply = query(app, {'type': 'read', 'record': identity})
                current = selected(reply)
                if current is not None:
                    if current['target']['type'] == 'frame' and current['target']['unit'] == 1:
                        records[identity] = current
                elif reply['status']['type'] == 'evicted':
                    evicted.append(identity)
            cursor = max(cursor, latest['id'])
        for identity in list(records):
            if records[identity]['visible'] is None:
                current = selected(query(app, {'type': 'read', 'record': identity}))
                if current is not None:
                    records[identity] = current
        time.sleep(min(interval, max(0, nextdrive - time.monotonic())) if drive != 'manual' else interval)
    for _ in range(20):
        pending = [identity for identity, value in records.items() if value['visible'] is None]
        if not pending:
            break
        for identity in pending:
            current = selected(query(app, {'type': 'read', 'record': identity}))
            if current is not None:
                records[identity] = current
        time.sleep(interval)
    return [records[key] for key in sorted(records)], sorted(set(evicted))


def refresh(app, records):
    for index, record in enumerate(records):
        if record['state']['type'] != 'complete' or record['visible'] is None:
            current = selected(query(app, {'type': 'read', 'record': record['id']}))
            if current is not None:
                records[index] = current


def summarize(records, evicted):
    complete = [record for record in records if record['state']['type'] == 'complete']
    successful = [record for record in complete if 'Ok' in record['state']['result']]
    visible = [record for record in successful if record['visible'] is not None]
    production = [record['produced'] - record['admitted'] for record in successful if record['produced'] is not None]
    visibility = [record['visible'] - record['admitted'] for record in visible]
    presentation = [record['visible'] - record['produced'] for record in visible if record['produced'] is not None]
    detection = [record['detected'] - record['produced'] for record in successful if record['produced'] is not None]
    cadence = []
    for left, right in zip(visible, visible[1:]):
        if right['admitted'] - left['admitted'] <= 100_000_000:
            cadence.append(right['visible'] - left['visible'])
    visible_span = None
    if len(visible) > 1:
        visible_span = visible[-1]['visible'] - visible[0]['visible']
    return {
        'frames': len(records),
        'complete': len(complete),
        'successful': len(successful),
        'visible': len(visible),
        'missing_visibility': len(successful) - len(visible),
        'evicted': evicted,
        'drops': {
            'total': sum(record['drops'] for record in records),
            'maximum': max((record['drops'] for record in records), default=0),
        },
        'visible_span': visible_span,
        'timing': {
            'admission_to_production': distribution(production),
            'admission_to_visibility': distribution(visibility),
            'production_to_visibility': distribution(presentation),
            'production_to_detection': distribution(detection),
            'active_visibility_cadence': distribution(cadence),
        },
    }


def assess(summary, final, observation, drive, seconds):
    reasons = []
    findings = []
    if not observation['complete'] or not observation['successful']:
        reasons.append('observation is incomplete or unsuccessful')
    if summary['evicted']:
        reasons.append('frame records were evicted')
    if summary['missing_visibility']:
        reasons.append('backend-complete frames lack physical visibility receipts')
    if summary['drops']['total']:
        findings.append('presentation reported dropped frames')
    if not final['settled']:
        reasons.append('presentation did not settle after input')
    if summary['frames'] == 0:
        reasons.append('no changed frame was observed')
    if drive != 'manual':
        span = summary['visible_span'] or 0
        if span < seconds * 1_000_000_000 * 0.8:
            reasons.append('physical visibility covers less than 80% of scripted input')
    return {'valid': not reasons, 'reasons': reasons, 'findings': findings}


def drain(app, seconds=2):
    deadline = time.monotonic() + seconds
    snapshot = frames.page(app)
    while not frames.settled(snapshot) and time.monotonic() < deadline:
        time.sleep(0.002)
        snapshot = frames.page(app)
    return snapshot


def run(args):
    if sys.platform != 'darwin':
        raise ValueError('feel recording requires macOS physical presentation evidence')
    if args.seconds <= 0 or args.interval <= 0:
        raise ValueError('seconds and interval must be positive')
    root = args.output.resolve()
    root.mkdir(parents=True, exist_ok=False)
    binary = args.binary.resolve(strict=True)
    project = args.project.resolve(strict=True)
    app = application.Application(binary, root / 'application', 'metal', project)
    observation = None
    try:
        app.ready()
        previous, startup, startup_queries, startup_inspection = initial(app)
        previous, _ = frames.action(app, previous, 'scenario.start(); 1;')
        observation = app.observation({'type': 'start', 'options': {}})['session']
        action = 'interact with the Plectron window now' if args.drive == 'manual' else f'using {args.drive} display-path input'
        print(f'feel: recording {args.label!r} for {args.seconds:g}s; {action}', flush=True)
        records, evicted = collect(app, args.seconds, args.interval / 1000, args.drive)
        finalrange = evaluate(app, 'scenario.range();')
        snapshot = drain(app)
        refresh(app, records)
        trace = report.write(app.export(observation), root / 'analysis')
        observation = None
    finally:
        if observation is not None:
            try:
                app.export(observation)
            except (OSError, RuntimeError):
                pass
        app.close()
    (root / 'frames.json').write_text(json.dumps(records, indent=2))
    final = {
        'state': snapshot['state'],
        'revision': snapshot['revision'],
        'frame': snapshot['frame'],
        'presented': snapshot['presented'],
        'displayed': snapshot['displayed'],
        'issued': snapshot['issued'],
        'pending': snapshot['pending'],
        'settled': frames.settled(snapshot),
        'range': finalrange,
    }
    observation = {
        'complete': trace['complete'],
        'successful': trace['successful'],
        'dropped': trace['dropped'],
        'open': trace['open'],
    }
    summary = summarize(records, evicted)
    result = {
        'version': 1,
        'unit': 'nanoseconds',
        'label': args.label,
        'drive': args.drive,
        'seconds': args.seconds,
        'poll_interval_ms': args.interval,
        'startup': {
            'ready_to_settled': startup,
            'queries': startup_queries,
            'inspection': startup_inspection,
        },
        'binary': {
            'path': str(binary),
            'sha256': hashlib.sha256(binary.read_bytes()).hexdigest(),
        },
        'project': str(project),
        'endpoint': 'Metal drawable presentedTime with frame submission, backend completion and owner detection on the platform-lifetime monotonic clock',
        'limits': [
            'Physical presentation timestamp is not photon arrival or display scanout completion.',
            'Cadence includes adjacent visible frames only when their admissions are at most 100ms apart; input cadence can still dominate it.',
            'The query service is external to the application owner loop, but socket, query and observation overhead remain present.',
            'A manual run records one human gesture session; scripted driving calibrates the Display path without claiming native input behavior.',
        ],
        'assessment': assess(summary, final, observation, args.drive, args.seconds),
        'summary': summary,
        'final': final,
        'observation': observation,
    }
    (root / 'summary.json').write_text(json.dumps(result, indent=2))
    return result
