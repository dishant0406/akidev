"use client";

import Hero from "@/components/hero";
import MediaRow from "@/components/row";
import { watchPath } from "@/constants/routes";
import { fraction, useRecent } from "@/lib/progress";
import { useScreens } from "@/providers/screens";
import { useHome } from "@/query/use-media";
import type { HomeResponse } from "@/types/media";

/** How many rows are drawn as placeholders while the page loads. */
const LOADING_ROWS = 3;

export default function HomeView({ initial }: { initial?: HomeResponse }) {
  const home = useHome(initial);
  const recent = useRecent();
  const loadingTitles = (useScreens()?.home ?? []).slice(0, LOADING_ROWS).map((row) => row.title);

  return (
    <div className="pb-8">
      <Hero items={home.featured} loading={home.loading} />

      {recent.length > 0 && (
        <MediaRow
          eager
          title="Continue watching"
          items={recent.map((entry) => entry.media)}
          resume={recent.map((entry) => ({
            fraction: fraction(entry),
            href: watchPath(entry.media.id, entry.episodeId),
          }))}
        />
      )}

      {home.loading
        ? loadingTitles.map((title) => <MediaRow key={title} title={title} items={[]} loading />)
        : home.rows.map((row, index) => <MediaRow key={row.title} title={row.title} items={row.items} eager={index < 2} ranked={row.style === "ranked"} />)}
    </div>
  );
}

