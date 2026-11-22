# Generates the film's original art with Seedream 5.0 Flash on OpenRouter.
# Usage: python3 gen.py OUT_DIR [name ...]   (no names = all; existing files are skipped)
import base64, json, os, sys, urllib.request
from concurrent.futures import ThreadPoolExecutor

STYLE = ("Original manga / manhwa cover illustration, portrait 2:3, high detail, clean professional cel shading "
         "with soft painted lighting, warm colour palette, strong single focal character or subject, clear silhouette, "
         "simple uncluttered background, cinematic composition. Completely original characters and world, not based on "
         "any existing manga, anime, manhwa, game or film. No text, no title, no logo, no letters, no watermark, "
         "no signature, no speech bubbles. Avoid large areas of bright yellow.")

COVERS = [
 "A young woman lighthouse keeper with short copper hair and a long rust-red coat stands on a rocky cliff at dusk, holding an old brass lantern that glows warm orange. Behind her a tall white lighthouse and a huge calm violet-and-peach sky. Lower third of the image fades into deep plum shadow. Upper third is mostly open sky. Palette: plum, peach, rust, warm orange.",
 "A boy courier on a small red scooter racing through a narrow hillside town of terracotta roofs and hanging laundry, a paper parcel strapped to the back, motion in his scarf. Morning light. Palette: terracotta, cream, sky blue, red.",
 "A calm young swordswoman in a dark teal kimono-style jacket sits on a wooden bridge over a koi pond in autumn, her sheathed sword resting across her knees, red maple leaves falling. Palette: teal, maple red, warm brown, soft gold.",
 "Two friends, a tall girl with a sketchbook and a shorter boy with headphones, sit on a rooftop at sunset above a warm city, sharing a single umbrella against light rain. Palette: coral, dusty pink, warm grey, soft orange.",
 "A young alchemist with round glasses in a cluttered attic workshop, glass bottles of glowing green and amber liquid, a small fox familiar sitting on a stack of old books. Warm lamp light. Palette: forest green, amber, walnut brown.",
 "A knight girl in light silver armour with a white cape stands in a field of tall wheat, a large friendly gentle dragon with scales the colour of sand resting its head beside her. Golden-hour light. Palette: sand, wheat, sky blue, silver.",
 "A street chef boy with a headband tossing noodles in a wok at a small night food stall, steam rising, paper lanterns overhead in red and orange. Palette: deep red, orange, charcoal, warm white.",
 "A girl astronomer in a thick knitted sweater on a snowy mountain observatory deck, looking through a big brass telescope, a deep blue night sky with soft warm stars. Palette: navy, warm cream, brass, soft rose.",
 "A quiet young librarian with long dark hair and a moss-green cardigan climbs a rolling ladder in a tall round library, books floating gently around her like birds. Palette: moss green, mahogany, warm cream, dusty blue.",
 "A boxer girl with clean white hand wraps and a short black ponytail stands in a small old gym, sunlight through dusty windows, a worn red punching bag behind her, calm determined expression. Plain red tank top with no print. Clean and wholesome, no blood, no injuries. Palette: brick red, warm grey, denim blue, peach.",
]

STRIP = ("One panel of an original full-colour vertical webtoon (manhwa), clean digital cel shading, warm palette, "
         "completely original characters not based on any existing work. Main character: a young woman lighthouse keeper "
         "with short copper hair and a long rust-red coat. No text, no letters, no sound effects, no watermark, no panel borders. ")
PANELS = [
 "Wide establishing shot of a white lighthouse on a cliff at dusk, violet and peach sky, calm sea.",
 "Medium shot of her climbing a spiral staircase inside the lighthouse, holding a glowing brass lantern.",
 "Close-up of her face, surprised, wind in her hair, warm orange light on her face. Include ONE large empty white oval speech bubble with a tail pointing to her, taking about 40 percent of the image, in the upper part. The bubble must be completely empty, with nothing written in it.",
 "Wide shot from behind her at the top of the lighthouse, looking out at a ship's light far away on the dark sea.",
]

PAGE = ("Original black-and-white Japanese manga page, traditional screentone shading, clean ink lines, white gutters, "
        "completely original characters not based on any existing work. No text, no letters, no sound effects, no watermark. ")
PAGES = {
 "spread-right": "Page 1 of a two-page spread read right to left. Main character: a young woman lighthouse keeper with short messy bob hair, a long double-breasted coat with a belt, tall boots, carrying an old brass lantern. 4 panels in classic right-to-left manga layout: (1) top-right, a wide shot of a lighthouse on a rocky cliff at night, waves below; (2) top-left, she pushes open the heavy lighthouse door against the wind; (3) middle, a tall panel of her on the gallery at the top, lantern raised, looking out to sea; (4) bottom, close-up of her eyes, narrowed against the wind. One empty white speech bubble in panel 3, with nothing written in it.",
 "spread-left": "Page 2 of the spread, the same lighthouse keeper with a short messy bob, long belted coat and brass lantern, continuing the scene, read right to left. 3 panels: (1) top, wide panel: far out on the dark sea a small fishing boat with one light; (2) middle-right, her face breaking into a relieved smile; (3) large bottom panel: she swings the lantern in a wide arc from the gallery railing, the beam crossing the waves. One empty white speech bubble in panel 2, with nothing written in it.",
}

JOBS = {f"cover-{i+1:02d}": (STYLE + " " + p, "2:3") for i, p in enumerate(COVERS)}
JOBS |= {f"strip-{i+1}": (STRIP + p, "4:5") for i, p in enumerate(PANELS)}
JOBS |= {k: (PAGE + p, "2:3") for k, p in PAGES.items()}

def run(name, out):
    path = os.path.join(out, name + ".png")
    if os.path.exists(path):
        return f"skip {name}"
    prompt, ratio = JOBS[name]
    body = json.dumps({"model": "bytedance-seed/seedream-5-0-flash", "prompt": prompt, "aspect_ratio": ratio,
                       "resolution": "2K", "output_format": "png", "n": 1}).encode()
    req = urllib.request.Request("https://openrouter.ai/api/v1/images", body, {
        "Authorization": "Bearer " + os.environ["OPENROUTER_API_KEY"], "Content-Type": "application/json"})
    try:
        with urllib.request.urlopen(req, timeout=300) as r:
            data = json.load(r)
    except urllib.error.HTTPError as e:
        return f"FAIL {name}: {e.code} {e.read()[:400]!r}"
    item = data["data"][0]
    if item.get("b64_json"):
        raw = base64.b64decode(item["b64_json"])
    else:
        raw = urllib.request.urlopen(item["url"], timeout=120).read()
    with open(path, "wb") as f:
        f.write(raw)
    return f"ok {name} {len(raw)//1024} KB"

out = sys.argv[1]
names = sys.argv[2:] or list(JOBS)
with ThreadPoolExecutor(4) as pool:
    for line in pool.map(lambda n: run(n, out), names):
        print(line, flush=True)
