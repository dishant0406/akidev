// What the site is made of. The catalogues, sorts, genres and home rows are not here: they come
// from the server (GET /v1/screens), as they do for the desktop app.
//
// There is one site. The anime and TV hostnames only redirect to it (see src/middleware.ts).

export type ContentType = "anime" | "series" | "movie";

/** The zone the site is served from. NEXT_PUBLIC_ so the middleware and the browser can read it. */
export const SITE_DOMAIN = process.env.NEXT_PUBLIC_SITE_DOMAIN || "akidev.live";

/** Where the source and the desktop releases live. */
export const REPO_URL = "https://github.com/dishant0406/akidev";

