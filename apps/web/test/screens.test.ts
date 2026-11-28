// Tests for reading the screens. The data is a copy of the screens of the sources document (the
// akidev-sources repo), kept next to the test so the website's checks need nothing from another repo.
// It is the shape GET /v1/screens sends.

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

import { defaultSort, genresFor, tabForSlug, tagFor } from "../src/lib/screens.ts";
import type { Screens } from "../src/types/media.ts";

const screens: Screens = JSON.parse(
  readFileSync(new URL("./screens.fixture.json", import.meta.url), "utf8"),
);

test("each catalogue page is found by its slug", () => {
  assert.equal(tabForSlug(screens, "anime")?.kind, "anime");
  assert.equal(tabForSlug(screens, "series")?.kind, "series");
  assert.equal(tabForSlug(screens, "films")?.kind, "movie");
  assert.equal(tabForSlug(screens, "movie"), undefined);
  assert.equal(tabForSlug(screens, "settings"), undefined);
});

test("the three catalogues are all offered, in the top bar's order", () => {
  assert.deepEqual(
    screens.tabs.map((tab) => tab.label),
    ["Anime", "Series", "Films"],
  );
  assert.deepEqual(
    screens.tabs.map((tab) => tab.kind),
    ["anime", "series", "movie"],
  );
});

test("a genre filter only offers genres the provider has", () => {
  // AniList and Cinemeta spell genres differently; a genre one lacks only ever returns nothing.
  assert.ok(genresFor(screens, "anime").includes("Mecha"));
  assert.ok(!genresFor(screens, "series").includes("Mecha"));
  assert.ok(genresFor(screens, "movie").includes("Documentary"));
  for (const genre of genresFor(screens, null)) {
    assert.ok(genresFor(screens, "anime").includes(genre) && genresFor(screens, "movie").includes(genre), genre);
  }
});

test("a kind with no genres offers none", () => {
  assert.deepEqual(genresFor({ ...screens, genres: {} }, "anime"), []);
});

test("a catalogue opens on the first sort, which is what the server renders it with", () => {
  assert.equal(defaultSort(screens), screens.sorts[0].sort);
  assert.equal(defaultSort(screens), "popularity");
});

test("every home row is for a catalogue that has a tab", () => {
  assert.equal(screens.home.length, 10);
  for (const row of screens.home) {
    assert.ok(screens.tabs.some((tab) => tab.kind === row.kind), row.title);
  }
});

test("the home page has a ranked row for each catalogue, and the other rows have no style", () => {
  const ranked = screens.home.filter((row) => row.style === "ranked");
  assert.deepEqual(ranked.map((row) => row.kind).sort(), ["anime", "movie", "series"]);
  assert.ok(screens.home.every((row) => row.style === undefined || row.style === "ranked"));
});

test("the label over a title comes from its tab, and is empty when the screens are missing", () => {
  assert.equal(tagFor(screens, "movie"), "FILM");
  assert.equal(tagFor(screens, "anime"), "ANIME");
  assert.equal(tagFor(null, "movie"), "");
});

test("the credits name the catalogues the footer and the privacy page list", () => {
  assert.match(screens.about.attribution, /AniList/);
  assert.ok(screens.about.catalogues.length > 0);
  assert.ok(screens.about.subtitles.includes("OpenSubtitles"));
});
