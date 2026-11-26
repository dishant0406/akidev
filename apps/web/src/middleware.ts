import { NextResponse, type NextRequest } from "next/server";

import { APP_HOST, SITE_DOMAIN, isAppHost } from "@/constants/site";

/**
 * Retired hostnames send visitors to the one site.
 *
 * The site used to be two, anime.akidev.live and tv.akidev.live. It is now one, with Anime,
 * Series and Films tabs, so each old host redirects to the matching page of the main one and
 * keeps the rest of the path, which keeps old title and watch links working.
 */
const RETIRED: Record<string, string> = {
  [`anime.${SITE_DOMAIN}`]: "/anime",
  [`tv.${SITE_DOMAIN}`]: "/films",
};

/** Sends a request to the same path on the other host, with a 308. */
function moveTo(request: NextRequest, host: string, pathname = request.nextUrl.pathname) {
  const target = new URL(request.url);
  target.protocol = "https:";
  target.host = host;
  target.port = "";
  target.pathname = pathname;
  return NextResponse.redirect(target, 308);
}

export function middleware(request: NextRequest) {
  const host = request.headers.get("host")?.trim().toLowerCase() ?? "";
  const path = request.nextUrl.pathname;

  // The phone app's landing page is the page /app, shown at the root of its own host. The host has nothing else,
  // so any other path goes to the same path on the site. On the site, /app goes to the landing page's host.
  if (isAppHost(host)) {
    if (path === "/" || path === "/app") return NextResponse.rewrite(new URL("/app", request.url));
    return moveTo(request, SITE_DOMAIN);
  }
  if (host === SITE_DOMAIN && path === "/app") return moveTo(request, APP_HOST, "/");

  const landing = RETIRED[host];
  if (!landing) return NextResponse.next();

  const target = new URL(request.url);
  target.protocol = "https:";
  target.host = SITE_DOMAIN;
  target.port = "";
  if (request.nextUrl.pathname === "/") target.pathname = landing;
  return NextResponse.redirect(target, 308);
}

export const config = {
  // Everything except Next's own assets and static files.
  matcher: ["/((?!_next/|api/|favicon.ico|.*\\.).*)"],
};
