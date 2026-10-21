#!/usr/bin/env python3
"""A fake Madara site for driving the app without any real site: `python3 scripts/demo-server.py [port]`.

Serves the hand-written fixtures from desktop/manga/sources/fixtures/madara-example, with the page
and cover images replaced by generated pictures on this server, plus the theme and a source
definition under /_demo/. Start the app with `-demoServer http://127.0.0.1:8770 -source local`.
"""
import json, struct, sys, zlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2] / "desktop/manga/sources"
FIX = ROOT / "fixtures/madara-example"
PORT = int(sys.argv[1]) if len(sys.argv) > 1 else 8770
BASE = f"http://127.0.0.1:{PORT}"


def png(width, height, seed):
    """A tall strip of coloured panels, as a PNG made with the standard library."""
    rows = bytearray()
    for y in range(height):
        panel = (y // 160 + seed) % 7
        band = 200 if (y % 160) < 8 else 0
        r, g, b = [(40 + panel * 30 + c * 17) % 256 for c in (0, 3, 6)]
        rows.append(0)
        line = bytes([max(r - band, 0), max(g - band, 0), max(b - band, 0)]) * width
        rows += line
    def chunk(kind, data):
        body = kind + data
        return struct.pack(">I", len(data)) + body + struct.pack(">I", zlib.crc32(body))
    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0)) + chunk(b"IDAT", zlib.compress(bytes(rows), 1)) + chunk(b"IEND", b"")


def fixture(name):
    text = (FIX / name).read_text()
    text = text.replace("https://madara.example", BASE).replace("https://cdn.madara.example", BASE)
    return text.encode()


ROUTES = {
    ("GET", "/manga/page/1/"): "popular.html",
    ("GET", "/page/1/"): "search.html",
    ("GET", "/manga/solo-leveling/"): "detail.html",
    ("POST", "/manga/solo-leveling/ajax/chapters/"): "chapters.html",
}


class Handler(BaseHTTPRequestHandler):
    def reply(self, body, kind="text/html"):
        self.send_response(200)
        self.send_header("content-type", kind)
        self.send_header("content-length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def handle_any(self, method):
        path = self.path.split("?")[0]
        if method == "POST":
            self.rfile.read(int(self.headers.get("content-length") or 0))
        if method == "GET" and path.startswith("/manga/solo-leveling/chapter-"):
            return self.reply(fixture("pages.html"))
        if (method, path) in ROUTES:
            return self.reply(fixture(ROUTES[(method, path)]))
        if path == "/_demo/madara.json":
            return self.reply((ROOT / "themes/madara.json").read_bytes(), "application/json")
        if path == "/_demo/local.json":
            return self.reply(json.dumps({"extends": "madara", "id": "local", "name": "Demo site", "version": 1, "lang": ["en"], "base_url": BASE, "rate_per_second": 20}).encode(), "application/json")
        if path.startswith(("/wp-content/", "/solo/")):
            tall = path.startswith("/solo/")
            return self.reply(png(800, 6400, len(path)) if tall else png(300, 450, len(path)), "image/png")
        self.send_response(404)
        self.end_headers()

    def do_GET(self):
        self.handle_any("GET")

    def do_POST(self):
        self.handle_any("POST")

    def log_message(self, *args):
        pass


print(f"demo site on {BASE}")
ThreadingHTTPServer(("127.0.0.1", PORT), Handler).serve_forever()
