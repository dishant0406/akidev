// Tests for reading media ids. They agree with desktop/core/src/media_id.rs.

import assert from "node:assert/strict";
import test from "node:test";

import { parseMediaId } from "../src/lib/media-id.ts";

test("each kind of id is read", () => {
  assert.deepEqual(parseMediaId("anime-16498-ep1"), {
    contentType: "anime", mediaId: "16498", season: null, episodeNumber: 1, key: "anime-16498",
  });
  assert.equal(parseMediaId("series-169-s1e5")?.season, 1);
  assert.equal(parseMediaId("movie-tt32261958")?.mediaId, "tt32261958");
});

test("a title that says where it comes from keeps its provider and its escaped id", () => {
  const parsed = parseMediaId("series-myprov~abc_2d12-s2e3");
  assert.equal(parsed?.mediaId, "myprov~abc_2d12");
  assert.equal(parsed?.key, "series-myprov~abc_2d12");
  assert.deepEqual([parsed?.season, parsed?.episodeNumber], [2, 3]);
});

test("ids that only look like that are refused", () => {
  for (const id of ["movie-~abc", "movie-p~", "movie-p~a~b", "movie-p~a_zz", "movie-p_a", "movie-tt1-ep1", "anime-1-s1e1"]) {
    assert.equal(parseMediaId(id), null, id);
  }
});
