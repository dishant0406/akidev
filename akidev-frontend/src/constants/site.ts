// What the site is made of. The catalogues, sorts, genres and home rows are not here: they come
// from the server (GET /v1/screens), as they do for the desktop app.
//
// There is one site. The anime and TV hostnames only redirect to it (see src/middleware.ts).

export type ContentType = "anime" | "series" | "movie";

/** The zone the site is served from. NEXT_PUBLIC_ so the middleware and the browser can read it. */
export const SITE_DOMAIN = process.env.NEXT_PUBLIC_SITE_DOMAIN || "akidev.live";

/** Where the source and the desktop releases live. */
export const REPO_URL = "https://github.com/dishant0406/akidev";


/** The landing page for the phone app (the manga, comics and books reader) has its own host, served by this same app. */
export const APP_HOST = `app.${SITE_DOMAIN}`;
export const isAppHost = (host: string | null) => host?.trim().toLowerCase() === APP_HOST;

/** The phone app in its two stores. */
export const APP_STORE_URL = "https://apps.apple.com/app/akidev/id6825537136";
export const PLAY_STORE_URL = "https://play.google.com/store/apps/details?id=live.akidev.manga";
