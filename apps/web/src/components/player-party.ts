"use client";

import { useCallback, useEffect, useRef, useState, type RefObject } from "react";

import { itemKey, type Choosing } from "@/lib/party/protocol";
import { fix, positionNow } from "@/lib/party/sync";
import { useParty } from "@/providers/party";

/** A host waits at least this long, and at most the longer one, for guests that are still loading. */
const HOLD_AT_LEAST = 1000;
const HOLD_AT_MOST = 45_000;
/** `readyState` at which there is enough picture ahead to be moving. */
const HAVE_FUTURE_DATA = 3;
/** How often a host says where the picture is, and how often a guest checks it is in step. */
const PARTY_BEAT = 2000;
const PARTY_CHECK = 1000;
/** A guest does not seek again this soon after a seek: a conversion that has not got there yet would be asked again and again. */
const SEEK_COOLDOWN = 2000;

/**
 * What a watch party asks of the picture: a host says where it is and waits for guests that are
 * still loading, and a guest stays where the host is. The player gives the element and what the
 * host has open; the rest is here so the player itself stays about playing.
 */
export function usePartyPlayback({
  video,
  mediaId,
  releaseId,
  startAt,
  choosing,
}: {
  video: RefObject<HTMLVideoElement>;
  mediaId: string;
  releaseId: string | null;
  startAt: number | null;
  choosing: Choosing | null;
}) {
  const party = useParty();
  // The party as the timers and handlers below see it, so they need not be rebuilt each time it changes.
  const now = useRef(party);
  now.current = party;
  const choosingNow = useRef(choosing);
  choosingNow.current = choosing;

  /** When a host began waiting for guests that are still loading, or null when it is not waiting. */
  const [heldSince, setHeldSince] = useState<number | null>(null);
  const heldNow = useRef<number | null>(null);
  heldNow.current = heldSince;
  /** The host's picture was paused because a panel opened, so it starts again when the panel closes. */
  const panelPaused = useRef(false);
  /** The browser refused to start the picture without a tap, so the guest is asked for one. */
  const [blocked, setBlocked] = useState(false);
  const lastSeek = useRef(0);

  /** How a guest names what it is ready for: the host's item, release included. */
  const statusKey = () => {
    const item = now.current.item;
    return item?.kind === "video" && item.id === mediaId ? (itemKey(item) ?? mediaId) : mediaId;
  };
  const report = useCallback((state: "ready" | "loading") => now.current.setStatus(state, statusKey()), []); // eslint-disable-line react-hooks/exhaustive-deps

  const publish = useCallback(
    (heartbeat: boolean) => {
      const element = video.current;
      if (now.current.role !== "host" || !element) return;
      // Before the picture has loaded there is no position to give: the host says where it will start, so a party is not sent back to the start.
      if (element.readyState < 1) {
        now.current.publish({ item: { kind: "video", id: mediaId, release: releaseId }, playing: false, position: startAt ?? 0, choosing: "finding" }, heartbeat);
        return;
      }
      // Playing means the picture is moving: a host that is still loading or buffering is not.
      const moving = !element.paused && element.readyState >= HAVE_FUTURE_DATA;
      now.current.publish(
        {
          item: { kind: "video", id: mediaId, release: releaseId },
          playing: moving,
          position: element.currentTime,
          choosing: choosingNow.current,
        },
        heartbeat,
      );
    },
    [video, mediaId, releaseId, startAt],
  );
  useEffect(() => publish(false), [publish, party.role]);

  // Hosting: a panel open pauses the picture for everyone while the host chooses, and closing it
  // carries on, even if the guests have gone in the meantime.
  useEffect(() => {
    const element = video.current;
    if (element) {
      if (choosing && !panelPaused.current && !element.paused && now.current.role === "host" && now.current.members.length > 1) {
        element.pause();
        panelPaused.current = true;
      } else if (!choosing && panelPaused.current) {
        panelPaused.current = false;
        // A new release holds the start itself, until the guests have it.
        if (heldNow.current === null) element.play().catch(() => {});
      }
    }
    publish(false);
  }, [choosing]); // eslint-disable-line react-hooks/exhaustive-deps

  // Hosting: the position is said every couple of seconds, paused too, so a guest can tell a host that is still there from one that is gone.
  useEffect(() => {
    if (party.role !== "host") return undefined;
    const timer = setInterval(() => publish(true), PARTY_BEAT);
    return () => clearInterval(timer);
  }, [party.role, publish]);

  // Hosting: starts when the guests are ready, or after a while whether they are or not.
  const { role, waitingFor } = party;
  useEffect(() => {
    if (heldSince === null) return undefined;
    const release = () => {
      setHeldSince(null);
      video.current?.play().catch(() => {});
    };
    const wait = role !== "host" ? 0 : Math.max(0, (waitingFor.length === 0 ? heldSince + HOLD_AT_LEAST : heldSince + HOLD_AT_MOST) - Date.now());
    const timer = setTimeout(release, wait);
    return () => clearTimeout(timer);
  }, [heldSince, role, waitingFor, video]);

  // Hosting: what guests ask for, while this is what the party is playing and the host is not choosing.
  useEffect(
    () =>
      party.onRequest((request) => {
        const element = video.current;
        const item = now.current.item;
        if (!element || item?.kind !== "video" || item.id !== mediaId || choosingNow.current) return;
        if (request.t === "play") element.play().catch(() => {});
        else if (request.t === "pause") element.pause();
        else {
          // Not to the very end: that would finish the film, and start the next episode, for everyone.
          const last = Number.isFinite(element.duration) ? Math.max(0, element.duration - 1) : request.position;
          element.currentTime = Math.min(request.position, last);
        }
      }),
    [party.onRequest, mediaId, video], // eslint-disable-line react-hooks/exhaustive-deps
  );

  // A host who leaves the film tells the party nothing is playing, so guests do not carry on without it.
  useEffect(
    () => () => {
      const last = now.current;
      if (last.role === "host" && last.item?.kind === "video" && last.item.id === mediaId) {
        last.publish({ item: null, playing: false, position: 0 });
      }
    },
    [mediaId],
  );

  // Following: be where the host is, playing or paused, a little faster or slower to close a small gap.
  useEffect(() => {
    if (party.role !== "guest") return undefined;
    const element = video.current;
    const follow = () => {
      const target = now.current.target;
      // Until the picture has loaded there is no position to correct, and a seek under way is left to finish.
      if (!element || !target || element.readyState < 1 || element.seeking || (element.ended && positionNow(target) >= element.duration - 1)) return;
      const item = target.playback.item;
      // The host has stopped, or has a panel open: everyone waits, paused, until it is done.
      if (!item || target.playback.choosing) {
        if (!element.paused) element.pause();
        return;
      }
      // What else the party is playing is another page's to show.
      if (item.kind !== "video" || item.id !== mediaId) return;
      const change = fix(element.currentTime, positionNow(target), target.playback.playing);
      if (change.seek !== null && performance.now() - lastSeek.current > SEEK_COOLDOWN) {
        element.currentTime = change.seek;
        lastSeek.current = performance.now();
      }
      element.playbackRate = change.speed;
      if (target.playback.playing && element.paused) {
        element.play().then(
          () => setBlocked(false),
          (failure: unknown) => {
            if (failure instanceof DOMException && failure.name === "NotAllowedError") setBlocked(true);
          },
        );
      } else {
        setBlocked(false);
        if (!target.playback.playing && !element.paused) element.pause();
      }
    };
    follow();
    const timer = setInterval(follow, PARTY_CHECK);
    // A tab that was in the background has been throttled: catch up the moment it is back.
    const shown = () => {
      if (document.visibilityState === "visible") follow();
    };
    document.addEventListener("visibilitychange", shown);
    return () => {
      clearInterval(timer);
      document.removeEventListener("visibilitychange", shown);
      if (element) element.playbackRate = 1;
    };
  }, [party.role, party.target, mediaId, video]);

  /** The tap a browser wanted before it would play for a guest. */
  const tapToPlay = useCallback(() => {
    setBlocked(false);
    video.current?.play().catch(() => setBlocked(true));
  }, [video]);

  /** Begins a host's wait for the guests that are still loading. */
  const hold = useCallback(() => setHeldSince(Date.now()), []);

  return {
    publish,
    report,
    hold,
    held: heldSince !== null,
    blocked: blocked && party.role === "guest",
    tapToPlay,
    /** What the host has open, for a guest to say so. */
    hostChoosing: party.role === "guest" ? (party.target?.playback.choosing ?? null) : null,
  };
}
