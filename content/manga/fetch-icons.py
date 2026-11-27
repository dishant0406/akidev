#!/usr/bin/env python3
"""Downloads the Solar icons the app uses into App/Assets.xcassets.

Solar is by 480 Design, licensed CC BY 4.0 (credit is given in Settings > About). It writes the iOS app's asset catalog and
the Android app's vector drawables (apps/manga-android/design) from the same download, and the drawables of the Android TV app
(apps/tv-android) from its own, shorter list (TV_USES). `fetch-icons.py --tv` writes only the TV ones, which leaves the other
apps as they are when Solar has changed an icon since they were written. Each name below is what the app
calls an icon; `name` is the outline version and `name-solid` the bold one (only for the names in SOLID). Run it again after
adding a name.
"""
import json
import pathlib
import re
import shutil
import subprocess
import sys
import xml.etree.ElementTree as ET

ICONS = {
    "home": "home-2", "library": "library", "inbox": "inbox", "sources": "widget-2", "settings": "settings",
    "add": "add-circle", "back": "alt-arrow-left", "down": "alt-arrow-down", "forward": "alt-arrow-right",
    "refresh": "refresh", "restore": "restart", "download": "download-minimalistic", "share": "share",
    "trash": "trash-bin-minimalistic", "folder": "folder", "folder-add": "add-folder", "filter": "filter",
    "order": "sort-from-top-to-bottom", "sliders": "tuning-2", "search": "magnifier", "bookmark": "bookmark",
    "bookmark-off": "bookmark-off", "check": "check-circle", "tick": "check", "clock": "clock-circle",
    "more": "menu-dots", "play": "play", "pause": "pause", "previous": "skip-previous", "next": "skip-next", "keyboard": "keyboard",
    "server": "server-2", "sparkles": "stars-minimalistic", "list": "list", "warning": "danger-triangle",
    "swap": "transfer-horizontal", "close": "close-circle", "zip": "file-zip", "image": "gallery", "book": "book-2",
    "headphones": "headphones-round", "qr": "qr-code", "lock": "lock",
}
# The Android TV app (apps/tv-android) draws these. Names above are reused; the ones here exist only for the TV.
TV_ONLY = {"film": "clapperboard", "tv": "tv", "info": "info-circle", "subtitles": "subtitles", "volume": "volume-loud", "eye": "eye", "eye-off": "eye-closed"}
TV_USES = [
    "home", "film", "tv", "sparkles", "search", "settings", "play", "pause", "info", "subtitles", "volume", "next", "check", "close",
    "warning", "refresh", "list", "back", "trash", "lock", "clock", "server", "download", "eye", "eye-off",
]
# Drawn filled on the TV: a tab that is shown, the play button, and the marks that say something is done or wrong.
TV_SOLID = {"home", "film", "tv", "sparkles", "search", "settings", "play", "check", "warning"}
STYLES = {"": "linear", "-solid": "bold"}
# Only these are drawn filled somewhere (a selected tab, a saved bookmark, a read tick, ...); the rest get no bold copy, which
# keeps the app smaller. Add a name here when the app starts to use "<name>-solid".
SOLID = {"book", "bookmark", "check", "close", "home", "inbox", "library", "lock", "play", "server", "sources", "warning"}
only_tv = "--tv" in sys.argv
root = pathlib.Path(__file__).resolve().parent.parent.parent / "apps" / "manga-ios" / "App" / "Assets.xcassets"
wanted = {f"{solar}-{style}": (name, suffix) for name, solar in ICONS.items() for suffix, style in STYLES.items() if suffix == "" or name in SOLID}
tv_names = {**ICONS, **TV_ONLY}
tv_wanted = {f"{tv_names[name]}-{style}": (name, suffix) for name in TV_USES for suffix, style in STYLES.items() if suffix == "" or name in TV_SOLID}
# One request for every icon: the service limits how often it is asked.
url = "https://api.iconify.design/solar.json?icons=" + ",".join({**wanted, **tv_wanted})
icons = json.loads(subprocess.run(["curl", "-fsSL", url], capture_output=True, text=True, check=True).stdout)["icons"]
missing = [icon for icon in {**wanted, **tv_wanted} if icon not in icons]
if missing:
    raise SystemExit(f"Solar has no icon called: {', '.join(missing)}")

if not only_tv:
    root.mkdir(exist_ok=True)
    (root / "Contents.json").write_text(json.dumps({"info": {"author": "xcode", "version": 1}}, indent=2))
    for stale in root.glob("solar-*.imageset"):
        shutil.rmtree(stale)

    for icon, (name, suffix) in wanted.items():
        # Xcode's SVG reader has no `currentColor`; the image is a template, so the colour is replaced by the app.
        body = icons[icon]["body"].replace("currentColor", "#000000")
        svg = f'<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24">{body}</svg>'
        folder = root / f"solar-{name}{suffix}.imageset"
        folder.mkdir(exist_ok=True)
        (folder / f"{name}{suffix}.svg").write_text(svg)
        (folder / "Contents.json").write_text(json.dumps({
            "images": [{"filename": f"{name}{suffix}.svg", "idiom": "universal"}],
            "info": {"author": "xcode", "version": 1},
            "properties": {"preserves-vector-representation": True, "template-rendering-intent": "template"},
        }, indent=2))
    print(f"{len(wanted)} icons written")


# ---- Android: one vector drawable per icon, and the list that `AkiIcon` looks them up in -----------------------------------

android = pathlib.Path(__file__).resolve().parent.parent.parent / "apps" / "manga-android" / "design" / "src" / "main"


def circle_path(cx, cy, r):
    return f"M{cx - r},{cy} a{r},{r} 0 1,0 {2 * r},0 a{r},{r} 0 1,0 {-2 * r},0"


def vector_paths(node, inherited):
    """The drawable `<path>` lines of an SVG node: `g` passes its attributes down, `circle` becomes a path."""
    attributes = {**inherited, **{key: value for key, value in node.attrib.items() if key not in ("d", "cx", "cy", "r")}}
    tag = node.tag.split("}")[1]
    if tag == "g":
        return [line for child in node for line in vector_paths(child, attributes)]
    data = node.attrib["d"] if tag == "path" else circle_path(float(node.attrib["cx"]), float(node.attrib["cy"]), float(node.attrib["r"]))
    lines = [f'    <path android:pathData="{data}"']
    fill, stroke = attributes.get("fill", "#000000"), attributes.get("stroke")
    if fill != "none":
        lines.append('        android:fillColor="#000000"')
        if attributes.get("fill-rule") == "evenodd":
            lines.append('        android:fillType="evenOdd"')
    if stroke:
        lines.append('        android:strokeColor="#000000"')
        lines.append(f'        android:strokeWidth="{attributes.get("stroke-width", "1")}"')
        lines.append(f'        android:strokeLineCap="{attributes.get("stroke-linecap", "butt")}"')
        lines.append(f'        android:strokeLineJoin="{attributes.get("stroke-linejoin", "miter")}"')
    lines[-1] += " />"
    return ["\n".join(lines)]


def write_android(names, drawables, kotlin_file, package):
    """One vector drawable for each icon in `names`, and the list of them that the app's `AkiIcon` looks them up in."""
    drawables.mkdir(parents=True, exist_ok=True)
    for stale in drawables.glob("aki_icon_*.xml"):
        stale.unlink()
    entries = []
    for icon, (name, suffix) in sorted(names.items(), key=lambda item: item[1]):
        svg = ET.fromstring(f'<svg xmlns="http://www.w3.org/2000/svg">{icons[icon]["body"]}</svg>')
        paths = [line for child in svg for line in vector_paths(child, {})]
        resource = "aki_icon_" + (name + suffix).replace("-", "_")
        (drawables / f"{resource}.xml").write_text(
            '<?xml version="1.0" encoding="utf-8"?>\n'
            '<vector xmlns:android="http://schemas.android.com/apk/res/android"\n'
            '    android:width="24dp" android:height="24dp" android:viewportWidth="24" android:viewportHeight="24">\n'
            + "\n".join(paths) + "\n</vector>\n"
        )
        entries.append(f'    "{name}{suffix}" to R.drawable.{resource},')
    kotlin_file.write_text(
        f"package {package}\n\n"
        "// Written by content/manga/fetch-icons.py: do not edit.\n"
        "internal val akiIconSet: Map<String, Int> = mapOf(\n" + "\n".join(entries) + "\n)\n"
    )


if not only_tv:
    write_android(wanted, android / "res" / "drawable", android / "kotlin" / "live" / "akidev" / "manga" / "design" / "AkiIconSet.kt", "live.akidev.manga.design")
    print(f"{len(wanted)} Android icons written")
tv = pathlib.Path(__file__).resolve().parent.parent.parent / "apps" / "tv-android" / "app" / "src" / "main"
write_android(tv_wanted, tv / "res" / "drawable", tv / "kotlin" / "live" / "akidev" / "tv" / "AkiIconSet.kt", "live.akidev.tv")
print(f"{len(tv_wanted)} TV icons written")
