// The two ends of a party over WebRTC: the host that holds it and the guest that follows. The
// desktop app does the same in desktop/party/src/{host,guest}.rs, and the numbers below are the same.

import { itemId, itemKey, parseMessage, type Member, type Message, type Readiness } from "./protocol.ts";
import * as signal from "./signal.ts";
import { Clock, steady, targetOf, type Playback, type Target } from "./sync.ts";

/** How long finding the way to the other device (gathering addresses, asking STUN) may take. */
const GATHER_FOR = 4_000;
/** How long the other device has to open the channel once the answer is in. */
const OPEN_WITHIN = 30_000;
/** How long a guest keeps asking for the host's answer. */
const ANSWER_WITHIN = 60_000;
/** How often the clocks are compared. The first few are quick, so a good reading comes early. */
const PING_EVERY = 5_000;
const QUICK_PINGS = 5;
/** What was just sent is given this long to leave before the connection is closed. */
const CLOSE_AFTER = 500;
/** The host says something (a state, an answer to a ping) at least this often, so a silence this long means it is not there. */
const HOST_SILENT_AFTER = 8_000;
/** A guest pings every few seconds; one silent this long is gone, whether or not the connection has noticed. */
const GUEST_SILENT_AFTER = 15_000;
/** A guest that has not said who it is within this long is not one of ours. */
const HELLO_WITHIN = 10_000;
/** What one guest may send: a burst, then this many a second. Nobody pressing buttons gets near. */
const BURST = 40;
const PER_SECOND = 20;
/** A guest that goes on sending after being told to stop is dropped once it is this far over. */
const FLOOD = 200;
/** How long the host waits between asking for guests when the server does not answer, at most. */
const LONGEST_WAIT = 10_000;
/** Not reaching the server this many times in a row is worth telling the host about. */
const UNREACHABLE_AFTER = 3;
/** What the host said about playing is not believed for a joiner after this long. */
const STALE_AFTER = 10_000;

function send(channel: RTCDataChannel, message: Message) {
  if (channel.readyState === "open") channel.send(JSON.stringify(message));
}

const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

/** Resolves once every address has been found, so the description can be sent whole. */
function gathered(pc: RTCPeerConnection): Promise<void> {
  if (pc.iceGatheringState === "complete") return Promise.resolve();
  return new Promise((resolve) => {
    const finish = () => {
      pc.removeEventListener("icegatheringstatechange", check);
      clearTimeout(timer);
      resolve();
    };
    const check = () => pc.iceGatheringState === "complete" && finish();
    // Gathering that has not finished by now is sent as it is: it has the addresses found.
    const timer = setTimeout(finish, GATHER_FOR);
    pc.addEventListener("icegatheringstatechange", check);
  });
}

/** A published state moved on to now: a playing one has been playing since it was said. */
function asOfNow(state: Message): Message {
  if (state.t !== "state" || !state.playing) return state;
  const now = Date.now();
  const late = Math.max(0, now - state.at);
  // A host that said "playing" and has been silent this long is not playing any more: a joiner is not sent into the film on its word.
  if (late > STALE_AFTER) return { ...state, playing: false, at: now };
  return { ...state, position: state.position + late / 1000, at: now };
}

/** A name from a guest is shown to everyone, so it is kept short and plain: no control, format or invisible characters, cut by whole characters. */
export function cleanName(name: string): string {
  const clean = [...name.replace(/[\p{Cc}\p{Cf}\p{Zl}\p{Zp}]/gu, "")].slice(0, 24).join("").trim();
  return clean || "Guest";
}

/** A random id for this page, the same every time it joins, so a host that sees it again drops the old connection. */
const SESSION = [...crypto.getRandomValues(new Uint8Array(8))].map((byte) => byte.toString(16).padStart(2, "0")).join("");

/** What one guest may send: a burst, then so many a second. */
class Allowance {
  private tokens = BURST;
  private at = performance.now();
  private over = 0;

  /** "fine", "skip" (drop this message) or "flood" (drop the guest). */
  take(): "fine" | "skip" | "flood" {
    const now = performance.now();
    this.tokens = Math.min(BURST, this.tokens + ((now - this.at) / 1000) * PER_SECOND);
    this.at = now;
    if (this.tokens >= 1) {
      this.tokens -= 1;
      this.over = 0;
      return "fine";
    }
    return ++this.over > FLOOD ? "flood" : "skip";
  }
}

export type Request = Extract<Message, { t: "play" | "pause" | "seek" }>;

export interface HostEvents {
  /** Someone joined or left, or how someone is doing with what is playing changed. */
  members: (members: Member[]) => void;
  /** A guest asked for something. The host does it, as if a person at the host had. */
  request: (request: Request) => void;
  /** The party server cannot be reached (false when it can again), or will not have the party: no one new can join meanwhile. */
  reachable: (reachable: boolean) => void;
}

export interface HostParty {
  code: string;
  /** The host's secret, kept so a reloaded page can open the same party again. */
  token: string;
  /** What is playing now. Called when it changes and every couple of seconds while it plays. */
  publish: (playback: Playback) => void;
  /** Ends the party: guests are told, and the party is closed on the server. */
  end: () => void;
}

interface GuestSlot {
  channel: RTCDataChannel;
  name: string;
  /** The guest's own id for this run of the page, when it has one. */
  session: string | null;
  /** What the guest last said about how it is doing, and about which item. */
  state: Readiness;
  about: string | null;
  /** Lets this guest go. */
  drop: () => void;
}

/**
 * How a guest is doing with what the host is playing now. What it said about an earlier item says
 * nothing about this one: it is still getting to it.
 */
export function guestState(said: Readiness, about: string | null, playing: string | null, plain: string | null = playing): Readiness {
  // A device from before releases were shared names the plain id, which is enough.
  return playing !== null && about !== playing && about !== plain ? "loading" : said;
}

/**
 * Opens a party, or (with `resume`, the code and token of one this page held before) opens that one
 * again. `name` is how the host appears to the guests.
 */
export async function openParty(name: string, on: HostEvents, resume?: { code: string; token: string }): Promise<HostParty> {
  const { code, token } = resume ? await reopen(resume) : await signal.create();
  const stop = new AbortController();
  const guests = new Map<string, GuestSlot>();
  const connections = new Set<RTCPeerConnection>();
  /** What was last published, so a guest who joins late is told at once. */
  let last: Message | null = null;
  let ended = false;

  const broadcast = (message: Message) => guests.forEach((slot) => send(slot.channel, message));
  const shareMembers = () => {
    const playing = last?.t === "state" ? itemKey(last.item) : null;
    const plain = last?.t === "state" ? itemId(last.item) : null;
    const members: Member[] = [
      { name, host: true, state: "ready" },
      ...[...guests.values()].map((slot): Member => ({ name: slot.name, host: false, state: guestState(slot.state, slot.about, playing, plain) })),
    ];
    broadcast({ t: "members", members });
    on.members(members);
  };

  /** Answers one guest's offer and looks after that guest until they are gone. */
  const accept = async (guest: string, offer: string) => {
    let pc: RTCPeerConnection | null = null;
    let finished = false;
    let watch: ReturnType<typeof setInterval> | undefined;
    const finish = () => {
      if (finished) return;
      finished = true;
      clearInterval(watch);
      if (pc) {
        connections.delete(pc);
        pc.close();
      }
      signal.guestGone(code, token, guest);
      if (guests.delete(guest)) shareMembers();
    };
    const connect = (iceServers: signal.IceServer[]): RTCPeerConnection => {
      const connection = new RTCPeerConnection({ iceServers });
      connections.add(connection);
      connection.onconnectionstatechange = () => {
        if (connection.connectionState === "failed" || connection.connectionState === "closed") finish();
      };
      connection.ondatachannel = ({ channel }) => listen(channel);
      return connection;
    };
    const listen = (channel: RTCDataChannel) => {
      const allowance = new Allowance();
      let heard = performance.now();
      let introduced = false;
      channel.onopen = () => {
        if (ended || finished) return finish();
        guests.set(guest, { channel, name: "Guest", session: null, state: "ready", about: null, drop: finish });
        if (last) send(channel, asOfNow(last));
        shareMembers();
        watch = setInterval(() => {
          if (performance.now() - heard > (introduced ? GUEST_SILENT_AFTER : HELLO_WITHIN)) finish();
        }, 1000);
      };
      channel.onclose = finish;
      channel.onmessage = (event) => {
        const message = typeof event.data === "string" ? parseMessage(event.data) : null;
        const slot = guests.get(guest);
        if (!message || !slot) return;
        heard = performance.now();
        const verdict = allowance.take();
        if (verdict === "flood") return finish();
        if (verdict === "skip") return;
        switch (message.t) {
          case "hello":
            introduced = true;
            // The same guest on a new connection: the old one is let go.
            if (message.id) guests.forEach((other, id) => id !== guest && other.session === message.id && other.drop());
            slot.name = cleanName(message.name);
            slot.session = message.id ?? null;
            shareMembers();
            break;
          case "status":
            slot.state = message.state;
            slot.about = message.id;
            shareMembers();
            break;
          case "ping":
            send(channel, { t: "pong", id: message.id, sent: message.sent, at: Date.now() });
            break;
          case "play":
          case "pause":
            on.request(message);
            break;
          case "seek":
            on.request({ t: "seek", position: Math.max(0, message.position) });
            break;
          case "bye":
            finish();
            break;
        }
      };
    };
    try {
      // Asked for each time: the relay's login lasts a few hours, and a party can last longer.
      pc = connect(await signal.ice(code));
      await pc.setRemoteDescription({ type: "offer", sdp: offer });
      await pc.setLocalDescription(await pc.createAnswer());
      await gathered(pc);
      await signal.answer(code, token, guest, pc.localDescription?.sdp ?? "");
      setTimeout(() => guests.has(guest) || finish(), OPEN_WITHIN);
    } catch {
      finish();
    }
  };

  /**
   * Asks the server for guests, one after another, and connects each as it comes. It goes on for as
   * long as the party does: a server that is restarting or a network that is down is waited out, and
   * a party the server forgot is opened again under its code.
   */
  const listen = async () => {
    let failures = 0;
    let told = false;
    while (!stop.signal.aborted) {
      try {
        const offers = await signal.offers(code, token, [...guests.keys()], stop.signal);
        failures = 0;
        if (told) on.reachable(true);
        told = false;
        offers.forEach((entry) => void accept(entry.guest, entry.offer));
        continue;
      } catch (error) {
        if (stop.signal.aborted) return;
        if (error instanceof signal.Gone) {
          try {
            if (await signal.register(code, token, stop.signal)) continue;
            // The code is someone else's now: no one new can join this party.
            return on.reachable(false);
          } catch {
            // Could not say so either: waited out like any other failure.
          }
        }
      }
      if (++failures >= UNREACHABLE_AFTER && !told) {
        told = true;
        on.reachable(false);
      }
      await sleep(Math.min(LONGEST_WAIT, 2000 * 2 ** Math.min(failures - 1, 4)));
    }
  };
  void listen();

  return {
    code,
    token,
    publish: (playback) => {
      const before = last?.t === "state" ? itemKey(last.item) : null;
      last = {
        t: "state",
        item: playback.item,
        playing: playback.playing,
        position: playback.position,
        at: Date.now(),
        ...(playback.choosing ? { choosing: playback.choosing } : {}),
      };
      broadcast(last);
      // A guest is loading whatever the host starts until it says it is ready for it, so a new
      // item changes who is.
      if (before !== itemKey(playback.item)) shareMembers();
    },
    end: () => {
      ended = true;
      stop.abort();
      broadcast({ t: "bye" });
      signal.end(code, token);
      guests.clear();
      // Closing at once can lose the goodbye, so the guests would only find out when their
      // connection timed out.
      setTimeout(() => connections.forEach((pc) => pc.close()), CLOSE_AFTER);
    },
  };
}

/** Opens a party this page held before, under its code. */
async function reopen(held: { code: string; token: string }): Promise<{ code: string; token: string }> {
  if (!(await signal.register(held.code, held.token))) throw new signal.Gone("That party is over.");
  return held;
}

export interface GuestEvents {
  /** What the party should be playing now. Sent when the host changes it and every few seconds. */
  target: (target: Target) => void;
  members: (members: Member[]) => void;
  /** Nothing has come from the host for a while (true), or it is heard again (false). */
  silent: (silent: boolean) => void;
  /** The host ended the party. */
  ended: () => void;
  /** The connection to the host is gone without a goodbye (a network change, the host's device asleep or its page reloaded): the party may still be there. */
  lost: () => void;
}

export interface GuestParty {
  /** Asks the host to play, pause or seek. The host does it, and everyone, this device
   *  included, hears what it is now. */
  ask: (request: Request) => void;
  /** Tells the host how this device is doing with the item whose id is `id`. */
  status: (state: Readiness, id: string | null) => void;
  leave: () => void;
}

/** Joins the party `code`. `name` is how this device appears to the others. `cancel` stops a join that is still being made. */
export async function joinParty(code: string, name: string, on: GuestEvents, cancel?: AbortSignal): Promise<GuestParty> {
  code = code.trim().toUpperCase();
  const stopped = () => {
    if (cancel?.aborted) throw new Error("Cancelled.");
  };
  const ice = await signal.ice(code);
  stopped();
  const pc = new RTCPeerConnection({ iceServers: ice });
  const channel = pc.createDataChannel("party");
  const clock = new Clock();
  let over = false;
  const timers: ReturnType<typeof setInterval>[] = [];
  /** Lets go of the party. What happened is told, unless it was this device that left. */
  const end = (outcome: "ended" | "lost" | "left") => {
    if (over) return;
    over = true;
    timers.forEach(clearInterval);
    // A goodbye sent just before is on its way: closing at once could lose it.
    if (outcome === "left") setTimeout(() => pc.close(), CLOSE_AFTER);
    else pc.close();
    if (outcome === "ended") on.ended();
    else if (outcome === "lost") on.lost();
  };

  try {
    await pc.setLocalDescription(await pc.createOffer());
    await gathered(pc);
    stopped();
    const guest = await signal.join(code, pc.localDescription?.sdp ?? "");
    let answer: string | null = null;
    for (const started = Date.now(); !answer; ) {
      stopped();
      if (Date.now() - started > ANSWER_WITHIN) throw new Error("The host did not answer.");
      answer = await signal.takeAnswer(code, guest);
    }
    stopped();
    await pc.setRemoteDescription({ type: "answer", sdp: answer });
    await new Promise<void>((resolve, reject) => {
      const timer = setTimeout(() => reject(new Error("The host did not connect.")), OPEN_WITHIN);
      cancel?.addEventListener("abort", () => reject(new Error("Cancelled.")), { once: true });
      channel.onopen = () => {
        clearTimeout(timer);
        resolve();
      };
      channel.onclose = () => reject(new Error("The connection closed."));
    });
  } catch (error) {
    over = true;
    pc.close();
    throw error;
  }

  let heard = steady();
  let silent = false;
  let goodbye = false;
  channel.onclose = () => end(goodbye ? "ended" : "lost");
  pc.onconnectionstatechange = () => {
    if (pc.connectionState === "failed" || pc.connectionState === "closed") end(goodbye ? "ended" : "lost");
  };
  // What the host said before the clocks were known is kept, and acted on at the first reading:
  // without one, how long ago it was said cannot be told.
  let waiting: Extract<Message, { t: "state" }> | null = null;
  channel.onmessage = (event) => {
    const message = typeof event.data === "string" ? parseMessage(event.data) : null;
    if (!message) return;
    heard = steady();
    if (silent) {
      silent = false;
      on.silent(false);
    }
    if (message.t === "state") {
      if (clock.known) on.target(targetOf(message, clock, steady()));
      else waiting = message;
    } else if (message.t === "members") on.members(message.members);
    else if (message.t === "pong") {
      clock.sample(message.sent, message.at, steady());
      if (waiting) on.target(targetOf(waiting, clock, steady()));
      waiting = null;
    } else if (message.t === "bye") {
      goodbye = true;
      end("ended");
    }
  };
  send(channel, { t: "hello", name: cleanName(name), id: SESSION });
  let pings = 0;
  const ping = () => send(channel, { t: "ping", id: pings++, sent: steady() });
  ping();
  // The first few are quick, so a good reading comes early.
  const quick = setInterval(() => {
    ping();
    if (pings >= QUICK_PINGS) clearInterval(quick);
  }, 400);
  const watchHost = setInterval(() => {
    if (!silent && steady() - heard > HOST_SILENT_AFTER) {
      silent = true;
      on.silent(true);
    }
  }, 1000);
  timers.push(quick, setInterval(ping, PING_EVERY), watchHost);

  return {
    ask: (request) => send(channel, request),
    status: (state, id) => send(channel, { t: "status", state, id }),
    leave: () => {
      send(channel, { t: "bye" });
      end("left");
    },
  };
}
