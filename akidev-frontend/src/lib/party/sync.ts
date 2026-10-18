// Keeping a guest's player in step with the host's. The same rules, and the same tests, as
// desktop/party/src/sync.rs.

import type { Choosing, Item } from "./protocol.ts";

/** A gap bigger than this is fixed with a seek; a smaller one by running a little faster or slower. */
const SEEK_ABOVE = 0.8;
/** A gap smaller than this is not worth correcting. */
const LEAVE_BELOW = 0.15;
/** How much faster or slower to run to close a small gap, which is not noticeable. */
const NUDGE = 0.05;
/**
 * However long ago the host's clock says it spoke, a guest moves its word on by no more than this
 * many seconds: a clock that is far out must not send a guest far from the picture.
 */
const MAX_LATE = 10;

/** What is playing, and how far in. */
export interface Playback {
  item: Item | null;
  playing: boolean;
  position: number;
  /** While the host has a panel open everyone waits, and the guests say so on screen. */
  choosing?: Choosing | null;
}

/** What a guest should be playing, with the position where the host was when it was received. */
export interface Target {
  playback: Playback;
  /** `performance.now()` when it was received. */
  received: number;
}

/** Where the host is now: the position it reported, moved on by the time since. */
export function positionNow(target: Target, now = performance.now()): number {
  return target.playback.playing ? target.playback.position + (now - target.received) / 1000 : target.playback.position;
}

/** What a guest does to its player to catch up: a seek, or a playback speed (1 is normal). */
export interface Fix {
  seek: number | null;
  speed: number;
}

/** How to bring a guest at `local` seconds to where the host is, `target`. */
export function fix(local: number, target: number, playing: boolean): Fix {
  const gap = local - target;
  if (!playing) return { seek: Math.abs(gap) > LEAVE_BELOW ? target : null, speed: 1 };
  if (Math.abs(gap) > SEEK_ABOVE) return { seek: target, speed: 1 };
  if (Math.abs(gap) < LEAVE_BELOW) return { seek: null, speed: 1 };
  return { seek: null, speed: gap > 0 ? 1 - NUDGE : 1 + NUDGE };
}

/** How long a reading of the host's clock is trusted. */
const READING_FOR = 60_000;
/** A round trip longer than this says nothing about the clock. */
const LONGEST_TRIP = 10_000;

/** Milliseconds on this device's steady clock, which a change of the clock (network time, wake from sleep) cannot move. */
export function steady(): number {
  return Math.round(performance.now());
}

/**
 * The difference between the host's clock and this device's steady clock, from the quickest round
 * trip in the last minute (the quickest is the least disturbed, so the closest to the truth).
 */
export class Clock {
  /** When it was read (steady clock), how long the round trip took, and the offset it gave. */
  private readings: { read: number; trip: number; offset: number }[] = [];

  /** A `pong`: sent at `sent` and received at `now` (both on the steady clock), answered at `at` (the host's clock). */
  sample(sent: number, at: number, now: number) {
    const trip = Math.max(0, now - sent);
    if (trip > LONGEST_TRIP) return;
    // The host read its clock about half way.
    this.readings.push({ read: now, trip, offset: at + Math.floor(trip / 2) - now });
    this.readings = this.readings.filter((reading) => now - reading.read <= READING_FOR);
  }

  /** Whether a `pong` has been seen, so the clocks are known. */
  get known(): boolean {
    return this.readings.length > 0;
  }

  /** The host's clock now, when this device's steady clock reads `now`. The newest of the quickest readings wins a tie. */
  hostTime(now: number): number {
    let best: { read: number; trip: number; offset: number } | null = null;
    for (const reading of this.readings) {
      if (!best || reading.trip < best.trip || (reading.trip === best.trip && reading.read > best.read)) best = reading;
    }
    return Math.max(0, now + (best?.offset ?? 0));
  }
}

/** What the host said, moved on by how long ago it said it. */
export function targetOf(
  state: { item: Item | null; playing: boolean; position: number; at: number; choosing?: Choosing | null },
  clock: Clock,
  now: number,
  received = performance.now(),
): Target {
  const late = Math.min(MAX_LATE, Math.max(0, clock.hostTime(now) - state.at) / 1000);
  return {
    playback: {
      item: state.item,
      playing: state.playing,
      position: state.playing ? state.position + late : state.position,
      choosing: state.choosing ?? null,
    },
    received,
  };
}
