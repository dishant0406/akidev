"use client";

import { Pause, Play } from "lucide-react";
import { useEffect, useRef, useState } from "react";

import { Phone } from "@/components/phone";
import type { PhoneScene } from "@/components/phone-scene";
import { cn } from "@/lib/utils";

const LABEL = "Akidev on an iPhone, playing a short tour of the app: a source, a series, the reader and back";

/**
 * The big panel under the opening words. It holds one phone that plays the app. The page first draws a flat phone, so
 * it is ready at once and works everywhere. Once the page is idle, and only on a device that can run WebGL, a 3D phone
 * takes its place: you can drag it, the page scroll turns it, and the same video plays on its screen.
 */
export function HeroStage() {
  const panel = useRef<HTMLDivElement>(null);
  const host = useRef<HTMLDivElement>(null);
  const video = useRef<HTMLVideoElement>(null);
  const scene = useRef<PhoneScene | null>(null);
  const reduced = useRef(false);
  const [threeD, setThreeD] = useState(false);
  const [paused, setPaused] = useState(false);
  const [visible, setVisible] = useState(true);

  useEffect(() => {
    reduced.current = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    if (reduced.current) setPaused(true);

    const webgl = (() => {
      try {
        return Boolean(document.createElement("canvas").getContext("webgl2"));
      } catch {
        return false;
      }
    })();
    const saveData = (navigator as Navigator & { connection?: { saveData?: boolean } }).connection?.saveData;
    if (!webgl || saveData) return;

    let cancelled = false;
    const start = async () => {
      const { createPhoneScene } = await import("@/components/phone-scene");
      if (cancelled || !host.current || !video.current) return;
      scene.current = createPhoneScene(host.current, video.current);
      setThreeD(true);
    };
    // Wait until the browser is idle, so the 3D phone never slows the first picture.
    const idle = window as Window & { requestIdleCallback?: (run: () => void) => number; cancelIdleCallback?: (id: number) => void };
    const handle = idle.requestIdleCallback ? idle.requestIdleCallback(() => void start()) : window.setTimeout(() => void start(), 600);
    return () => {
      cancelled = true;
      if (idle.cancelIdleCallback) idle.cancelIdleCallback(handle);
      else window.clearTimeout(handle);
      scene.current?.dispose();
      scene.current = null;
    };
  }, []);

  // The stage works only while it is on screen, and the page scroll turns the phone as it passes.
  useEffect(() => {
    const el = panel.current;
    if (!el) return;
    const observer = new IntersectionObserver(([entry]) => setVisible(entry.isIntersecting), { rootMargin: "80px" });
    observer.observe(el);
    const onScroll = () => {
      if (reduced.current) return;
      const box = el.getBoundingClientRect();
      const progress = (window.innerHeight - box.top) / (window.innerHeight + box.height);
      scene.current?.setTurn(progress * 2 - 1);
    };
    onScroll();
    window.addEventListener("scroll", onScroll, { passive: true });
    return () => {
      observer.disconnect();
      window.removeEventListener("scroll", onScroll);
    };
  }, [threeD]);

  useEffect(() => {
    scene.current?.setActive(visible);
    if (visible && !paused) video.current?.play().catch(() => {});
    else video.current?.pause();
  }, [visible, paused, threeD]);

  return (
    <div>
      <div ref={panel} className="relative h-[30rem] overflow-hidden rounded-2xl bg-surface md:h-[40rem]">
        <div role="img" aria-label={LABEL} className="absolute inset-0">
          {/* The flat phone: the first picture, and the only one where WebGL is missing. */}
          <div className={cn("absolute inset-0 flex items-center justify-center transition-opacity duration-500", threeD && "opacity-0")}>
            <Phone width="var(--pw)" className="[--pw:10rem] md:[--pw:14rem]">
              <video ref={video} className="absolute inset-0 h-full w-full object-cover" src="/phone/app-loop.mp4" poster="/phone/app-poster.webp" muted loop playsInline />
            </Phone>
          </div>
          <div
            ref={host}
            className={cn("absolute inset-0 transition-opacity duration-500", threeD ? "opacity-100" : "pointer-events-none opacity-0")}
          />
          {threeD && (
            <div aria-hidden className="pointer-events-none absolute bottom-[7%] left-1/2 h-3 w-28 -translate-x-1/2 rounded-[50%] bg-black/25 blur-md md:w-40" />
          )}
        </div>
        <button
          type="button"
          onClick={() => setPaused((value) => !value)}
          aria-label={paused ? "Play the tour" : "Pause the tour"}
          className="absolute bottom-3 right-3 flex size-11 items-center justify-center rounded-full border border-line bg-canvas text-ink transition-transform duration-150 ease-out active:scale-[0.96] motion-reduce:transition-none [@media(hover:hover)]:hover:bg-wash"
        >
          {paused ? <Play aria-hidden className="size-[18px]" /> : <Pause aria-hidden className="size-[18px]" />}
        </button>
      </div>
      {threeD && (
        <p className="mt-3 text-center text-[13px] leading-5 text-faint">
          <span className="[@media(pointer:coarse)]:hidden">Drag to turn the phone.</span>
          <span className="hidden [@media(pointer:coarse)]:inline">Swipe sideways to turn the phone.</span>
        </p>
      )}
    </div>
  );
}
