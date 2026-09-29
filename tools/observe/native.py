import json
import os
import pathlib
import signal
import subprocess
import sys
import xml.etree.ElementTree as ET

import report
import trace


def invoke(command, log, timeout=60):
    with log.open('w') as stream:
        result = subprocess.run(command, stdout=stream, stderr=subprocess.STDOUT, stdin=subprocess.DEVNULL, timeout=timeout)
    if result.returncode:
        raise RuntimeError(f'command failed ({result.returncode}); see {log}')


def export(source, schema, target):
    invoke(['xcrun', 'xctrace', 'export', '--input', str(source), '--xpath', f'/trace-toc/run[@number="1"]/data/table[@schema="{schema}"]', '--output', str(target)], target.with_suffix('.log'))


def cleanup(toc, binary):
    if not toc.exists():
        return
    target = ET.parse(toc).find('.//target/process')
    if target is None or 'return-exit-status' in target.attrib:
        return
    pid = int(target.attrib['pid'])
    command = subprocess.run(['ps', '-ww', '-p', str(pid), '-o', 'command='], capture_output=True, text=True).stdout.strip()
    if command.startswith(str(binary) + ' '):
        try:
            os.kill(pid, signal.SIGTERM)
        except ProcessLookupError:
            pass


def record(args):
    if sys.platform != 'darwin':
        raise ValueError('native recording requires macOS; spec trace and report remain portable')
    if args.seconds <= 0 or (args.limit is not None and args.limit < 0):
        raise ValueError('seconds must be positive and limit nonnegative')
    root = args.output.resolve()
    root.mkdir(parents=True, exist_ok=False)
    binary = args.binary.resolve(strict=True)
    source = root / 'cpu.trace'
    target = root / 'scenario'
    options = {'native': True, 'cpu': True}
    if args.limit is not None:
        options['limit'] = args.limit
    command = ['xcrun', 'xctrace', 'record', '--template', 'CPU Counters', '--output', str(source), '--time-limit', f'{args.seconds}s', '--no-prompt', '--target-stdout', str(root / 'target.log'), '--launch', '--', str(binary), 'trace', str(args.plan.resolve(strict=True)), '--output', str(target), '--options', json.dumps(options)]
    (root / 'command.json').write_text(json.dumps(command))
    toc = root / 'toc.xml'
    try:
        invoke(command, root / 'record.log', args.seconds + 60)
        with toc.open('w') as stream:
            subprocess.run(['xcrun', 'xctrace', 'export', '--input', str(source), '--toc'], stdout=stream, check=True, timeout=60)
    finally:
        cleanup(toc, binary)
    document = target / 'observation.json'
    if not document.exists():
        raise RuntimeError('target did not finish; partial recording retained')
    for schema in ('os-signpost', 'CountingModeSamples', 'kdebug-counters-with-time-sample'):
        export(source, schema, root / f'{schema}.xml')
    summary = report.write(document, root / 'analysis')
    correlated = trace.correlate(root)
    return {'matched': correlated['matched'], 'missing': len(correlated['missing']), 'samples': correlated['samples'], 'successful': summary['successful'], 'complete': summary['complete'], 'observation': str(document), 'native': str(source), 'analysis': str(root / 'analysis')}
