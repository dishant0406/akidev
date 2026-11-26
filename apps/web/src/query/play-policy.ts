import type { PlayResult } from "../types/media.ts";

/** Keep playable URLs stable, but let a remount/focus retry a transient negative result. */
export function playStaleTime(result: PlayResult | undefined): number {
  return result?.available === false ? 0 : 10 * 60 * 1000;
}
