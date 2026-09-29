import argparse
import functools
import json
import os
from pathlib import Path
import signal
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer


class Handler(SimpleHTTPRequestHandler):
    extensions_map = {
        **SimpleHTTPRequestHandler.extensions_map,
        ".js": "text/javascript",
        ".ttf": "font/ttf",
        ".otf": "font/otf",
        ".woff": "font/woff",
        ".woff2": "font/woff2",
    }

    def guess_type(self, path):
        content = super().guess_type(path)
        if content.startswith("text/"):
            return content + "; charset=utf-8"
        return content

    def send_head(self):
        root = Path(self.directory)
        target = Path(self.translate_path(self.path))
        candidates = [target]
        if target.is_dir():
            candidates.extend(target / name for name in ("index.html", "index.htm"))
        try:
            contained = all(path.resolve().is_relative_to(root) for path in candidates)
        except (OSError, RuntimeError):
            contained = False
        if not contained:
            self.send_error(403, "Resource is outside the static root")
            return None
        if "If-Modified-Since" in self.headers:
            del self.headers["If-Modified-Since"]
        return super().send_head()

    def list_directory(self, path):
        self.send_error(404, "Directory has no index")
        return None

    def end_headers(self):
        self.send_header("Cache-Control", "no-store")
        super().end_headers()


def stop(signum, frame):
    raise KeyboardInterrupt


def main():
    parser = argparse.ArgumentParser(description="Serve local static assets on loopback HTTP.")
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[2] / "apps")
    parser.add_argument("--port", type=int, default=0, help="listen port; 0 selects a free port")
    args = parser.parse_args()
    if not 0 <= args.port <= 65535:
        parser.error("port must be between 0 and 65535")
    root = args.root.resolve()
    if not root.is_dir():
        parser.error("root must be an existing directory")
    handler = functools.partial(Handler, directory=str(root))
    try:
        server = ThreadingHTTPServer(("127.0.0.1", args.port), handler)
    except OSError as error:
        parser.exit(2, f"serve: {error}\n")
    signal.signal(signal.SIGTERM, stop)
    try:
        with server:
            print(json.dumps({
                "url": f"http://127.0.0.1:{server.server_port}/",
                "root": str(root),
                "pid": os.getpid(),
            }), flush=True)
            server.serve_forever()
    except KeyboardInterrupt:
        pass


if __name__ == "__main__":
    main()
