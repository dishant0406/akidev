#!/usr/bin/env python3
"""Downloads the Solar icons the app uses into App/Assets.xcassets.

Solar is by 480 Design, licensed CC BY 4.0 (credit is given in Settings > About). Each name below is what the app
calls an icon; `name` is the outline version and `name-solid` the bold one. Run it again after adding a name.
"""
import json
import pathlib
import shutil
import subprocess

ICONS = {
    "home": "home-2", "library": "library", "inbox": "inbox", "sources": "widget-2", "settings": "settings",
    "add": "add-circle", "back": "alt-arrow-left", "down": "alt-arrow-down", "forward": "alt-arrow-right",
    "refresh": "refresh", "restore": "restart", "download": "download-minimalistic", "share": "share",
    "trash": "trash-bin-minimalistic", "folder": "folder", "folder-add": "add-folder", "filter": "filter",
    "order": "sort-from-top-to-bottom", "sliders": "tuning-2", "search": "magnifier", "bookmark": "bookmark",
    "bookmark-off": "bookmark-off", "check": "check-circle", "tick": "check", "clock": "clock-circle",
    "more": "menu-dots", "play": "play", "previous": "skip-previous", "next": "skip-next", "keyboard": "keyboard",
    "server": "server-2", "sparkles": "stars-minimalistic", "list": "list", "warning": "danger-triangle",
    "swap": "transfer-horizontal", "close": "close-circle", "zip": "file-zip", "image": "gallery",
}
STYLES = {"": "linear", "-solid": "bold"}
root = pathlib.Path(__file__).resolve().parent.parent / "App" / "Assets.xcassets"
root.mkdir(exist_ok=True)
(root / "Contents.json").write_text(json.dumps({"info": {"author": "xcode", "version": 1}}, indent=2))

for stale in root.glob("solar-*.imageset"):
    shutil.rmtree(stale)

wanted = {f"{solar}-{style}": (name, suffix) for name, solar in ICONS.items() for suffix, style in STYLES.items()}
# One request for every icon: the service limits how often it is asked.
url = "https://api.iconify.design/solar.json?icons=" + ",".join(wanted)
icons = json.loads(subprocess.run(["curl", "-fsSL", url], capture_output=True, text=True, check=True).stdout)["icons"]
missing = [icon for icon in wanted if icon not in icons]
if missing:
    raise SystemExit(f"Solar has no icon called: {', '.join(missing)}")

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
