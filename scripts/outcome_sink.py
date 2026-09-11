#!/usr/bin/env python3
"""Stand-in for Worker POST /internal/outcome. HMAC-checked. Writes each body to a file."""
from __future__ import annotations

import hashlib
import hmac
import json
import os
import sys
from http.server import BaseHTTPRequestHandler, HTTPServer
from pathlib import Path

OUT = Path(os.environ["SEAM_OUTCOME_LOG"])
KEY_SECRET = os.environ["CURATOM_HMAC_KEY"]


def key_bytes(secret: str) -> bytes:
    s = secret.strip()
    try:
        import base64

        return base64.b64decode(s, validate=True)
    except Exception:
        return s.encode("utf-8")


KEY = key_bytes(KEY_SECRET)


class Handler(BaseHTTPRequestHandler):
    def do_GET(self):
        self.send_response(200)
        self.end_headers()
        self.wfile.write(b"ok")

    def do_POST(self):
        n = int(self.headers.get("Content-Length", "0"))
        body = self.rfile.read(n)
        provided = (self.headers.get("x-curatom-hmac") or "").encode()
        expected = hmac.new(KEY, body, hashlib.sha256).hexdigest().encode()
        if not hmac.compare_digest(provided, expected):
            self.send_response(401)
            self.end_headers()
            self.wfile.write(b'{"error":"bad hmac"}')
            return
        if self.path.rstrip("/") != "/internal/outcome":
            self.send_response(404)
            self.end_headers()
            return
        OUT.parent.mkdir(parents=True, exist_ok=True)
        with OUT.open("a") as f:
            f.write(body.decode() + "\n")
        self.send_response(200)
        self.send_header("content-type", "application/json")
        self.end_headers()
        self.wfile.write(b'{"ok":true}')


def main():
    port = int(os.environ.get("SEAM_SINK_PORT", "18080"))
    HTTPServer(("127.0.0.1", port), Handler).serve_forever()


if __name__ == "__main__":
    main()
