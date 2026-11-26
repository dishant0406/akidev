import { notFound } from "next/navigation";

import { CatalogueView } from "@/components/browse";
import { getScreens, isClientNavigation, serverGet } from "@/lib/api-server";
import { defaultSort, tabForSlug } from "@/lib/screens";
import type { Page } from "@/types/media";

// Rendered for each request with the first page already in it; see app/page.tsx.
export const dynamic = "force-dynamic";

export default async function CataloguePage({ params }: { params: Promise<{ kind: string }> }) {
  const screens = await getScreens();
  // With no answer the page is an error, not a 404: a 404 would be kept by the edge for a while.
  if (!screens) throw new Error("The API did not answer.");
  const tab = tabForSlug(screens, (await params).kind);
  if (!tab) notFound();
  const first = (await isClientNavigation())
    ? null
    : await serverGet<Page>(`/v1/catalogue?kind=${tab.kind}&sort=${defaultSort(screens)}&page=1`, 60);
  return <CatalogueView kind={tab.kind} initial={first ?? undefined} />;
}
