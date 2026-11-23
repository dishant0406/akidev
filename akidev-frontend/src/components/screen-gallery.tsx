"use client";

/* eslint-disable @next/next/no-img-element -- local pictures at their exact size: the image optimizer would only resize them */
import { ChevronLeft, ChevronRight } from "lucide-react";
import { useEffect, useRef, useState } from "react";

import { Phone } from "@/components/phone";
import { cn } from "@/lib/utils";

export type Slide = { id: string; title: string; body: string; screen: string; alt: string };

/**
 * The app's screens in a row you swipe. The next card always peeks past the edge, so it is plain that there is more.
 * On a wide screen two buttons step through it.
 */
export function ScreenGallery({ slides }: { slides: Slide[] }) {
  const track = useRef<HTMLUListElement>(null);
  const [edge, setEdge] = useState({ start: true, end: false });

  useEffect(() => {
    const el = track.current;
    if (!el) return;
    const read = () => setEdge({ start: el.scrollLeft <= 4, end: el.scrollLeft + el.clientWidth >= el.scrollWidth - 4 });
    read();
    el.addEventListener("scroll", read, { passive: true });
    window.addEventListener("resize", read);
    return () => {
      el.removeEventListener("scroll", read);
      window.removeEventListener("resize", read);
    };
  }, []);

  const go = (direction: -1 | 1) => {
    const el = track.current;
    const card = el?.firstElementChild as HTMLElement | null;
    if (!el || !card) return;
    const reduced = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    el.scrollBy({ left: direction * (card.offsetWidth + 16), behavior: reduced ? "auto" : "smooth" });
  };

  const button = (direction: -1 | 1, disabled: boolean) => (
    <button
      type="button"
      onClick={() => go(direction)}
      disabled={disabled}
      aria-label={direction < 0 ? "Show earlier screens" : "Show later screens"}
      className="flex size-11 items-center justify-center rounded-full border border-line text-ink transition-[transform,opacity] duration-150 ease-out active:scale-[0.96] disabled:opacity-30 motion-reduce:transition-none [@media(hover:hover)]:enabled:hover:bg-wash"
    >
      {direction < 0 ? <ChevronLeft aria-hidden className="size-5" /> : <ChevronRight aria-hidden className="size-5" />}
    </button>
  );

  return (
    <div className="[container-type:inline-size]">
      <div className="gutter mx-auto flex max-w-[72rem] items-end justify-between gap-6">
        <h2 className="text-balance font-sans text-[clamp(1.75rem,3.6vw,2.5rem)] font-semibold leading-[1.15] tracking-[-0.02em] text-ink">Inside the app</h2>
        <div className="hidden gap-2 md:flex">
          {button(-1, edge.start)}
          {button(1, edge.end)}
        </div>
      </div>
      <ul
        ref={track}
        className={cn(
          "no-scrollbar mt-8 flex snap-x snap-mandatory gap-4 overflow-x-auto overscroll-x-contain pb-2",
          "ps-5 pe-5 scroll-ps-5",
          "md:ps-[max(2.5rem,calc((100cqw-72rem)/2+2.5rem))] md:pe-10 md:scroll-ps-[max(2.5rem,calc((100cqw-72rem)/2+2.5rem))]",
        )}
      >
        {slides.map((slide) => (
          <li key={slide.id} className="w-[72%] max-w-[18rem] shrink-0 snap-start md:w-72">
            <div className="relative h-[26rem] overflow-hidden rounded-2xl bg-surface">
              <Phone width="11.5rem" className="absolute left-1/2 top-8 -translate-x-1/2">
                <img src={slide.screen} alt={slide.alt} width={924} height={2160} loading="lazy" decoding="async" className="h-full w-full object-cover" />
              </Phone>
            </div>
            <h3 className="mt-5 text-[17px] font-semibold leading-6 tracking-[-0.01em] text-ink">{slide.title}</h3>
            <p className="mt-1.5 text-pretty text-[15px] leading-6 text-dim">{slide.body}</p>
          </li>
        ))}
      </ul>
    </div>
  );
}
