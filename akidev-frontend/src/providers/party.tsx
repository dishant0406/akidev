"use client";

import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState } from "react";

import { joinParty, openParty, type GuestEvents, type GuestParty, type HostEvents, type HostParty, type Request } from "@/lib/party/connection";
import { partyCodeFrom } from "@/lib/party/code";
import * as signal from "@/lib/party/signal";
import { sameItem, type Item, type Member, type Readiness } from "@/lib/party/protocol";
import type { Playback, Target } from "@/lib/party/sync";

type Role = "none" | "host" | "guest";

interface Party {
  /** Hosting a party, following someone else's, or neither. */
  role: Role;
  connecting: boolean;
  code: string | null;
  members: Member[];
  /** Why the last attempt to start or join failed, or the party stopped working. */
  error: string | null;
  /** What the party is playing: what the host last published, or what the host last said. */
  item: Item | null;
  /** A guest's latest word from the host. */
  target: Target | null;
  /** A guest lost the connection and is trying to get back in. */
  reconnecting: boolean;
  /** A guest has heard nothing from the host for a while. */
  hostSilent: boolean;
  /** The code in the address the page was opened with, for the panel to offer to join. */
  invite: string | null;
  name: string;
  setName: (name: string) => void;
  create: () => Promise<void>;
  join: (code: string) => Promise<void>;
  /** Stops a start or join that is still being made. */
  cancel: () => void;
  leave: () => void;
  /** The host says what is playing: when it changes, and every couple of seconds while it plays.
   *  A `heartbeat` from something that is not what is playing is ignored. */
  publish: (playback: Playback, heartbeat?: boolean) => void;
  /** A guest asks the host to do something. */
  ask: (request: Request) => void;
  /** A guest says how it is doing with the item whose key is `id`: ready, still loading, or unable. */
  setStatus: (state: Readiness, id: string | null) => void;
  /** The names of the guests who are still loading what the host is playing, for the host. */
  waitingFor: string[];
  /** The host is told what guests ask for. Returns what stops it. */
  onRequest: (handler: (request: Request) => void) => () => void;
}

const Context = createContext<Party | null>(null);

export function useParty(): Party {
  const party = useContext(Context);
  if (!party) throw new Error("useParty needs PartyProvider");
  return party;
}

const NAME_KEY = "akidev:party-name";
/** The code and token of the party this tab hosts, kept for the length of the tab so a reload opens the same party again. */
const HELD_KEY = "akidev:party-held";
/** How long a guest keeps trying to get back into a party it lost. */
const RECONNECT_FOR = 2 * 60_000;
const RECONNECT_EVERY = 3_000;

function rememberedName(): string {
  try {
    const saved = localStorage.getItem(NAME_KEY);
    if (saved) return saved;
  } catch {
    // Storage is blocked: a name for this visit will do.
  }
  return `Guest ${Math.floor(1000 + Math.random() * 9000)}`;
}

function heldParty(): { code: string; token: string } | null {
  try {
    const held: unknown = JSON.parse(sessionStorage.getItem(HELD_KEY) ?? "null");
    if (held && typeof held === "object" && "code" in held && "token" in held && typeof held.code === "string" && typeof held.token === "string") {
      return { code: held.code, token: held.token };
    }
  } catch {
    // Nothing kept, or storage is blocked: no party to reopen.
  }
  return null;
}

function keepParty(held: { code: string; token: string } | null) {
  try {
    if (held) sessionStorage.setItem(HELD_KEY, JSON.stringify(held));
    else sessionStorage.removeItem(HELD_KEY);
  } catch {
    // Not kept: a reload ends the party for the page.
  }
}

export default function PartyProvider({ children }: { children: React.ReactNode }) {
  const [role, setRole] = useState<Role>("none");
  const [connecting, setConnecting] = useState(false);
  const [code, setCode] = useState<string | null>(null);
  const [members, setMembers] = useState<Member[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [item, setItem] = useState<Item | null>(null);
  const [target, setTarget] = useState<Target | null>(null);
  const [reconnecting, setReconnecting] = useState(false);
  const [hostSilent, setHostSilent] = useState(false);
  const [invite, setInvite] = useState<string | null>(null);
  const [name, setNameState] = useState("Guest");

  const host = useRef<HostParty | null>(null);
  const guest = useRef<GuestParty | null>(null);
  const handlers = useRef(new Set<(request: Request) => void>());
  const published = useRef<Item | null>(null);
  /** What this guest last told the host, so it only tells it when that changes, and tells it again after rejoining. */
  const said = useRef<{ state: Readiness; id: string | null } | null>(null);
  /** The start or join being made, so Cancel and Leave can stop it, and one that is stopped does nothing when it ends. */
  const attempt = useRef<AbortController | null>(null);
  const nameNow = useRef(name);
  nameNow.current = name;

  const setName = useCallback((next: string) => {
    setNameState(next);
    try {
      localStorage.setItem(NAME_KEY, next);
    } catch {
      // Not remembered, which only means asking again next time.
    }
  }, []);

  const reset = useCallback(() => {
    attempt.current?.abort();
    attempt.current = null;
    host.current = null;
    guest.current = null;
    published.current = null;
    said.current = null;
    keepParty(null);
    setRole("none");
    setConnecting(false);
    setReconnecting(false);
    setHostSilent(false);
    setCode(null);
    setMembers([]);
    setItem(null);
    setTarget(null);
  }, []);

  const leave = useCallback(() => {
    host.current?.end();
    guest.current?.leave();
    reset();
  }, [reset]);

  const cancel = useCallback(() => {
    attempt.current?.abort();
    attempt.current = null;
    setConnecting(false);
  }, []);

  /** Starts an attempt that the rest of the page can stop. */
  const begin = useCallback((): AbortController => {
    attempt.current?.abort();
    const mine = new AbortController();
    attempt.current = mine;
    setError(null);
    setConnecting(true);
    return mine;
  }, []);

  const hostEvents = useCallback(
    (): HostEvents => ({
      members: setMembers,
      request: (request) => handlers.current.forEach((handler) => handler(request)),
      reachable: (reachable) => setError(reachable ? null : "Can't reach the party server, so no one new can join for now."),
    }),
    [],
  );

  /** Takes over as host of a party that is open. */
  const hosting = useCallback((party: HostParty) => {
    host.current = party;
    keepParty({ code: party.code, token: party.token });
    setRole("host");
    setCode(party.code);
    setInvite(null);
  }, []);

  const create = useCallback(async () => {
    if (attempt.current) return;
    leave();
    const mine = begin();
    try {
      const party = await openParty(nameNow.current, hostEvents());
      if (mine.signal.aborted) return party.end();
      hosting(party);
      setMembers([{ name: nameNow.current, host: true, state: "ready" }]);
    } catch (failure) {
      if (!mine.signal.aborted) setError(failure instanceof Error ? failure.message : "Could not start the party.");
    }
    if (attempt.current === mine) {
      attempt.current = null;
      setConnecting(false);
    }
  }, [begin, hostEvents, hosting, leave]);

  // A page that held a party before (a reload) opens the same one again, under the same code. The
  // guests come back to it on their own.
  useEffect(() => {
    setNameState(rememberedName());
    const params = new URLSearchParams(window.location.search);
    const invited = params.get("party");
    if (invited) {
      setInvite(partyCodeFrom(invited));
      // The code is private to the people it was shared with: it does not stay in the address bar, the history or the page's analytics.
      params.delete("party");
      const query = params.toString();
      window.history.replaceState(window.history.state, "", `${window.location.pathname}${query ? `?${query}` : ""}${window.location.hash}`);
    }
    const held = heldParty();
    if (!held || host.current) return;
    const mine = begin();
    openParty(rememberedName(), hostEvents(), held)
      .then((party) => {
        if (mine.signal.aborted) return party.end();
        hosting(party);
        setMembers([{ name: nameNow.current, host: true, state: "ready" }]);
      })
      .catch((failure: unknown) => {
        // Only a party that is really gone is forgotten: a failed network leaves it to try again on the next reload.
        const gone = failure instanceof signal.Gone;
        if (gone) keepParty(null);
        if (!mine.signal.aborted) setError(gone ? "Your party ended while the page was away." : "Could not open your party again. Reload to try again.");
      })
      .finally(() => {
        if (attempt.current === mine) {
          attempt.current = null;
          setConnecting(false);
        }
      });
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  /** What a guest hears from the host. */
  const guestEvents = useCallback(
    (reconnect: (code: string) => void, joined: string): GuestEvents => ({
      target: (next) => {
        setTarget(next);
        setItem((current) => (sameItem(current, next.playback.item) ? current : next.playback.item));
      },
      members: setMembers,
      silent: setHostSilent,
      ended: () => {
        reset();
        setError("The host ended the party.");
      },
      lost: () => {
        guest.current = null;
        reconnect(joined);
      },
    }),
    [reset],
  );

  /** Joins, and keeps what is needed after: the code, the role, and what was last said to the host. */
  const connect = useCallback(
    async (wanted: string, cancelled: AbortSignal): Promise<void> => {
      const party = await joinParty(wanted, nameNow.current, guestEvents(reconnectRef.current, wanted), cancelled);
      // Left while the connection was opening: it is not kept.
      if (cancelled.aborted) return party.leave();
      guest.current = party;
      setHostSilent(false);
      if (said.current) party.status(said.current.state, said.current.id);
    },
    [guestEvents],
  );

  /** A guest that lost the host keeps trying to get back in, for a while: the host's page may only have reloaded. */
  const reconnectRef = useRef<(code: string) => void>(() => {});
  const reconnect = useCallback(
    async (lost: string) => {
      const mine = begin();
      setConnecting(false);
      setError(null);
      setReconnecting(true);
      for (const started = Date.now(); !mine.signal.aborted;) {
        try {
          await connect(lost, mine.signal);
          break;
        } catch (failure) {
          if (mine.signal.aborted) return;
          if (failure instanceof signal.Gone || Date.now() - started > RECONNECT_FOR) {
            reset();
            setError(failure instanceof signal.Gone ? "The party ended." : "Lost the party and could not get back in.");
            return;
          }
          await new Promise((resolve) => setTimeout(resolve, RECONNECT_EVERY));
        }
      }
      if (attempt.current === mine) attempt.current = null;
      setReconnecting(false);
    },
    [begin, connect, reset],
  );
  reconnectRef.current = (lost) => void reconnect(lost);

  const join = useCallback(
    async (wanted: string) => {
      if (attempt.current) return;
      leave();
      const mine = begin();
      const joined = wanted.trim().toUpperCase();
      try {
        await connect(joined, mine.signal);
        if (mine.signal.aborted) {
          guest.current?.leave();
          return;
        }
        setRole("guest");
        setCode(joined);
        setInvite(null);
      } catch (failure) {
        if (!mine.signal.aborted) setError(failure instanceof Error ? failure.message : "Could not join the party.");
      }
      if (attempt.current === mine) {
        attempt.current = null;
        setConnecting(false);
      }
    },
    [begin, connect, leave],
  );

  // A guest closing the page says goodbye, so the host does not keep waiting for it. A host's
  // party is kept (see above): a reload opens it again, and a tab closed for good ends it on the
  // server in two minutes.
  useEffect(() => {
    const goodbye = () => guest.current?.leave();
    // A page put away and brought back (the back button) has said goodbye and has no connection: it is out of the party.
    const back = (event: PageTransitionEvent) => {
      if (event.persisted && guest.current) reset();
    };
    window.addEventListener("pagehide", goodbye);
    window.addEventListener("pageshow", back);
    return () => {
      window.removeEventListener("pagehide", goodbye);
      window.removeEventListener("pageshow", back);
    };
  }, [reset]);

  const publish = useCallback((playback: Playback, heartbeat = false) => {
    if (!host.current) return;
    if (heartbeat && !sameItem(playback.item, published.current)) return;
    published.current = playback.item;
    setItem((current) => (sameItem(current, playback.item) ? current : playback.item));
    host.current.publish(playback);
  }, []);

  const ask = useCallback((request: Request) => guest.current?.ask(request), []);

  const setStatus = useCallback((state: Readiness, id: string | null) => {
    if (said.current?.state === state && said.current.id === id) return;
    said.current = { state, id };
    guest.current?.status(state, id);
  }, []);

  const waitingFor = useMemo(() => members.filter((member) => !member.host && member.state === "loading").map((member) => member.name), [members]);

  const onRequest = useCallback((handler: (request: Request) => void) => {
    handlers.current.add(handler);
    return () => void handlers.current.delete(handler);
  }, []);

  const value = useMemo<Party>(
    () => ({
      role,
      connecting,
      code,
      members,
      error,
      item,
      target,
      reconnecting,
      hostSilent,
      invite,
      name,
      setName,
      create,
      join,
      cancel,
      leave,
      publish,
      ask,
      setStatus,
      waitingFor,
      onRequest,
    }),
    [
      role,
      connecting,
      code,
      members,
      error,
      item,
      target,
      reconnecting,
      hostSilent,
      invite,
      name,
      setName,
      create,
      join,
      cancel,
      leave,
      publish,
      ask,
      setStatus,
      waitingFor,
      onRequest,
    ],
  );

  return <Context.Provider value={value}>{children}</Context.Provider>;
}
