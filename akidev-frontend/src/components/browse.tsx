"use client";

import { ChevronDown, ChevronUp } from "lucide-react";
import { useState } from "react";

import type { ContentType } from "@/constants/site";
import type { Card } from "@/lib/media";
import { defaultSort, genresFor } from "@/lib/screens";
import { useScreens } from "@/providers/screens";
import type { Page as CataloguePage, Screens } from "@/types/media";
import { cn } from "@/lib/utils";
import { useBrowse, useSearch } from "@/query/use-media";

import { Tab } from "./button";
import { EmptyState, ErrorState, LoadMore, Skeleton } from "./states";
import Tile from "./tile";

const SKELETONS = 12;

/** Two or more columns that share the width on a phone, and the fixed poster width beyond it. */
const GRID = "grid grid-cols-[repeat(auto-fill,minmax(8rem,1fr))] gap-x-4 gap-y-5 sm:grid-cols-[repeat(auto-fill,176px)] sm:gap-x-5 sm:gap-y-6";

function Grid({ cards }: { cards: Card[] }) {
  return (
    <div className={GRID}>
      {cards.map((card, index) => (
        <Tile key={card.id} card={card} priority={index < 6} className="w-full" />
      ))}
    </div>
  );
}

function Loading() {
  return (
    <div className={GRID}>
      {Array.from({ length: SKELETONS }, (_, index) => (
        <Skeleton key={index} className="aspect-[2/3] w-full rounded-lg" />
      ))}
    </div>
  );
}

function Page({ heading, filters, children }: { heading: string; filters: React.ReactNode; children: React.ReactNode }) {
  return (
    <div className="gutter flex flex-col gap-6 pb-16 pt-[84px]">
      <h1 className="break-words font-title text-3xl font-semibold sm:text-4xl tracking-[-0.02em] text-ink">{heading}</h1>
      {filters}
      {children}
    </div>
  );
}

/** One catalogue, or all of them: sort tabs, a genre list, and a grid that loads more. */
export function CatalogueView({ kind, initial }: { kind: ContentType | null; initial?: CataloguePage }) {
  const screens = useScreens();
  if (!screens) {
    return (
      <Page heading="Browse" filters={null}>
        <ErrorState title="Couldn't reach the catalogue" detail="Check your connection and try again." />
      </Page>
    );
  }
  return <Catalogue screens={screens} kind={kind} initial={initial} />;
}

function Catalogue({ screens, kind, initial }: { screens: Screens; kind: ContentType | null; initial?: CataloguePage }) {
  const [picked, setSort] = useState<string | undefined>();
  const sort = picked ?? defaultSort(screens);
  const [genre, setGenre] = useState<string | undefined>();
  const [genresOpen, setGenresOpen] = useState(false);
  const browse = useBrowse(kind, sort, genre, sort === defaultSort(screens) ? initial : undefined);

  const heading = screens.tabs.find((tab) => tab.kind === kind)?.label ?? "Browse";

  const filters = (
    <div className="flex flex-col">
      <div className="flex flex-col sm:flex-row sm:items-center sm:justify-between sm:gap-4">
        {/* Reaches the screen edges on a phone, so a tab that does not fit is seen running off. */}
        <div className="no-scrollbar -mx-5 flex gap-1 overflow-x-auto px-4 sm:mx-0 sm:gap-5 sm:px-0">
          {screens.sorts.map((each) => (
            <Tab key={each.sort} active={sort === each.sort} onClick={() => setSort(each.sort)}>
              {each.label}
            </Tab>
          ))}
        </div>
        <button
          type="button"
          onClick={() => setGenresOpen((open) => !open)}
          aria-expanded={genresOpen}
          className="flex min-h-11 shrink-0 cursor-pointer items-center gap-1.5 self-start text-sm sm:min-h-0 sm:pb-[7px]"
        >
          <span className="text-dim">{genre ? "Genre" : "All genres"}</span>
          {genre && <span className="font-semibold text-ink">{genre}</span>}
          {genresOpen ? <ChevronUp className="size-4 text-dim" /> : <ChevronDown className="size-4 text-dim" />}
        </button>
      </div>
      {genresOpen && (
        <div className="flex flex-wrap gap-x-6 gap-y-2 pt-5">
          {genresFor(screens, kind).map((each) => {
            const active = genre === each;
            return (
              <button
                key={each}
                type="button"
                onClick={() => setGenre(active ? undefined : each)}
                className={cn(
                  "flex cursor-pointer items-center gap-2 py-2 text-sm transition-colors hover:text-ink sm:py-1",
                  active ? "font-semibold text-ink" : "text-dim",
                )}
              >
                <span className={cn("size-[5px] rounded-full", active && "bg-accent")} />
                {each}
              </button>
            );
          })}
        </div>
      )}
    </div>
  );

  return (
    <Page heading={heading} filters={filters}>
      {browse.loading ? (
        <Loading />
      ) : browse.failed ? (
        <ErrorState title="Couldn't reach the catalogue" detail="Check your connection and try again." onRetry={browse.retry} />
      ) : browse.results.length === 0 ? (
        <EmptyState title="No results" detail="Nothing matches these filters." />
      ) : (
        <>
          <Grid cards={browse.results} />
          {browse.hasMore && <LoadMore shown={browse.results.length} loading={browse.loadingMore} onMore={browse.loadMore} />}
        </>
      )}
    </Page>
  );
}

/** The full results of a search, across every catalogue, narrowed by a tab. */
export function SearchView({ term }: { term: string }) {
  const screens = useScreens();
  const kindTabs: { kind: ContentType | null; label: string }[] = [
    { kind: null, label: "All" },
    ...(screens?.tabs ?? []).map((tab) => ({ kind: tab.kind, label: tab.label })),
  ];
  const [kind, setKind] = useState<ContentType | null>(null);
  const search = useSearch(term, 60);
  const shown = search.results.filter((card) => !kind || card.kind === kind);

  const filters = (
    <div className="no-scrollbar -mx-5 flex gap-1 overflow-x-auto px-4 sm:mx-0 sm:gap-5 sm:px-0">
      {kindTabs.map((tab) => (
        <Tab key={tab.label} active={kind === tab.kind} onClick={() => setKind(tab.kind)}>
          {tab.label}
        </Tab>
      ))}
    </div>
  );

  return (
    <Page heading={`Results for “${term}”`} filters={filters}>
      {search.searching && (search.results.length === 0 || search.stale) ? (
        <Loading />
      ) : search.failed ? (
        <ErrorState title="Couldn't reach the catalogue" detail="Check your connection and try again." />
      ) : shown.length === 0 ? (
        <EmptyState title="No results" detail="Nothing matched. Try another spelling or fewer words." />
      ) : (
        <Grid cards={shown} />
      )}
    </Page>
  );
}
