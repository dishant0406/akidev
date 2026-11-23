import type { MetadataRoute } from "next";
import { headers } from "next/headers";

import { APP_HOST, SITE_DOMAIN, isAppHost } from "@/constants/site";

/** The landing page's host has one page. On the site this lists only the home page. */
export default async function sitemap(): Promise<MetadataRoute.Sitemap> {
  const host = isAppHost((await headers()).get("host")) ? APP_HOST : SITE_DOMAIN;
  return [{ url: `https://${host}/`, changeFrequency: "monthly", priority: 1 }];
}
