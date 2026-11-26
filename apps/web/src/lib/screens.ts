// Reading the screens the server sends (GET /v1/screens).

import type { Kind, Screens } from "@/types/media";

/** The tab whose address is `slug`. */
export function tabForSlug(screens: Screens, slug: string) {
  return screens.tabs.find((tab) => tab.slug === slug);
}

/** The genres to offer for one catalogue, or for all of them when `kind` is null. */
export function genresFor(screens: Screens, kind: Kind | null): string[] {
  return screens.genres[kind ?? "all"] ?? [];
}

/** The sort a catalogue opens on, which is what the server renders it with. */
export function defaultSort(screens: Screens): string {
  return screens.sorts[0].sort;
}

/** The small label over a title of this kind ("FILM"), from the tab that shows it. */
export function tagFor(screens: Screens | null, kind: Kind): string {
  return screens?.tabs.find((tab) => tab.kind === kind)?.tag ?? "";
}
