#!/usr/bin/env python3
"""A fake Madara site for driving the app without any real site: `python3 scripts/demo-server.py [port]`.

Serves the hand-written fixtures from desktop/manga/testdata/fixtures/madara-example, with the page and cover
images replaced by generated pictures on this server, plus a repository under /_demo/repo/ (an index.json
that lists the madara theme and a source called `local`). Start the app with
`-demoRepo http://127.0.0.1:8770/_demo/repo/ -source local`.
"""
import hashlib, json, os, struct, sys, time, zlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2] / "desktop/manga/testdata"
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
        genres = [{"value": g.lower(), "name": g} for g in ["Action", "Adventure", "Comedy", "Drama", "Fantasy", "Horror", "Romance", "Sci-fi", "Slice of life", "Thriller"]]
        make = lambda id, name, extra: json.dumps({"extends": "madara", "id": id, "name": name, "version": 1, "lang": ["en"], "base_url": BASE, "rate_per_second": 20, **extra}, indent=2).encode()
        # `local` defines a genre filter (to see the filter UI); `second` only exists so there is more than one source.
        local = make("local", "Demo site", {"search": {"request": {"url": "{base}/page/{page}/?post_type=wp-manga{?&s={term}?}{?&genre={genre}?}"}}, "filters": [{"id": "genre", "name": "Genre", "kind": "select", "default": "", "options": [{"value": "", "name": "All"}] + genres}]})
        second = make("second", "Second site", {})
        sha = lambda data: hashlib.sha256(data).hexdigest()
        entry = lambda id, name, data: {"id": id, "name": name, "version": 1, "path": f"sources/{id}.json", "sha256": sha(data), "lang": ["en"], "requires": ["madara"]}
        index = {
            "schema": 1, "name": "Demo repository",
            "sources": [entry("local", "Demo site", local), entry("second", "Second site", second)],
            "themes": [{"id": "madara", "version": json.loads(theme)["version"], "path": "themes/madara.json", "sha256": sha(theme)}],
        }
        files = {"index.json": json.dumps(index, indent=2).encode(), "themes/madara.json": theme, "sources/local.json": local, "sources/second.json": second}
        if inside in files:
            return self.reply(files[inside], "application/json")
        self.send_response(404)
        self.end_headers()

    def handle_any(self, method):
        path = self.path.split("?")[0]
        if method == "POST":
            self.rfile.read(int(self.headers.get("content-length") or 0))
        # DEMO_LATENCY=0.3 makes every page of the site (not its pictures or the repository) take 300 ms, like a real network.
        if not path.startswith(("/solo/", "/wp-content/", "/_demo/")):
            time.sleep(float(os.environ.get("DEMO_LATENCY", 0)))
        if method == "GET" and path.startswith("/manga/solo-leveling/chapter-"):
            return self.reply(chapter_pages(path.strip("/").split("/")[-1]))
        if (method, path) in ROUTES:
            body = fixture(ROUTES[(method, path)])
            # `touch /tmp/akidev-demo-grow` and the site gains a Chapter 180: to see new-chapter alerts and downloads.
            if path.endswith("/ajax/chapters/") and Path("/tmp/akidev-demo-grow").exists():
                new = f'<ul class="main version-chap no-volumn">\n<li class="wp-manga-chapter"><a href="{BASE}/manga/solo-leveling/chapter-180/">Chapter 180</a><span class="chapter-release-date"><i>today</i></span></li>\n'
                body = body.replace(b'<ul class="main version-chap no-volumn">\n', new.encode(), 1)
            # DEMO_POPULAR=12 makes the popular list twelve series with titles of every length: to see the Top 10 shelf line up.
            if path == "/manga/page/1/" and os.environ.get("DEMO_POPULAR"):
                titles = ["Solo", "A Fortune-Telling Princess", "Try Begging Me", "The Regressed Extra Who Was Only Trying To Live Quietly In The Academy", "Tower", "Secret Love Affair Between a Former Saint and the Dark Duke", "Eleceed", "I", "Return of the Mount Hua Sect", "The Beginning After the End Season Two", "Omniscient Reader", "Lookism"]
                cards = "".join(f'<div class="page-item-detail manga"><div class="item-thumb"><a href="{BASE}/manga/series-{i}/"><img src="/wp-content/c/{i}.jpg"></a></div><div class="item-summary"><div class="post-title font-title"><h3 class="h5"><a href="{BASE}/manga/series-{i}/">{t}</a></h3></div></div></div>' for i, t in enumerate(titles[: int(os.environ["DEMO_POPULAR"])]))
                body = f'<html><body><div class="c-page">{cards}</div></body></html>'.encode()
            # DEMO_EXTRA=8 adds eight older chapters (every third a tall webtoon strip): to scroll through a long strip of many chapters.
            if path.endswith("/ajax/chapters/") and os.environ.get("DEMO_EXTRA"):
                more = ""
                for n in range(177, 177 - int(os.environ["DEMO_EXTRA"]), -1):
                    slug = f"chapter-{n}" if n % 3 else f"chapter-{n}-5"
                    more += f'<li class="wp-manga-chapter"><a href="{BASE}/manga/solo-leveling/{slug}/">Chapter {n}</a><span class="chapter-release-date"><i>Jan {max(1, n - 150)}, 2024</i></span></li>\n'
                body = body.replace(b"</ul>", more.encode() + b"</ul>", 1)
            return self.reply(body)
        if path.startswith("/_demo/sources/"):
            # The demo repository folder (desktop/manga/testdata), as GitHub would serve it.
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
