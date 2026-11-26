"use client";

import { useQuery, useQueryClient } from "react-query";

import { post } from "@/lib/api";
import { canPlayHevc } from "@/lib/play";
import type { PlayResult } from "@/types/media";
import { QUERY_KEYS } from "./keys";
import { playStaleTime } from "./play-policy";

/**
 * Finds a playable source for one episode or film.
 *
 * This is the slow call - it searches torrent indexers and may join a swarm - so the page
 * renders the title and episode list from the detail query while this is still in flight.
 *
* A result with `available: false` is a normal outcome, rendered as an honest message,
 * but immediately stale so an outage does not hide newly available sources for ten minutes.
 */
export function usePlay(id: string | undefined, start: number | null = null, release?: string, skip: string[] = []) {
  const client = useQueryClient();
  const key = [QUERY_KEYS.PLAY, id, release ?? null, skip];
  const previous = client.getQueryData<PlayResult>(key);
  return useQuery(
    key,
    // `start` is where the viewer means to begin, so a conversion runs that far ahead. It is read
    // when the request is made and is not part of the key: the same episode is the same source.
    // `release` is one the viewer picked from the list; without it the server tries the best first,
    // leaving out the `skip` releases, which did not play.
    ({ signal }) => post<PlayResult>("/v1/play", { id, hevc: canPlayHevc(), start: start ?? undefined, release, skip: skip.length > 0 ? skip : undefined }, signal),
    {
      enabled: Boolean(id),
      // Finding a source is expensive on the server, so a resolved episode is kept for a
      // while and re-used when the viewer moves back and forth.
      staleTime: playStaleTime(previous),
      retry: false,
    },
  );
}
