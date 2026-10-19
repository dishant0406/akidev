import { ROUTES } from "../constants/routes.ts";

/**
 * The pages this tab has gone through, newest last, so Back can tell where it would land. Kept in
 * the tab's session storage, so a reload does not forget it.
 */
const KEY = "akidev.trail";
const MOST = 50;

type Trail = { pages: string[]; replacing?: boolean };

function read(): Trail {
  try {
    const trail = JSON.parse(sessionStorage.getItem(KEY) ?? "null");
    if (Array.isArray(trail?.pages)) return trail;
  } catch {
    // Storage is blocked or holds something else: start again.
  }
  return { pages: [] };
}

function write(trail: Trail) {
  try {
    sessionStorage.setItem(KEY, JSON.stringify(trail));
  } catch {
    // Storage is blocked: Back falls back to going home.
  }
}

/** Called on every page change. */
export function visit(path: string) {
  const { pages, replacing } = read();
  const last = pages.length - 1;
  const inPlayer = path.startsWith(ROUTES.WATCH) && pages[last]?.startsWith(ROUTES.WATCH);
  if (pages[last] === path) {
    // A reload.
  } else if (pages[last - 1] === path && !replacing) {
    // Back.
    pages.pop();
  } else if ((replacing || inPlayer) && last >= 0) {
    // A page that takes the place of the last one, as a swap of episodes in the player does.
    pages[last] = path;
  } else {
    pages.push(path);
    if (pages.length > MOST) pages.shift();
  }
  write({ pages });
}

/** To be called just before a page is replaced by another, which adds no step to the history. */
export function willReplace() {
  write({ ...read(), replacing: true });
}

/** The page this one was opened from, or null when it was the first one in this tab. */
export function cameFrom(): string | null {
  const { pages } = read();
  return pages[pages.length - 2] ?? null;
}
