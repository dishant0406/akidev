// Tests for the record of pages that Back goes by. A watch page and its title used to take turns
// for ever, because opening the title again from the player added a step instead of taking one.

import assert from "node:assert/strict";
import test from "node:test";

import { cameFrom, visit, willReplace } from "../src/lib/trail.ts";

const TITLE = "/title/anime-1";
const WATCH = "/watch/anime-1/anime-1-ep1";

function freshTab() {
  const saved = new Map<string, string>();
  Object.assign(globalThis, {
    sessionStorage: {
      getItem: (key: string) => saved.get(key) ?? null,
      setItem: (key: string, value: string) => void saved.set(key, value),
    },
  });
}

test("a page opened first has nothing behind it", () => {
  freshTab();
  visit(TITLE);
  assert.equal(cameFrom(), null);
});

test("pages opened one after another each know the one before", () => {
  freshTab();
  visit("/anime");
  visit(TITLE);
  visit(WATCH);
  assert.equal(cameFrom(), TITLE);
});

test("going back to the title takes a step off, so Back from the title lands where it was opened from", () => {
  freshTab();
  visit("/anime");
  visit(TITLE);
  visit(WATCH);
  visit(TITLE);
  assert.equal(cameFrom(), "/anime");
});

test("a reload changes nothing", () => {
  freshTab();
  visit(TITLE);
  visit(WATCH);
  visit(WATCH);
  assert.equal(cameFrom(), TITLE);
});

test("changing episodes in the player does not add a page", () => {
  freshTab();
  visit(TITLE);
  visit(WATCH);
  visit("/watch/anime-1/anime-1-ep2");
  visit("/watch/anime-1/anime-1-ep3");
  assert.equal(cameFrom(), TITLE);
  visit(TITLE);
  assert.equal(cameFrom(), null);
});

test("a page that replaces the last one leaves nothing behind it", () => {
  freshTab();
  visit(WATCH);
  willReplace();
  visit(TITLE);
  assert.equal(cameFrom(), null);
});

test("without storage nothing is remembered and nothing breaks", () => {
  Object.assign(globalThis, { sessionStorage: undefined });
  visit(TITLE);
  assert.equal(cameFrom(), null);
});
