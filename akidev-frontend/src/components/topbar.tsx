"use client";

import { ChevronLeft, Clapperboard, House, Search, Sparkles, Tv, type LucideIcon } from "lucide-react";
import dynamic from "next/dynamic";
import Link from "next/link";
import { usePathname, useRouter } from "next/navigation";
import { useEffect, useState } from "react";

import { CATALOGUES } from "@/constants/site";
import { ROUTES } from "@/constants/routes";
import { cn } from "@/lib/utils";

import { IconButton } from "./button";
import PartyButton from "./party";
// Search is loaded the first time it is opened, not with every page.
const Palette = dynamic(() => import("./palette"));

/** The bar turns solid once the page has moved this far. */
const SOLID_AFTER = 40;

const ICONS: Record<string, LucideIcon> = { anime: Sparkles, series: Tv, films: Clapperboard };

const TABS: { label: string; href: string; icon: LucideIcon }[] = [
  { label: "Home", href: ROUTES.HOME, icon: House },
  ...CATALOGUES.map((page) => ({
    label: page.label,
    href: `/${page.slug}`,
    icon: ICONS[page.slug] ?? Clapperboard,
  })),
];

/**
 * Home, the three catalogues and search, in one bar that floats over the artwork. On a phone the
 * catalogues move to a bar at the foot of the screen, where a thumb reaches them.
 */
export default function TopBar() {
  const pathname = usePathname();
  const router = useRouter();
  const [scrolled, setScrolled] = useState(false);
  const [searching, setSearching] = useState(false);
  const [canGoBack, setCanGoBack] = useState(false);

  useEffect(() => {
    const onScroll = () => setScrolled(window.scrollY > SOLID_AFTER);
    onScroll();
    window.addEventListener("scroll", onScroll, { passive: true });
    return () => window.removeEventListener("scroll", onScroll);
  }, []);

  // Read after each navigation: a page opened straight from a link has nothing behind it.
  useEffect(() => setCanGoBack(window.history.length > 1 && pathname !== ROUTES.HOME), [pathname]);

  // Cmd-K or Ctrl-K opens search from anywhere, as it does in the app.
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "k") {
        event.preventDefault();
        setSearching((open) => !open);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const active = (href: string) => pathname === href;

  return (
    <>
      <header
        className="fixed inset-x-0 top-0 z-40 h-14"
      >
        {/* Two layers cross-fade: a soft shade over the artwork at the top, and once the page has
            moved, a blurred material with a hairline that only shows where content passes under. */}
        <div
          aria-hidden
          className={cn("absolute inset-0 bg-gradient-to-b from-canvas/85 to-transparent transition-opacity duration-300 ease-spring", scrolled && "opacity-0")}
        />
        <div
          aria-hidden
          className={cn(
            "material absolute inset-0 shadow-[0_1px_0_rgb(var(--ink)/0.07)] transition-opacity duration-300 ease-spring",
            !scrolled && "opacity-0",
          )}
        />
        <div className="gutter relative flex h-full items-center gap-3 md:gap-6">
          <IconButton
            label="Back"
            onClick={() => (canGoBack ? router.back() : router.push(ROUTES.HOME))}
            className={cn(!canGoBack && "hidden opacity-35 md:flex")}
          >
            <ChevronLeft className="size-5" />
          </IconButton>

          <Link href={ROUTES.HOME} className="flex items-center justify-center gap-2 text-xl font-semibold text-ink max-sm:size-11">
            {/* eslint-disable-next-line @next/next/no-img-element */}
            <img src="/icon.svg" alt="" width={28} height={28} className="size-7 shrink-0" />
            <span className="hidden sm:inline">AKIDEV</span>
          </Link>

          <nav aria-label="Catalogues" className="no-scrollbar hidden min-w-0 flex-1 items-center gap-5 overflow-x-auto md:flex">
            {TABS.map((tab) => (
              <Link
                key={tab.href}
                href={tab.href}
                // Fetched ahead, so a tab opens the moment it is pressed.
                prefetch
                aria-current={active(tab.href) ? "page" : undefined}
                className={cn(
                  "flex flex-col items-center gap-1 px-1 py-1 text-sm transition-colors",
                  active(tab.href) ? "font-semibold text-ink" : "text-dim hover:text-ink",
                )}
              >
                {tab.label}
                <span className={cn("h-0.5 w-full rounded-full", active(tab.href) && "bg-accent")} />
              </Link>
            ))}
          </nav>

          <div className="flex-1 md:hidden" />
          <PartyButton />
          <IconButton label="Search" onClick={() => setSearching(true)} className={cn(searching && "bg-wash-2")}>
            <Search className="size-[18px]" />
          </IconButton>
        </div>
      </header>

      {!pathname.startsWith("/watch") && (
        <nav aria-label="Catalogues" className="material fixed inset-x-0 bottom-0 z-40 flex shadow-[0_-1px_0_rgb(var(--ink)/0.07)] md:hidden">
          {TABS.map(({ label, href, icon: Icon }) => (
            <Link
              key={href}
              href={href}
              prefetch
              aria-current={active(href) ? "page" : undefined}
              className={cn(
                "relative flex h-14 flex-1 flex-col items-center justify-center gap-1 text-[11px] transition-colors",
                active(href) ? "font-semibold text-ink" : "text-dim",
              )}
            >
              {active(href) && <span className="absolute inset-x-6 top-0 h-0.5 rounded-full bg-accent" />}
              <Icon className="size-5" />
              {label}
            </Link>
          ))}
        </nav>
      )}

      {searching && <Palette onClose={() => setSearching(false)} />}
    </>
  );
}
