#!/usr/bin/env python3
"""Serve docs/demo as a local GitHub Pages replica under /<repo>/."""

from __future__ import annotations

import argparse
import http.server
import socketserver
import urllib.parse
from pathlib import Path


ROOT_DIR = Path(__file__).resolve().parents[1]
DEFAULT_DEMO_DIR = ROOT_DIR / "docs" / "demo"
DEFAULT_REPO_NAME = ROOT_DIR.name


class DemoPagesHandler(http.server.SimpleHTTPRequestHandler):
    repo_name = DEFAULT_REPO_NAME

    def _rewrite_request_path(self) -> bool:
        parsed = urllib.parse.urlsplit(self.path)
        requested = urllib.parse.unquote(parsed.path or "/")

        if requested in {"", "/"}:
            self.send_response(302)
            self.send_header("Location", f"/{self.repo_name}/")
            self.end_headers()
            return False

        if requested == f"/{self.repo_name}":
            self.send_response(302)
            self.send_header("Location", f"/{self.repo_name}/")
            self.end_headers()
            return False

        prefix = f"/{self.repo_name}/"
        if not requested.startswith(prefix):
            self.send_error(
                404,
                f"Only '/{self.repo_name}/' is served by this local Pages replica.",
            )
            return False

        relative = requested[len(prefix) :]
        if not relative:
            relative = "index.html"

        sanitized = str(Path(relative).as_posix()).lstrip("/")
        if sanitized.startswith(".."):
            self.send_error(403, "Path traversal is not allowed.")
            return False

        rewritten = f"/{sanitized}"
        if parsed.query:
            rewritten = f"{rewritten}?{parsed.query}"
        self.path = rewritten
        return True

    def do_GET(self) -> None:  # noqa: N802
        if not self._rewrite_request_path():
            return
        super().do_GET()

    def do_HEAD(self) -> None:  # noqa: N802
        if not self._rewrite_request_path():
            return
        super().do_HEAD()

    def end_headers(self) -> None:
        # Avoid stale-browser artifacts while iterating locally.
        self.send_header("Cache-Control", "no-store, no-cache, must-revalidate, max-age=0")
        self.send_header("Pragma", "no-cache")
        self.send_header("Expires", "0")
        super().end_headers()


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Serve docs/demo as a local GitHub Pages replica under /<repo>/",
    )
    parser.add_argument("--host", default="127.0.0.1", help="Bind host (default: 127.0.0.1)")
    parser.add_argument("--port", type=int, default=4175, help="Bind port (default: 4175)")
    parser.add_argument(
        "--repo",
        default=DEFAULT_REPO_NAME,
        help=f"Repo path prefix (default: {DEFAULT_REPO_NAME})",
    )
    parser.add_argument(
        "--demo-dir",
        default=str(DEFAULT_DEMO_DIR),
        help=f"Static demo directory (default: {DEFAULT_DEMO_DIR})",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    demo_dir = Path(args.demo_dir).resolve()
    if not demo_dir.exists():
        raise SystemExit(f"error: demo directory does not exist: {demo_dir}")
    if not (demo_dir / "index.html").exists():
        raise SystemExit(f"error: missing {demo_dir / 'index.html'}")

    handler = lambda *a, **k: DemoPagesHandler(*a, directory=str(demo_dir), **k)  # noqa: E731
    DemoPagesHandler.repo_name = args.repo.strip().strip("/")
    socketserver.TCPServer.allow_reuse_address = True
    with socketserver.ThreadingTCPServer((args.host, args.port), handler) as server:
        print(f"Serving local Pages replica from: {demo_dir}")
        print(f"Repo prefix: /{DemoPagesHandler.repo_name}/")
        print(f"Demo URL: http://{args.host}:{args.port}/{DemoPagesHandler.repo_name}/")
        print("Press Ctrl+C to stop.")
        server.serve_forever()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
