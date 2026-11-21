"use client";

import { createContext, useContext, type ReactNode } from "react";
import { useQuery } from "react-query";

import { get } from "@/lib/api";
import type { Screens } from "@/types/media";

const ScreensContext = createContext<Screens | null>(null);

/**
 * Hands the screens to every page. The server that rendered the page reads them first; when it
 * could not, the browser asks, so a page that arrived without tabs gets them a moment later.
 */
export default function ScreensProvider({ screens, children }: { screens: Screens | null; children: ReactNode }) {
  const query = useQuery(["screens"], () => get<Screens>("/v1/screens"), {
    initialData: screens ?? undefined,
    staleTime: 5 * 60 * 1000,
  });
  return <ScreensContext.Provider value={query.data ?? null}>{children}</ScreensContext.Provider>;
}

export function useScreens(): Screens | null {
  return useContext(ScreensContext);
}
