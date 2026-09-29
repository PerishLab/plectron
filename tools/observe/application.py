import json
import os
import pathlib
import socket
import subprocess
import time


class Application:
    def __init__(self, binary, root, renderer='cpu', project=None):
        self.binary = pathlib.Path(binary).resolve(strict=True)
        self.root = root
        root.mkdir(parents=True, exist_ok=False)
        self.environment = {**os.environ, 'PLECTRON_CONTROL_DIR': str(root / 'control'), 'PLECTRON_RENDERER': renderer}
        if project is None:
            source = root / 'main.js'
            source.write_text('let count = 0;')
            command = [str(self.binary), 'run', '--wait', str(source)]
        else:
            command = [str(self.binary), 'dev', 'start', str(pathlib.Path(project).resolve(strict=True))]
        self.stdout = (root / 'stdout.log').open('w')
        self.stderr = (root / 'stderr.log').open('w')
        self.child = subprocess.Popen(command, env=self.environment, stdout=self.stdout, stderr=self.stderr)
        self.socket = root / 'control' / f'{self.child.pid}.sock'

    def ready(self):
        deadline = time.monotonic() + 10
        while not self.socket.exists():
            if self.child.poll() is not None or time.monotonic() >= deadline:
                raise RuntimeError(f'application startup failed: {self.root}')
            time.sleep(0.005)
        while time.monotonic() < deadline:
            jobs = self.request({'type': 'control'})['snapshot']['jobs']
            if any(j['kind'] == 'startup' and j['phase'] == 'complete' for j in jobs):
                if any(j.get('result', {}).get('Err') for j in jobs):
                    raise RuntimeError(f'startup rejected: {jobs}')
                return
        raise RuntimeError('startup completion timed out')

    def request(self, command):
        with socket.socket(socket.AF_UNIX) as connection:
            connection.settimeout(6)
            connection.connect(str(self.socket))
            connection.sendall(json.dumps({'version': 1, 'command': command}).encode() + b'\n')
            with connection.makefile('rb') as stream:
                response = json.loads(stream.readline(8_388_608))
        if response['version'] != 1 or 'Ok' not in response['result']:
            raise RuntimeError(f'IPC rejected {command}: {response}')
        return response['result']['Ok']

    def evaluate(self, source):
        start = time.perf_counter_ns()
        accepted = self.request({'type': 'eval', 'source': source})
        admission = time.perf_counter_ns() - start
        polls = 0
        deadline = time.monotonic() + 10
        while time.monotonic() < deadline:
            state = self.request({'type': 'control'})['snapshot']
            polls += 1
            job = next(j for j in state['jobs'] if j['id'] == accepted['job'])
            if job['phase'] == 'complete':
                end = time.perf_counter_ns()
                if 'Ok' not in job['result']:
                    raise RuntimeError(f'job failed: {job}')
                return {'wall': end - start, 'admission': admission, 'polls': polls, 'result': job['result']['Ok']}
        raise RuntimeError('job completion timed out')

    def observation(self, command):
        return self.request({'type': 'observe', 'command': command})['status']

    def export(self, identity):
        target = self.root / 'observation.json'
        self.observation({'type': 'stop', 'id': identity})
        deadline = time.monotonic() + 5
        while self.observation({'type': 'status'})['open']:
            if time.monotonic() >= deadline:
                raise RuntimeError('observation did not drain')
        result = subprocess.run([str(self.binary), 'observe', str(self.child.pid), str(identity), str(target)], env=self.environment, capture_output=True, timeout=15)
        if result.returncode:
            raise RuntimeError(result.stderr.decode())
        self.observation({'type': 'discard', 'id': identity})
        return target

    def close(self):
        try:
            if self.child.poll() is None:
                result = self.request({'type': 'shutdown'})
                if result['type'] != 'shutdown' or result['errors']:
                    raise RuntimeError(f'shutdown failed: {result}')
                self.child.wait(timeout=10)
            if self.child.returncode != 0 or self.socket.exists():
                raise RuntimeError(f'unclean application exit: {self.root}')
        finally:
            if self.child.poll() is None:
                self.child.kill()
                self.child.wait()
            self.stdout.close()
            self.stderr.close()
