// What the site is made of. The catalogues, sorts, genres and home rows are not here: they come
// from the server (GET /v1/screens), as they do for the desktop app.
//
// There is one site. The anime and TV hostnames only redirect to it (see src/middleware.ts).

export type ContentType = "anime" | "series" | "movie";

/** The zone the site is served from. NEXT_PUBLIC_ so the middleware and the browser can read it. */
export const SITE_DOMAIN = process.env.NEXT_PUBLIC_SITE_DOMAIN || "akidev.live";

export const CONTENT_TYPES: ContentType[] = ["anime", "series", "movie"];

/** Where the source and the desktop releases live. */
export const REPO_URL = "https://github.com/dishant0406/akidev";

/** The attribution the metadata providers require. */
export const ATTRIBUTION =
  "Metadata from AniList, Cinemeta and TVMaze (CC BY-SA). Akidev stores no media: playback links are resolved from public sources when you press play.";

/** The small label over a title: "ANIME", "SERIES", "FILM". */
export const KIND_LABEL: Record<ContentType, string> = {
  anime: "ANIME",
  series: "SERIES",
  movie: "FILM",
};
