// Tests for the watch party's messages and the rules that keep a guest in step. The same cases
// as desktop/party/src/{protocol,sync}.rs, so the two apps agree.

import assert from "node:assert/strict";
import test from "node:test";

import { CODE_LENGTH, partyCodeFrom } from "../src/lib/party/code.ts";
import { cleanName, guestState } from "../src/lib/party/connection.ts";
import { itemKey, parseMessage, sameItem } from "../src/lib/party/protocol.ts";
import { Clock, fix, positionNow, targetOf } from "../src/lib/party/sync.ts";

test("messages are read the way the desktop app writes them", () => {
  assert.deepEqual(
    parseMessage('{"t":"state","item":{"kind":"video","id":"movie-tt1"},"playing":true,"position":12.5,"at":1700000000000}'),
    { t: "state", item: { kind: "video", id: "movie-tt1", release: null }, playing: true, position: 12.5, at: 1700000000000 },
  );
  assert.deepEqual(parseMessage('{"t":"pause"}'), { t: "pause" });
  assert.deepEqual(parseMessage('{"t":"seek","position":3.0}'), { t: "seek", position: 3 });
  assert.deepEqual(parseMessage('{"t":"state","item":{"kind":"music","id":"abc","title":"A","artist":"B"},"playing":false,"position":0,"at":1}'), {
    t: "state",
    item: { kind: "music", id: "abc", title: "A", artist: "B", cover: null },
    playing: false,
    position: 0,
    at: 1,
  });
  assert.deepEqual(parseMessage('{"t":"members","members":[{"name":"Hana","host":true,"state":"ready"}]}'), {
    t: "members",
    members: [{ name: "Hana", host: true, state: "ready" }],
  });
  assert.deepEqual(parseMessage('{"t":"status","state":"loading","id":"movie-tt1"}'), { t: "status", state: "loading", id: "movie-tt1" });
  assert.deepEqual(parseMessage('{"t":"status","state":"ready","id":null}'), { t: "status", state: "ready", id: null });
});

test("anything else is turned away", () => {
  for (const text of ["", "nonsense", '{"t":"status","state":"asleep","id":null}', '{"t":"status","trouble":true}', "[]", "null", '{"t":"unknown"}', '{"t":"seek"}', '{"t":"seek","position":"3"}', '{"t":"open","item":{"kind":"video","id":"x"}}', '{"t":"state","item":{"kind":"x"},"playing":true,"position":1,"at":1}']) {
    assert.equal(parseMessage(text), null, text);
  }
});

test("a small gap is closed by changing speed and a big one by seeking", () => {
  assert.deepEqual(fix(10, 10.05, true), { seek: null, speed: 1 });
  assert.deepEqual(fix(10, 10.5, true), { seek: null, speed: 1.05 });
  assert.deepEqual(fix(10.5, 10, true), { seek: null, speed: 0.95 });
  assert.deepEqual(fix(10, 14, true), { seek: 14, speed: 1 });
  assert.deepEqual(fix(14, 10, true), { seek: 10, speed: 1 });
});

test("a paused party is matched exactly at normal speed", () => {
  assert.deepEqual(fix(10, 10.5, false), { seek: 10.5, speed: 1 });
  assert.deepEqual(fix(10, 10.1, false), { seek: null, speed: 1 });
});

test("the clock is read from the quickest round trip", () => {
  const clock = new Clock();
  // The host's clock is 5000 ms ahead. A slow trip, then a fast one.
  clock.sample(1000, 6300, 1600);
  clock.sample(2000, 7010, 2020);
  assert.equal(clock.hostTime(3000), 8000);
});

test("a state is moved on by the time it took to arrive", () => {
  const clock = new Clock();
  clock.sample(1000, 6010, 1020);
  // The host said "10.0 seconds" at its 7000; it is now its 7400 (this device's 2400).
  const target = targetOf({ item: null, playing: true, position: 10, at: 7000 }, clock, 2400, 0);
  assert.ok(Math.abs(target.playback.position - 10.4) < 0.001);
  assert.equal(targetOf({ item: null, playing: false, position: 10, at: 7000 }, clock, 2400, 0).playback.position, 10);
  // And on from there as time passes here.
  assert.ok(Math.abs(positionNow(target, 2000) - 12.4) < 0.001);
});

test("a guest is loading what it has not yet said it is ready for", () => {
  // Nothing playing: whatever the guest said stands.
  assert.equal(guestState("ready", null, null), "ready");
  // Something is playing and the guest has said nothing, or said it of the one before.
  assert.equal(guestState("ready", null, "b"), "loading");
  assert.equal(guestState("ready", "a", "b"), "loading");
  // Said of this one.
  assert.equal(guestState("ready", "b", "b"), "ready");
  assert.equal(guestState("loading", "b", "b"), "loading");
  assert.equal(guestState("trouble", "b", "b"), "trouble");
});

test("a clock that is far out cannot send a guest far away", () => {
  // No reading yet, and the host's clock is ten minutes behind this one.
  const target = targetOf({ item: null, playing: true, position: 10, at: 0 }, new Clock(), 600_000, 0);
  assert.equal(target.playback.position, 20);
});

test("a guest's name is kept short and plain", () => {
  assert.equal(cleanName("  Sam \n"), "Sam");
  assert.equal(cleanName(""), "Guest");
  assert.equal(cleanName("x".repeat(100)).length, 24);
});

test("the release and what the host is choosing travel with the state, as the desktop app writes them", () => {
  const text =
    '{"t":"state","item":{"kind":"video","id":"movie-tt1","release":"abc123"},"playing":false,"position":4.0,"at":7,"choosing":"source"}';
  assert.deepEqual(parseMessage(text), {
    t: "state",
    item: { kind: "video", id: "movie-tt1", release: "abc123" },
    playing: false,
    position: 4,
    at: 7,
    choosing: "source",
  });
  // A state from a device that predates both fields still reads, and an unknown panel is ignored.
  const old = parseMessage('{"t":"state","item":{"kind":"video","id":"movie-tt1"},"playing":true,"position":1,"at":1,"choosing":"what"}');
  assert.equal(old?.t === "state" && old.choosing, undefined);
});

test("a status names the release too, so a new release is something to get to", () => {
  const a = { kind: "video" as const, id: "m", release: "x" };
  const b = { kind: "video" as const, id: "m", release: "y" };
  assert.notEqual(itemKey(a), itemKey(b));
  assert.equal(itemKey({ kind: "video", id: "m", release: null }), "m");
  assert.equal(sameItem(a, b), false);
  assert.equal(sameItem(a, { ...a }), true);
  // The same film on another release is a new thing to get to, and a device from before releases
  // were shared says the plain id, which is enough.
  assert.equal(guestState("ready", "m#x", "m#y", "m"), "loading");
  assert.equal(guestState("ready", "m#y", "m#y", "m"), "ready");
  assert.equal(guestState("ready", "m", "m#y", "m"), "ready");
});

test("what the host is choosing reaches the guest's target", () => {
  const clock = new Clock();
  clock.sample(1000, 6010, 1020);
  const target = targetOf({ item: null, playing: false, position: 3, at: 7000, choosing: "episode" }, clock, 2400);
  assert.equal(target.playback.choosing, "episode");
  assert.equal(targetOf({ item: null, playing: false, position: 3, at: 7000 }, clock, 2400).playback.choosing, null);
});

test("a pasted link gives its code, and nothing else that is pasted is turned into one by accident", () => {
  assert.equal(CODE_LENGTH, 6);
  assert.equal(partyCodeFrom("https://akidev.live/?party=k7qf92"), "K7QF92");
  assert.equal(partyCodeFrom("see https://akidev.live/anime?x=1&party=K7QF92#top"), "K7QF92");
  assert.equal(partyCodeFrom("k7q f92"), "K7QF92");
  // O, 0, I and 1 are never in a code, so they are dropped, and a code is never longer than six.
  assert.equal(partyCodeFrom("K7QF92ABC"), "K7QF92");
  assert.equal(partyCodeFrom("K0O1I7"), "K7");
  // A link that is not a party link is not a code (its letters would otherwise be "HTTPSA").
  assert.equal(partyCodeFrom("https://akidev.live/anime"), "");
});

test("a name has no invisible or direction-changing characters and is cut by whole characters", () => {
  assert.equal(cleanName("  Sam \n"), "Sam");
  assert.equal(cleanName("\u202eSam\u200b"), "Sam");
  assert.equal(cleanName(""), "Guest");
  assert.equal(cleanName("\u200b\u200e"), "Guest");
  // 30 emoji are cut to 24 whole ones, never half of one.
  const cut = cleanName("😀".repeat(30));
  assert.equal([...cut].length, 24);
  assert.equal(cut, "😀".repeat(24));
});

test("the clock follows the host after a change, because an old reading is forgotten", () => {
  const clock = new Clock();
  // The host's clock is 5000 ms ahead, read with a very quick round trip.
  clock.sample(1000, 6001, 1002);
  assert.equal(clock.hostTime(2000), 7000);
  // Two minutes later it has been put right (2000 ms ahead), and the trips are slower.
  clock.sample(121_000, 123_050, 121_100);
  assert.equal(clock.hostTime(122_000), 124_000);
  // A trip that took forever says nothing.
  const fresh = new Clock();
  fresh.sample(0, 5000, 60_000);
  assert.equal(fresh.known, false);
});
