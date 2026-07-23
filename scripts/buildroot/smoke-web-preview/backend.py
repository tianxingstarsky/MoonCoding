#!/usr/bin/env python3
"""Minimal preview backend — binds host-assigned MOONCODING_BACKEND_PORT."""
from __future__ import annotations

import json
import os
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer


HOST = os.environ.get("MOONCODING_BACKEND_HOST", "127.0.0.1")
PORT = int(os.environ.get("MOONCODING_BACKEND_PORT", "18765"))
API_BASE = os.environ.get("MOONCODING_API_BASE", f"http://{HOST}:{PORT}")


class Handler(BaseHTTPRequestHandler):
    def log_message(self, fmt: str, *args) -> None:  # quieter on board
        return

    def _json(self, code: int, payload: dict) -> None:
        body = json.dumps(payload).encode("utf-8")
        self.send_response(code)
        self.send_header("Content-Type", "application/json; charset=utf-8")
        self.send_header("Access-Control-Allow-Origin", "*")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_OPTIONS(self) -> None:
        self.send_response(204)
        self.send_header("Access-Control-Allow-Origin", "*")
        self.send_header("Access-Control-Allow-Methods", "GET, OPTIONS")
        self.send_header("Access-Control-Allow-Headers", "*")
        self.end_headers()

    def do_GET(self) -> None:
        if self.path in ("/", "/health", "/api/health"):
            self._json(200, {"ok": True, "api_base": API_BASE})
            return
        self._json(404, {"ok": False, "error": "not found"})


def main() -> None:
    server = ThreadingHTTPServer((HOST, PORT), Handler)
    print(f"READY {API_BASE}", flush=True)
    server.serve_forever()


if __name__ == "__main__":
    main()
