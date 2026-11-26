import assert from "node:assert/strict";
import test from "node:test";

import { playStaleTime } from "../src/query/play-policy.ts";
import type { PlayResult } from "../src/types/media.ts";

test("unavailable play results are immediately retryable while successful URLs stay fresh", () => {
  assert.equal(playStaleTime({ available: false, reason: "Timed out." }), 0);
  assert.equal(playStaleTime({ available: false, reason: "No source yet." }), 0);
  const success: PlayResult = {
    available: true, url: "/v1/stream/session/file", stream_type: "file",
    release: "Show", release_id: "a".repeat(40), source: "torrent", duration: null,
  };
  assert.equal(playStaleTime(success), 10 * 60 * 1000);
  assert.equal(playStaleTime(undefined), 10 * 60 * 1000);
  assert.equal(success.url, "/v1/stream/session/file");
});
