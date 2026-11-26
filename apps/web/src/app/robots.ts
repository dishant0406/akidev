import type { MetadataRoute } from "next";
import { headers } from "next/headers";

import { APP_HOST, isAppHost } from "@/constants/site";

/** Every crawler may read everything. The landing page's host also names its own sitemap. */
export default async function robots(): Promise<MetadataRoute.Robots> {
  const rules = { userAgent: "*", allow: "/" };
  if (!isAppHost((await headers()).get("host"))) return { rules };
  return { rules, sitemap: `https://${APP_HOST}/sitemap.xml` };
}
