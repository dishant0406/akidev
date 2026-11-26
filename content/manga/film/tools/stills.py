# Usage: python stills.py URL OUT_DIR t1 t2 ...   (times in seconds) -> OUT_DIR/still-<t>.png
import asyncio, sys
from playwright.async_api import async_playwright
async def main(url, out, times):
    async with async_playwright() as p:
        b = await p.chromium.launch()
        pg = await b.new_page(viewport={'width': 1440, 'height': 1440})
        pg.on('console', lambda m: print('console:', m.text))
        pg.on('pageerror', lambda e: print('pageerror:', e))
        await pg.goto(url)
        await pg.evaluate('window.__ready')
        for t in times:
            await pg.evaluate(f'window.seek({t})')
            await pg.locator('#frame').screenshot(path=f'{out}/still-{t:06.3f}.png')
            print('saved', t)
        await b.close()
asyncio.run(main(sys.argv[1], sys.argv[2], [float(x) for x in sys.argv[3:]]))
