// Tests for the REST client: the URL it builds, the body it sends, and the errors it turns
// responses into. fetch is replaced with a stub, so nothing here touches a network.

import assert from "node:assert/strict";
import test from "node:test";

import { ApiError, get, post } from "../src/lib/api.ts";

type Call = { url: string; init: RequestInit };

function stub(respond: (init: RequestInit) => Response | Promise<Response>): Call[] {
  const calls: Call[] = [];
  globalThis.fetch = (async (url: string, init: RequestInit) => {
    calls.push({ url, init });
    return respond(init);
  }) as typeof fetch;
  return calls;
}

const json = (body: unknown, status = 200) =>
  new Response(JSON.stringify(body), { status, headers: { "Content-Type": "application/json" } });

test("a get adds the query and leaves out what is not set", async () => {
  const calls = stub(() => json({ results: [] }));
  await get("/v1/catalogue", { kind: "all", sort: "popularity", genre: undefined, page: 2 });
  assert.equal(calls[0].url, "http://localhost:9000/v1/catalogue?kind=all&sort=popularity&page=2");
  assert.equal(calls[0].init.method, "GET");
});

test("a get with no query has no question mark", async () => {
  const calls = stub(() => json({}));
  await get("/v1/home");
  assert.equal(calls[0].url, "http://localhost:9000/v1/home");
});

test("a post sends its body as JSON", async () => {
  const calls = stub(() => json({ available: false, reason: "none" }));
  await post("/v1/play", { id: "movie-tt0111161", hevc: true });
  assert.equal(calls[0].init.method, "POST");
  assert.equal(calls[0].init.body, '{"id":"movie-tt0111161","hevc":true}');
  assert.deepEqual(calls[0].init.headers, { "Content-Type": "application/json" });
});

test("an error body's message and the status reach the caller", async () => {
  stub(() => json({ error: "busy" }, 503));
  await assert.rejects(post("/v1/play", { id: "x" }), (error: unknown) => {
    assert.ok(error instanceof ApiError);
    assert.equal(error.message, "busy");
    assert.equal(error.status, 503);
    return true;
  });
});

test("a failure with no readable body still says what happened", async () => {
  stub(() => new Response("<html>", { status: 502 }));
  await assert.rejects(get("/v1/home"), /returned 502/);
});

test("an unreachable API is reported as such", async () => {
  stub(() => Promise.reject(new TypeError("fetch failed")));
  await assert.rejects(get("/v1/home"), /Could not reach/);
});

test("a timeout is reported as too slow", async () => {
  stub(() => Promise.reject(new DOMException("timed out", "TimeoutError")));
  await assert.rejects(get("/v1/home"), /took too long/);
});

function untilAborted(init: RequestInit): Promise<Response> {
  return new Promise((_, reject) => {
    const signal = init.signal!;
    if (signal.aborted) reject(signal.reason);
    else signal.addEventListener("abort", () => reject(signal.reason), { once: true });
  });
}

const isCancellation = (error: unknown) => {
  assert.ok(error instanceof Error);
  assert.equal(error.name, "AbortError");
  assert.doesNotMatch(error.message, /Could not reach|took too long|unavailable/i);
  return true;
};

test("an already cancelled request never calls fetch", async () => {
  const calls = stub(() => json({}));
  const controller = new AbortController();
  controller.abort(new Error("private caller reason"));
  await assert.rejects(post("/v1/play", { id: "movie-1" }, controller.signal), isCancellation);
  assert.equal(calls.length, 0);
});

test("caller cancellation aborts an in-flight play without reporting an outage", async () => {
  const calls = stub(untilAborted);
  const controller = new AbortController();
  const pending = post("/v1/play", { id: "movie-1" }, controller.signal);
  controller.abort();
  await assert.rejects(pending, isCancellation);
  assert.equal(calls[0].init.signal?.aborted, true);
  assert.notEqual(calls[0].init.signal, controller.signal);
});

test("timeout still aborts fetch when a caller signal is supplied", async () => {
  const original = AbortSignal.timeout;
  const timeout = new AbortController();
  AbortSignal.timeout = () => timeout.signal;
  try {
    stub(untilAborted);
    const caller = new AbortController();
    const pending = get("/v1/home", undefined, caller.signal);
    timeout.abort(new DOMException("timed out", "TimeoutError"));
    caller.abort();
    await assert.rejects(pending, /took too long/);
    assert.equal(caller.signal.aborted, true);
  } finally {
    AbortSignal.timeout = original;
  }
});

test("cancellation while reading JSON remains a cancellation", async () => {
  const controller = new AbortController();
  stub((init) => ({ ok: true, json: () => untilAborted(init) }) as unknown as Response);
  const pending = get("/v1/home", undefined, controller.signal);
  await Promise.resolve();
  controller.abort();
  await assert.rejects(pending, isCancellation);
});

test("completed requests remove caller cancellation listeners", async () => {
  const controller = new AbortController();
  const original = controller.signal.removeEventListener.bind(controller.signal);
  let removed = 0;
  controller.signal.removeEventListener = (type: string, listener: EventListenerOrEventListenerObject | null, options?: boolean | EventListenerOptions) => {
    if (type === "abort") removed++;
    if (listener !== null) original(type, listener, options);
  };
  stub(() => json({}));
  await get("/v1/home", undefined, controller.signal);
  assert.equal(removed, 1);
});
