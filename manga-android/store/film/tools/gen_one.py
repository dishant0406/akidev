import base64, json, os, sys, urllib.request
out, ratio, prompt = sys.argv[1], sys.argv[2], sys.argv[3]
body = json.dumps({"model": "bytedance-seed/seedream-5-0-flash", "prompt": prompt, "aspect_ratio": ratio, "resolution": "2K", "output_format": "png", "n": 1}).encode()
req = urllib.request.Request("https://openrouter.ai/api/v1/images", body, {"Authorization": "Bearer " + os.environ["OPENROUTER_API_KEY"], "Content-Type": "application/json"})
item = json.load(urllib.request.urlopen(req, timeout=300))["data"][0]
raw = base64.b64decode(item["b64_json"]) if item.get("b64_json") else urllib.request.urlopen(item["url"]).read()
open(out, "wb").write(raw); print("ok", out, len(raw) // 1024, "KB")
