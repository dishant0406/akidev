"use client";

/* eslint-disable @next/next/no-img-element -- local pictures at their exact size: the image optimizer would only resize them */
import { useEffect, useRef, useState, type CSSProperties, type ReactNode } from "react";

import { Phone } from "@/components/phone";
import type { PhoneScene, Pose } from "@/components/phone-scene";
import type { Slide } from "@/components/screen-gallery";
import { cn } from "@/lib/utils";

const LABEL = "Akidev on an iPhone, turning through its screens as the page scrolls";

// Where the phone is for the opening and for each step: across, up, turn, tilt and size. Between two steps it is in
// between, so it moves the whole time the page scrolls. Step 0 sits low under the opening words (see `update`).
const POSES: Pose[] = [
  { x: 0, y: -0.7, yaw: -0.25, pitch: 0.05, scale: 0.9 },
  { x: 0.5, y: 0, yaw: -0.5, pitch: 0.06, scale: 1.5 },
  { x: -0.5, y: 0, yaw: 0.5, pitch: 0.02, scale: 1.65 },
  { x: 0.5, y: -0.03, yaw: -0.2, pitch: -0.04, scale: 1.8 },
  { x: -0.5, y: 0, yaw: 0.5, pitch: 0.05, scale: 1.55 },
  { x: 0.5, y: 0, yaw: -0.5, pitch: 0, scale: 1.7 },
  { x: -0.5, y: 0, yaw: 0.4, pitch: 0.03, scale: 1.6 },
];
// Which side the words take for each step; the phone takes the other.
const SIDE = ["center", "left", "right", "left", "right", "left", "right"] as const;
// Each step holds its pose for the first part of its scroll, then the phone moves to the next one in the rest. The move is one
// smooth turn to the next side. The phone is large, so each screen nearly fills the window.
const HOLD = 0.5;
// The stage changes colour with the story, in flat colours: the app's white and grey, its Night theme for books, and its
// yellow (the highlighter) for read aloud.
const LOOK = [
  { bg: "rgb(var(--canvas))", fg: "rgb(var(--ink))", dim: "rgb(var(--dim))" },
  { bg: "rgb(var(--canvas))", fg: "rgb(var(--ink))", dim: "rgb(var(--dim))" },
  { bg: "rgb(var(--surface))", fg: "rgb(var(--ink))", dim: "rgb(var(--dim))" },
  { bg: "rgb(var(--canvas))", fg: "rgb(var(--ink))", dim: "rgb(var(--dim))" },
  { bg: "rgb(var(--surface))", fg: "rgb(var(--ink))", dim: "rgb(var(--dim))" },
  { bg: "#1C1C1E", fg: "#F5F5F5", dim: "#A1A1A8" },
  { bg: "rgb(var(--accent))", fg: "rgb(var(--ink))", dim: "#3A3A3E" },
];

const clamp = (x: number, a = 0, b = 1) => Math.min(b, Math.max(a, x));
const ease = (u: number) => u * u * (3 - 2 * u);
const mix = (a: number, b: number, u: number) => a + (b - a) * u;
const compact = () => window.innerWidth < 768;
/** The same picture at half the size, for narrow screens: it looks the same there, and takes a quarter of the graphics memory. */
const half = (src: string) => src.replace(".webp", "-m.webp");
/** A colour between two CSS colours. `u` is 0 for `a` and 1 for `b`. */
const blend = (a: string, b: string, u: number) => (u <= 0 ? a : u >= 1 ? b : `color-mix(in srgb, ${a}, ${b} ${Math.round(u * 100)}%)`);
/** Once the opening has faded out, its buttons leave the tab order (the heading stays). */
const fade0 = (f: number) => f > 0.45;
/** A pose for this screen. On a narrow one the phone stays in the middle and smaller, with the title above it and the text below. */
const poseFor = (index: number, narrow: boolean): Pose => {
  const p = POSES[index];
  return narrow ? { ...p, x: p.x * 0.15, y: index === 0 ? p.y : 0, scale: index === 0 ? p.scale : 1.05 + (p.scale - 1) * 0.15 } : p;
};
/**
 * Whether step `i`'s words show when the scroll is at `f`. They show while the phone rests at its pose. They go as soon as
 * it starts to move (the second half of a step) and come back a moment after it has arrived, so it never passes over them.
 */
const showing = (f: number, i: number, last: boolean) => f >= i - 0.12 && (last || f <= i + HOLD - 0.04);

/**
 * The whole page's main show. A tall block of scroll holds one full-screen stage that stays on screen. The opening
 * words are its first picture, with the phone rising under them. As you scroll, the phone flies across the stage, turns
 * and grows, the screen on it changes, giant words sit behind it, and the stage changes colour. The page first draws a
 * flat phone, so it is ready at once and works everywhere. When the browser is idle, and only on a device that can run
 * WebGL, a real 3D phone takes its place; you can also drag it. For a visitor who asks for less motion there is no
 * stage: the opening words and a plain row of screens (see page.tsx).
 */
export function FeatureStory({ steps, heading, actions }: { steps: Slide[]; heading: ReactNode; actions: ReactNode }) {
  const count = steps.length;
  const track = useRef<HTMLDivElement>(null);
  const stage = useRef<HTMLDivElement>(null);
  const host = useRef<HTMLDivElement>(null);
  const hero = useRef<HTMLDivElement>(null);
  const flatPhone = useRef<HTMLDivElement>(null);
  const buttons = useRef<HTMLDivElement>(null);
  const titles = useRef<(HTMLElement | null)[]>([]);
  const bodies = useRef<(HTMLElement | null)[]>([]);
  const shown = useRef<boolean[]>([]);
  const scene = useRef<PhoneScene | null>(null);
  const reduced = useRef(false);
  const [threeD, setThreeD] = useState(false);
  const [active, setActive] = useState(0);
  const [narrow, setNarrow] = useState(false);

  // Where the scroll has the show: 0 to `count`, in steps. It moves the phone and picks the screen.
  const update = () => {
    const el = track.current;
    const view = stage.current;
    if (!el || !view || reduced.current) return;
    const box = el.getBoundingClientRect();
    const stepPx = (box.height - window.innerHeight) / (count + HOLD);
    const scrolled = stepPx > 0 ? Math.max(0, -box.top / stepPx) : 0; // in steps, past the last one too
    const f = Math.min(scrolled, count);
    const k = Math.min(count - 1, Math.floor(f));
    const v = clamp((f - k - HOLD) / (1 - HOLD)); // 0 to 1 through the move
    const u = ease(v);
    const small = compact();
    // On a narrow screen the phone is sized to the room between the title (top) and the words (bottom), whatever the height.
    const fit = (pose: Pose, index: number): Pose => (small && index > 0 && scene.current ? { ...pose, scale: 0.6 / scene.current.heightFraction() } : pose);
    const a = fit(poseFor(k, small), k);
    const b = fit(poseFor(k + 1, small), k + 1);
    // At the start the phone sits just under the opening words, whatever size the window is.
    const room = scene.current && hero.current ? (hero.current.getBoundingClientRect().bottom - view.getBoundingClientRect().top + 24) / view.clientHeight : null;
    const first = (pose: Pose): Pose => {
      if (room === null || !scene.current) return pose;
      const centre = room + (scene.current.heightFraction() * pose.scale) / 2;
      return { ...pose, y: (0.5 - centre) * 2 };
    };
    const from = k === 0 ? first(a) : a;
    // The flat placeholder sits under the opening words too, so it never covers them.
    const flatBox = flatPhone.current;
    if (flatBox && hero.current) {
      const top = hero.current.getBoundingClientRect().bottom - view.getBoundingClientRect().top + 24;
      const down = Math.max(0, top + flatBox.offsetHeight / 2 - view.clientHeight / 2);
      flatBox.style.setProperty("--fy", `${down}px`);
    }
    const base = { x: mix(from.x, b.x, u), y: mix(from.y, b.y, u), yaw: mix(from.yaw, b.yaw, u), pitch: mix(from.pitch, b.pitch, u), scale: mix(from.scale, b.scale, u) };
    scene.current?.setPose(base);
    // The colour of the whole page follows the scroll: it moves from step to step as the phone moves, and goes back to the
    // page's white while the last step rests, so the tour ends on plain white with no box and no edge. (A browser that
    // cannot mix colours keeps the last colour it understood.)
    const bg = scrolled <= count ? blend(LOOK[k].bg, LOOK[k + 1].bg, u) : blend(LOOK[count].bg, "rgb(var(--canvas))", clamp((scrolled - count) / HOLD));
    document.documentElement.style.backgroundColor = bg;
    document.body.style.backgroundColor = bg;
    // The screen and the words change in the middle of the move.
    const index = Math.min(count, Math.floor(f + 0.5));
    scene.current?.setScreen(Math.max(0, index - 1));
    setActive((current) => (current === index ? current : index));
    setNarrow(small);
    // The opening's words and buttons go as soon as the story starts.
    for (let i = 1; i <= count; i++) {
      const on = showing(f, i, i === count);
      if (shown.current[i] === on) continue;
      shown.current[i] = on;
      for (const el of [titles.current[i - 1], bodies.current[i - 1]]) {
        if (!el) continue;
        // Out at once; in after the phone has had time to arrive.
        el.style.transition = `opacity 0.4s ease-out ${on ? 0.1 : 0}s, transform 0.5s ease-out ${on ? 0.1 : 0}s`;
        el.style.opacity = on ? "1" : "0";
        el.style.transform = on ? "none" : "translateY(1.25rem)";
      }
    }
    if (hero.current) {
      const fade = clamp(1 - (f - 0.05) / 0.35);
      hero.current.style.opacity = String(fade);
      hero.current.style.transform = `translateY(${(1 - fade) * -2}rem)`;
      hero.current.style.pointerEvents = fade < 0.4 ? "none" : "";
    }
    if (buttons.current) buttons.current.inert = fade0(f);
  };

  useEffect(() => {
    reduced.current = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    if (reduced.current) return;

    const webgl = (() => {
      try {
        return Boolean(document.createElement("canvas").getContext("webgl2"));
      } catch {
        return false;
      }
    })();
    const saveData = (navigator as Navigator & { connection?: { saveData?: boolean } }).connection?.saveData;

    let cancelled = false;
    const start = async () => {
      const { createPhoneScene } = await import("@/components/phone-scene");
      if (cancelled || !host.current || !stage.current) return;
      scene.current = createPhoneScene(
        host.current,
        stage.current,
        steps.map((step) => (compact() ? half(step.screen) : step.screen)),
      );
      setThreeD(true);
      update();
    };
    // Wait until the browser is idle, so the 3D phone never slows the first picture.
    const idle = window as Window & { requestIdleCallback?: (run: () => void) => number; cancelIdleCallback?: (id: number) => void };
    const handle = webgl && !saveData ? (idle.requestIdleCallback ? idle.requestIdleCallback(() => void start()) : window.setTimeout(() => void start(), 600)) : null;

    let frame = 0;
    const onScroll = () => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(update);
    };
    update();
    window.addEventListener("scroll", onScroll, { passive: true });
    window.addEventListener("resize", onScroll);
    return () => {
      cancelled = true;
      cancelAnimationFrame(frame);
      window.removeEventListener("scroll", onScroll);
      window.removeEventListener("resize", onScroll);
      if (handle !== null) {
        if (idle.cancelIdleCallback) idle.cancelIdleCallback(handle);
        else window.clearTimeout(handle);
      }
      scene.current?.dispose();
      scene.current = null;
      document.documentElement.style.backgroundColor = "";
      document.body.style.backgroundColor = "";
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps -- starts once; `update` reads only refs and the step count
  }, []);

  // The stage works only while it is on screen.
  useEffect(() => {
    const el = track.current;
    if (!el) return;
    const observer = new IntersectionObserver(([entry]) => scene.current?.setActive(entry.isIntersecting), { rootMargin: "120px" });
    observer.observe(el);
    return () => observer.disconnect();
  }, [threeD]);

  // The tour moves one step for each scroll. While it is on screen the page snaps to the steps (the markers below are the
  // snap points), which handles touch and the keyboard. A wheel or a trackpad is handled here, because a hard flick would
  // otherwise carry the page past several steps: one gesture goes to the next step, whatever its speed.
  useEffect(() => {
    const el = track.current;
    if (!el || window.matchMedia("(prefers-reduced-motion: reduce)").matches) return;
    const root = document.documentElement;
    // Snapping is on only while the stage is pinned, from the first step to the end of the last one. Above and below that
    // the page scrolls freely, so nothing pulls a visitor back as they leave the tour or come back into it.
    const syncSnap = () => {
      const box = el.getBoundingClientRect();
      const f = -box.top / ((box.height - window.innerHeight) / (count + HOLD));
      root.style.scrollSnapType = f >= -0.02 && f <= count + HOLD - 0.02 ? "y mandatory" : "";
    };
    syncSnap();
    window.addEventListener("scroll", syncSnap, { passive: true });
    window.addEventListener("resize", syncSnap);

    let lockedUntil = 0;
    let lastWheel = 0;
    const onWheel = (event: WheelEvent) => {
      if (event.ctrlKey || Math.abs(event.deltaY) < 4) return; // a zoom, or a sideways or tiny move
      const box = el.getBoundingClientRect();
      const stepPx = (box.height - window.innerHeight) / (count + HOLD);
      const f = -box.top / stepPx; // where the scroll is, in steps
      const down = event.deltaY > 0;
      if (f < -0.02 || f > count + HOLD + 0.02) return; // outside the tour: the page scrolls as usual
      if (!down && f <= 0.02) return; // already at the top
      event.preventDefault();
      const now = performance.now();
      const newGesture = now - lastWheel > 120; // a trackpad keeps sending events after the finger lifts
      lastWheel = now;
      if (!newGesture || now < lockedUntil) return;
      lockedUntil = now + 750;
      const target = down ? Math.floor(f + 0.02) + 1 : Math.min(count, Math.ceil(f - 0.02) - 1);
      const start = window.scrollY + box.top;
      const top = target > count ? start + box.height : start + target * stepPx;
      window.scrollTo({ top, behavior: "smooth" });
    };
    window.addEventListener("wheel", onWheel, { passive: false });
    return () => {
      root.style.scrollSnapType = "";
      window.removeEventListener("scroll", syncSnap);
      window.removeEventListener("resize", syncSnap);
      window.removeEventListener("wheel", onWheel);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps -- starts once; it reads only the step count
  }, []);

  const look = LOOK[active];
  // The flat phone is only the placeholder (and the fallback without WebGL): it stays small, so it never reaches the words.
  const flat = { ...poseFor(active, narrow), scale: 1 };
  const stageStyle = { color: look.fg, "--dim": look.dim, touchAction: "pan-y" } as CSSProperties;
  const words = steps.map((step, i) => ({ step, i: i + 1, side: SIDE[i + 1] }));

  return (
    <section aria-label="Akidev for iPhone and Android" className="relative">
      <div ref={track} className="relative [--step:55svh] motion-reduce:!h-auto md:[--step:70svh]" style={{ height: `calc(100svh + ${count + HOLD} * var(--step))` }}>
        <div
          ref={stage}
          className="sticky top-0 h-dvh overflow-hidden transition-colors duration-700 ease-out motion-reduce:static motion-reduce:h-auto motion-reduce:overflow-visible"
          style={stageStyle}
        >
          <h2 className="sr-only">A tour of the app</h2>

          {/* The title of each step, large, on its own side. The phone is on the other side, so they never meet. */}
          {words.map(({ step, i, side }) => (
            <div
              key={step.id}
              aria-hidden
              className={cn(
                "pointer-events-none absolute inset-0 z-0 flex items-center px-[6vw] max-md:items-start max-md:px-5 max-md:pt-[6svh] motion-reduce:hidden",
                side === "left" ? "justify-start text-left" : "justify-end text-right",
                "max-md:justify-center max-md:text-center",
              )}
            >
              <span
                ref={(el) => void (titles.current[i - 1] = el)}
                style={{ opacity: 0 }}
                className="block max-w-[36vw] text-[clamp(2.5rem,7vw,6.75rem)] font-semibold leading-[0.98] tracking-[-0.03em] max-md:max-w-[88vw] max-md:text-[clamp(2.25rem,11vw,3.5rem)]"
              >
                {step.title}
              </span>
            </div>
          ))}

          {/* The flat phone: the first picture, and the only one where WebGL is missing. */}
          <div
            role="img"
            aria-label={LABEL}
            className={cn("pointer-events-none absolute inset-0 z-10 flex items-center justify-center transition-opacity duration-500 motion-reduce:hidden", threeD && "opacity-0")}
          >
            <div ref={flatPhone} style={{ transform: `translate(${flat.x * 50}vw, ${active === 0 ? "var(--fy, 35svh)" : `${-flat.y * 50}svh`}) scale(${flat.scale})`, transition: "transform 0.8s cubic-bezier(0.32, 0.72, 0, 1)" }}>
              <Phone width="var(--pw)" className="[--pw:8.5rem] md:[--pw:13rem]">
                {steps.map((step, i) => (
                  // Only the picture on show and the next one: the 3D phone fetches the rest itself, one after the other.
                  threeD || i > Math.max(0, active - 1) + 1 ? null : <img
                    key={step.id}
                    src={step.screen}
                    srcSet={`${half(step.screen)} 462w, ${step.screen} 924w`}
                    sizes="(max-width: 767px) 9rem, 14rem"
                    alt=""
                    width={924}
                    height={2160}
                    decoding="async"
                    className={cn("absolute inset-0 h-full w-full object-cover transition-opacity duration-500", Math.max(0, active - 1) === i ? "opacity-100" : "opacity-0")}
                  />
                ))}
              </Phone>
            </div>
          </div>
          <div ref={host} aria-hidden className={cn("pointer-events-none absolute inset-0 z-10 transition-opacity duration-500 motion-reduce:hidden", threeD ? "opacity-100" : "opacity-0")} />

          {/* The opening: its words and buttons. */}
          <div
            ref={hero}
            className="absolute inset-x-0 top-0 z-20 flex flex-col items-center px-5 pt-9 text-center md:pt-12 motion-reduce:relative motion-reduce:py-16"
          >
            <div>{heading}</div>
            <div ref={buttons}>{actions}</div>
          </div>

          {/* The words of each step. Every step is in the page, so a screen reader reads them all in order; one shows at a time. */}
          <ol className="motion-reduce:hidden">
            {words.map(({ step, i, side }) => (
              <li
                key={step.id}
                ref={(el) => void (bodies.current[i - 1] = el)}
                style={{ opacity: 0 }}
                className={cn(
                  "pointer-events-none absolute inset-x-0 bottom-[7svh] z-20 px-5 text-center md:bottom-[14svh] md:inset-x-auto md:max-w-[26rem] md:px-0",
                  side === "left" ? "md:left-[6vw] md:text-left" : "md:right-[6vw] md:text-right",
                )}
              >
                <p className="sr-only">{step.title}.</p>
                <p className="mx-auto max-w-[24rem] text-pretty text-[16px] leading-[1.5] text-[color:var(--dim)] md:text-[19px]">{step.body}</p>
              </li>
            ))}
          </ol>
        </div>
        {/* The snap points: the start of each step, and the end of the last one. `snap-always` stops a fast scroll from passing one. */}
        {[...Array.from({ length: count + 1 }, (_, i) => i), count + HOLD].map((at) => (
          <span key={at} aria-hidden className="pointer-events-none absolute left-0 h-px w-px snap-start snap-always motion-reduce:hidden" style={{ top: `calc(${at} * var(--step))` }} />
        ))}
      </div>
    </section>
  );
}
