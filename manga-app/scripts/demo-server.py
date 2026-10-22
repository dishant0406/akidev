#!/usr/bin/env python3
"""A fake Madara site for driving the app without any real site: `python3 scripts/demo-server.py [port]`.

Serves the hand-written fixtures from manga-sources/fixtures/madara-example, with the page and cover
images replaced by generated pictures on this server, plus a repository under /_demo/repo/ (an index.json
that lists the madara theme and a source called `local`). Start the app with
`-demoRepo http://127.0.0.1:8770/_demo/repo/ -source local`.
"""
import hashlib, json, os, struct, sys, time, zlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2] / "manga-sources"
FIX = ROOT / "fixtures/madara-example"
PORT = int(sys.argv[1]) if len(sys.argv) > 1 else 8770
BASE = f"http://127.0.0.1:{PORT}"


GLYPHS = {
    "0": ["01110", "10001", "10011", "10101", "11001", "10001", "01110"],
    "1": ["00100", "01100", "00100", "00100", "00100", "00100", "01110"],
    "2": ["01110", "10001", "00001", "00010", "00100", "01000", "11111"],
    "3": ["11110", "00001", "00001", "01110", "00001", "00001", "11110"],
    "4": ["00010", "00110", "01010", "10010", "11111", "00010", "00010"],
    "5": ["11111", "10000", "11110", "00001", "00001", "10001", "01110"],
    "6": ["00110", "01000", "10000", "11110", "10001", "10001", "01110"],
    "7": ["11111", "00001", "00010", "00100", "01000", "01000", "01000"],
    "8": ["01110", "10001", "10001", "01110", "10001", "10001", "01110"],
    "9": ["01110", "10001", "10001", "01111", "00001", "00010", "01100"],
    "C": ["01110", "10001", "10000", "10000", "10000", "10001", "01110"],
    "P": ["11110", "10001", "10001", "11110", "10000", "10000", "10000"],
    "Y": ["10001", "10001", "01010", "00100", "00100", "00100", "00100"],
    "-": ["00000", "00000", "00000", "11111", "00000", "00000", "00000"],
    " ": ["00000"] * 7,
}


def png(width, height, chapter, page):
    """A strip of coloured bands, each labelled with its chapter, page and distance down (Y in hundreds
    of pixels), so a screenshot shows exactly which part of which page is on screen. Standard library only."""
    scale = 14
    band = 400
    rows = []
    for y in range(height):
        index = y // band
        hue = (page * 3 + index) % 7
        base = [(60 + hue * 25 + c * 40) % 200 + 30 for c in (0, 3, 6)]
        rows.append(bytearray(bytes(base) * width))
    for index in range(height // band + 1):
        text = f"C{chapter} P{page} Y{index * 4}"
        top = index * band + 30
        for gi, ch in enumerate(text):
            glyph = GLYPHS.get(ch, GLYPHS[" "])
            for gy, line in enumerate(glyph):
                for gx, bit in enumerate(line):
                    if bit == "1":
                        for dy in range(scale):
                            yy = top + gy * scale + dy
                            if yy >= height:
                                continue
                            x0 = 30 + (gi * 6 + gx) * scale
                            for dx in range(scale):
                                if x0 + dx < width:
                                    rows[yy][(x0 + dx) * 3:(x0 + dx) * 3 + 3] = b"\xff\xff\xff"
        if index * band < height:
            rows[min(index * band, height - 1)][:] = b"\x00" * (width * 3)
    raw = bytearray()
    for line in rows:
        raw.append(0)
        raw += line

    def chunk(kind, data):
        body = kind + data
        return struct.pack(">I", len(data)) + body + struct.pack(">I", zlib.crc32(body))

    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0)) + chunk(b"IDAT", zlib.compress(bytes(raw), 1)) + chunk(b"IEND", b"")


def chapter_pages(slug):
    """A chapter's page list: decimal chapters are tall webtoon strips, the rest are portrait manga pages."""
    number = slug.replace("chapter-", "")
    tall = "-" in number
    count = 3 if tall else 6
    images = "".join(f'<div class="page-break"><img data-src="{BASE}/solo/{number}/{n:03d}.png" class="wp-manga-chapter-img"></div>' for n in range(1, count + 1))
    return f'<html><body><div class="reading-content">{images}</div></body></html>'.encode()


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

    def repo(self, inside):
        """A repository (index.json plus the madara theme and a source called `local`), the way a GitHub repo
        would serve it: the index names each file with its sha256."""
        theme = (ROOT / "themes/madara.json").read_bytes()
        source = json.dumps({"extends": "madara", "id": "local", "name": "Demo site", "version": 1, "lang": ["en"], "base_url": BASE, "rate_per_second": 20}, indent=2).encode()
        sha = lambda data: hashlib.sha256(data).hexdigest()
        index = {
            "schema": 1, "name": "Demo repository",
            "sources": [{"id": "local", "name": "Demo site", "version": 1, "path": "sources/local.json", "sha256": sha(source), "lang": ["en"], "requires": ["madara"]}],
            "themes": [{"id": "madara", "version": json.loads(theme)["version"], "path": "themes/madara.json", "sha256": sha(theme)}],
        }
        files = {"index.json": json.dumps(index, indent=2).encode(), "themes/madara.json": theme, "sources/local.json": source}
        if inside in files:
            return self.reply(files[inside], "application/json")
        self.send_response(404)
        self.end_headers()

    def handle_any(self, method):
        path = self.path.split("?")[0]
        if method == "POST":
            self.rfile.read(int(self.headers.get("content-length") or 0))
        if method == "GET" and path.startswith("/manga/solo-leveling/chapter-"):
            return self.reply(chapter_pages(path.strip("/").split("/")[-1]))
        if (method, path) in ROUTES:
            return self.reply(fixture(ROUTES[(method, path)]))
        if path.startswith("/_demo/sources/"):
            # The real repository folder (manga-sources), as GitHub would serve it.
            target = (ROOT / path[len("/_demo/sources/"):]).resolve()
            if ROOT.resolve() in target.parents and target.is_file():
                return self.reply(target.read_bytes(), "application/json")
        if path.startswith("/_demo/repo/"):
            return self.repo(path[len("/_demo/repo/"):])
        if path.startswith("/solo/"):
            # DEMO_SLOW=3 holds each page picture for 3 seconds (to see the loading look); DEMO_FAIL=1 refuses them.
            time.sleep(float(os.environ.get("DEMO_SLOW", 0)))
            if os.environ.get("DEMO_FAIL"):
                self.send_response(500)
                self.end_headers()
                return
            _, _, number, name = path.split("/")
            page = int(name.split(".")[0])
            tall = "-" in number
            return self.reply(png(800, 4000, number, page) if tall else png(800, 1200, number, page), "image/png")
        if path.startswith("/wp-content/"):
            return self.reply(png(300, 450, "COVER", len(path) % 9), "image/png")
        self.send_response(404)
        self.end_headers()

    def do_GET(self):
        self.handle_any("GET")

    def do_POST(self):
        self.handle_any("POST")

    def log_message(self, fmt, *args):
        # DEMO_LOG=1 prints each request, to see what the app asks for and when.
        if os.environ.get("DEMO_LOG"):
            print(self.command, self.path, flush=True)


print(f"demo site on {BASE}")
ThreadingHTTPServer(("127.0.0.1", PORT), Handler).serve_forever()
