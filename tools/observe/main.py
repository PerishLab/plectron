import argparse
import json
import pathlib
import sys

import report


def main():
    parser = argparse.ArgumentParser()
    commands = parser.add_subparsers(dest='command', required=True)
    inspect = commands.add_parser('report')
    inspect.add_argument('source', type=pathlib.Path)
    inspect.add_argument('--output', type=pathlib.Path, required=True)
    handoff = commands.add_parser('handoff')
    handoff.add_argument('source', type=pathlib.Path)
    handoff.add_argument('--output', type=pathlib.Path, required=True)
    ipc = commands.add_parser('ipc')
    for name in ('baseline', 'internal', 'candidate', 'output'):
        ipc.add_argument('--' + name, type=pathlib.Path, required=True)
    ipc.add_argument('--rounds', type=int, default=3)
    ipc.add_argument('--runs', type=int, default=40)
    frames = commands.add_parser('frames')
    for name in ('baseline', 'candidate', 'output', 'apps'):
        frames.add_argument('--' + name, type=pathlib.Path, required=True)
    frames.add_argument('--rounds', type=int, default=3)
    frames.add_argument('--runs', type=int, default=20)
    frames.add_argument('--interval', type=float, default=0)
    passive = commands.add_parser('passive')
    for name in ('binary', 'output', 'apps'):
        passive.add_argument('--' + name, type=pathlib.Path, required=True)
    passive.add_argument('--rounds', type=int, default=3)
    passive.add_argument('--runs', type=int, default=20)
    activity = commands.add_parser('activity')
    for name in ('binary', 'application', 'output'):
        activity.add_argument('--' + name, type=pathlib.Path, required=True)
    activity.add_argument('--backends', nargs='+', choices=('cpu', 'metal'), default=['cpu', 'metal'])
    activity.add_argument('--rounds', type=int, default=3)
    activity.add_argument('--seconds', type=float, default=2)
    feel = commands.add_parser('feel')
    for name in ('binary', 'project', 'output'):
        feel.add_argument('--' + name, type=pathlib.Path, required=True)
    feel.add_argument('--label', default='manual')
    feel.add_argument('--drive', choices=('manual', 'forward', 'reversal'), default='manual')
    feel.add_argument('--seconds', type=float, default=20)
    feel.add_argument('--interval', type=float, default=2)
    correlate = commands.add_parser('correlate')
    correlate.add_argument('source', type=pathlib.Path)
    for name in ('record', 'bench'):
        capture = commands.add_parser(name)
        capture.add_argument('plan', type=pathlib.Path)
        capture.add_argument('--output', type=pathlib.Path, required=True)
        capture.add_argument('--binary', type=pathlib.Path, default=pathlib.Path(__file__).resolve().parents[2] / 'target/release/spec')
        capture.add_argument('--seconds', type=int, default=30)
        capture.add_argument('--limit', type=int)
        if name == 'bench':
            capture.add_argument('--rounds', type=int, default=3)
            capture.add_argument('--runs', type=int)
            capture.add_argument('--modes', nargs='+', choices=('off', 'wall', 'cpu', 'native'), default=['off', 'wall', 'cpu', 'native'])
    args = parser.parse_args()
    if args.command == 'report':
        result = report.write(args.source, args.output)
    elif args.command == 'correlate':
        import trace
        result = trace.correlate(args.source)
    elif args.command == 'bench':
        import bench
        result = bench.run(args)
    elif args.command == 'handoff':
        import handoff
        result = handoff.write(args.source, args.output)
    elif args.command == 'ipc':
        import ipc
        result = ipc.run(args)
    elif args.command == 'passive':
        import passive
        result = passive.run(args)
    elif args.command == 'frames':
        import frames
        result = frames.run(args)
    elif args.command == 'activity':
        import activity
        result = activity.run(args)
    elif args.command == 'feel':
        import feel
        result = feel.run(args)
    else:
        import native
        result = native.record(args)
    print(json.dumps(result, indent=2))


if __name__ == '__main__':
    try:
        main()
    except (OSError, ValueError, RuntimeError) as error:
        print(f'observe: {error}', file=sys.stderr)
        sys.exit(1)
