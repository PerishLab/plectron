import http.client
import json
from pathlib import Path
import selectors
import socket
import subprocess
import sys
import tempfile
import unittest
from urllib.parse import urlsplit


SCRIPT = Path(__file__).with_name("main.py")
TIMEOUT = 5


class Serving(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name) / "assets"
        self.root.mkdir()
        (self.root / "index.html").write_text("<p>cards</p>")
        (self.root / "style.css").write_text("body { color: blue; }")
        (self.root / "font.ttf").write_bytes(b"\x00font\xff")
        (self.root / "hello world.js").write_text("let label = '中文';")
        self.process = subprocess.Popen(
            [sys.executable, "-B", str(SCRIPT), "--root", str(self.root)],
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        )
        self.addCleanup(self.close)
        with selectors.DefaultSelector() as selector:
            selector.register(self.process.stdout, selectors.EVENT_READ)
            self.assertTrue(selector.select(TIMEOUT), "server did not announce readiness")
        self.ready = json.loads(self.process.stdout.readline())
        self.port = urlsplit(self.ready["url"]).port

    def close(self):
        if self.process.poll() is None:
            self.process.terminate()
        try:
            self.process.communicate(timeout=TIMEOUT)
        except subprocess.TimeoutExpired:
            self.process.kill()
            self.process.communicate()
            self.fail("server did not stop")

    def request(self, path, method="GET", headers=None):
        connection = http.client.HTTPConnection("127.0.0.1", self.port, timeout=TIMEOUT)
        try:
            connection.request(method, path, headers=headers or {})
            response = connection.getresponse()
            return response.status, response.headers, response.read()
        finally:
            connection.close()

    def test_resources(self):
        self.assertEqual(self.ready["root"], str(self.root.resolve()))
        self.assertEqual(self.ready["pid"], self.process.pid)
        for path, mime, body in [
            ("/", "text/html; charset=utf-8", b"<p>cards</p>"),
            ("/style.css?v=1", "text/css; charset=utf-8", b"body { color: blue; }"),
            ("/font.ttf", "font/ttf", b"\x00font\xff"),
            ("/hello%20world.js", "text/javascript; charset=utf-8", "let label = '中文';".encode()),
        ]:
            status, headers, actual = self.request(path)
            self.assertEqual((status, actual), (200, body))
            self.assertEqual(headers["Content-Type"], mime)
            self.assertEqual(headers["Cache-Control"], "no-store")
            self.assertEqual(int(headers["Content-Length"]), len(body))
        status, headers, body = self.request("/font.ttf", "HEAD")
        self.assertEqual((status, body, headers["Content-Length"]), (200, b"", "6"))
        modified = headers["Last-Modified"]
        (self.root / "font.ttf").write_bytes(b"changed")
        status, _, body = self.request("/font.ttf", headers={"If-Modified-Since": modified})
        self.assertEqual((status, body), (200, b"changed"))
        self.assertEqual(self.request("/missing")[0], 404)

    def test_scope(self):
        outside = self.root.parent / "private.txt"
        outside.write_text("outside")
        (self.root / "escape.txt").symlink_to(outside)
        (self.root / "nested").mkdir()
        (self.root / "nested" / "index.html").symlink_to(outside)
        (self.root / "empty").mkdir()
        for path in ["/escape.txt", "/nested/"]:
            self.assertEqual(self.request(path)[0], 403)
        self.assertEqual(self.request("/empty/")[0], 404)
        status, _, body = self.request("/%2e%2e/private.txt")
        self.assertEqual(status, 404)
        self.assertNotIn(b"outside", body)

    def test_lifecycle(self):
        self.process.terminate()
        self.process.wait(timeout=TIMEOUT)
        self.assertEqual(self.process.returncode, 0)
        with self.assertRaises(OSError):
            socket.create_connection(("127.0.0.1", self.port), timeout=TIMEOUT)
        for arguments in [["--root", str(self.root / "missing")], ["--port", "65536"]]:
            result = subprocess.run(
                [sys.executable, "-B", str(SCRIPT), *arguments],
                capture_output=True,
                timeout=TIMEOUT,
            )
            self.assertEqual(result.returncode, 2)
            self.assertEqual(result.stdout, b"")


if __name__ == "__main__":
    unittest.main()
