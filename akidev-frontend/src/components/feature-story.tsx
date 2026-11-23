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
  { x: 0.42, y: 0, yaw: -0.5, pitch: 0.06, scale: 1.05 },
  { x: -0.42, y: 0, yaw: 0.5, pitch: 0.02, scale: 1.1 },
  { x: 0.3, y: -0.05, yaw: -0.18, pitch: -0.04, scale: 1.5 },
  { x: -0.42, y: 0, yaw: 0.5, pitch: 0.05, scale: 1.08 },
  { x: 0.42, y: 0, yaw: -0.5, pitch: 0, scale: 1.1 },
  { x: -0.4, y: 0, yaw: 0.4, pitch: 0.03, scale: 1.2 },
];
// Which side the words take for each step; the phone takes the other.
const SIDE = ["center", "left", "right", "left", "right", "left", "right"] as const;
// Each step holds its pose for the first half of its scroll, then the phone moves to the next one in the second half.
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
/** Once the opening has faded out, its buttons leave the tab order (the heading stays). */
const fade0 = (f: number) => f > 0.45;
/** A pose for this screen. On a narrow one the phone stays nearer the middle and smaller, so the words have room above and below. */
const poseFor = (index: number, narrow: boolean): Pose => {
  const p = POSES[index];
  return narrow ? { ...p, x: p.x * 0.25, y: index === 0 ? p.y : 0, scale: index === 0 ? p.scale : 0.78 + (p.scale - 1) * 0.2 } : p;
};

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
  const buttons = useRef<HTMLDivElement>(null);
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
    const f = stepPx > 0 ? clamp(-box.top / stepPx, 0, count) : 0;
    const k = Math.min(count - 1, Math.floor(f));
    const u = ease(clamp((f - k - HOLD) / (1 - HOLD)));
    const small = compact();
    const a = poseFor(k, small);
    const b = poseFor(k + 1, small);
    // At the start the phone sits just under the opening words, whatever size the window is.
    const room = scene.current && hero.current ? (hero.current.getBoundingClientRect().bottom - view.getBoundingClientRect().top + 24) / view.clientHeight : null;
    const first = (pose: Pose): Pose => {
      if (room === null || !scene.current) return pose;
      const centre = room + (scene.current.heightFraction() * pose.scale) / 2;
      return { ...pose, y: (0.5 - centre) * 2 };
    };
    const from = k === 0 ? first(a) : a;
    scene.current?.setPose({
      x: mix(from.x, b.x, u),
      y: mix(from.y, b.y, u),
      yaw: mix(from.yaw, b.yaw, u),
      pitch: mix(from.pitch, b.pitch, u),
      scale: mix(from.scale, b.scale, u),
    });
    // The screen and the words change in the middle of the move.
    const index = Math.min(count, Math.floor(f + 0.25));
    scene.current?.setScreen(Math.max(0, index - 1));
    setActive((current) => (current === index ? current : index));
    setNarrow(small);
    // The opening's words and buttons go as soon as the story starts.
    if (hero.current) {
      const fade = clamp(1 - (f - 0.1) / 0.45);
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
        steps.map((step) => step.screen),
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

  const jumpTo = (index: number) => {
    const el = track.current;
    if (!el) return;
    const stepPx = (el.getBoundingClientRect().height - window.innerHeight) / (count + HOLD);
    const top = el.getBoundingClientRect().top + window.scrollY + index * stepPx + 2;
    window.scrollTo({ top, behavior: reduced.current ? "auto" : "smooth" });
  };

  const look = LOOK[active];
  const flat = poseFor(active, narrow);
  const stageStyle = { backgroundColor: look.bg, color: look.fg, "--dim": look.dim, touchAction: "pan-y" } as CSSProperties;
  const words = steps.map((step, i) => ({ step, i: i + 1, side: SIDE[i + 1] }));

  return (
    <section aria-label="Akidev for iPhone and Android" className="relative">
      <div ref={track} className="relative [--step:55svh] motion-reduce:!h-auto md:[--step:70svh]" style={{ height: `calc(100svh + ${count + HOLD} * var(--step))` }}>
        <div
          ref={stage}
          className="sticky top-0 h-svh overflow-hidden transition-colors duration-700 ease-out motion-reduce:static motion-reduce:h-auto motion-reduce:overflow-visible"
          style={stageStyle}
        >
          <h2 className="sr-only">A tour of the app</h2>

          {/* Giant words, behind the phone. */}
          {words.map(({ step, i, side }) => (
            <div
              key={step.id}
              aria-hidden
              className={cn(
                "pointer-events-none absolute inset-0 z-0 flex items-center px-[5vw] transition-[opacity,transform] duration-700 ease-out max-md:items-start max-md:pt-[7svh] motion-reduce:hidden",
                side === "left" && "justify-start text-left",
                side === "right" && "justify-end text-right",
                side === "center" && "justify-center text-center",
                i === active ? "translate-y-0 opacity-100" : "translate-y-8 opacity-0",
              )}
            >
              <span className="block max-w-[10ch] text-[clamp(2.75rem,10.5vw,9.5rem)] font-semibold leading-[0.96] tracking-[-0.035em] md:max-w-[9ch]">{step.title}</span>
            </div>
          ))}

          {/* The flat phone: the first picture, and the only one where WebGL is missing. */}
          <div
            role="img"
            aria-label={LABEL}
            className={cn("pointer-events-none absolute inset-0 z-10 flex items-center justify-center transition-opacity duration-500 motion-reduce:hidden", threeD && "opacity-0")}
          >
            <div style={{ transform: `translate(${flat.x * 50}vw, ${-flat.y * 50}svh) scale(${flat.scale})`, transition: "transform 0.8s cubic-bezier(0.32, 0.72, 0, 1)" }}>
              <Phone width="var(--pw)" className="[--pw:8.5rem] md:[--pw:13rem]">
                {steps.map((step, i) => (
                  <img
                    key={step.id}
                    src={step.screen}
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
                className={cn(
                  "pointer-events-none absolute inset-x-0 bottom-[10svh] z-20 px-5 text-center transition-[opacity,transform] duration-700 ease-out md:bottom-[15svh] md:inset-x-auto md:max-w-[26rem] md:px-0 md:text-left",
                  side === "left" && "md:left-[6vw]",
                  side === "right" && "md:right-[6vw] md:text-right",
                  side === "center" && "md:left-1/2 md:-translate-x-1/2 md:text-center",
                  i === active ? "translate-y-0 opacity-100" : "translate-y-4 opacity-0",
                )}
              >
                <p className="sr-only">{step.title}.</p>
                <p className="mx-auto max-w-[24rem] text-pretty text-[17px] leading-[1.5] text-[color:var(--dim)] md:text-[19px]">{step.body}</p>
              </li>
            ))}
          </ol>

          <nav aria-label="Steps of the tour" className="absolute inset-x-0 bottom-3 z-20 flex justify-center motion-reduce:hidden">
            {Array.from({ length: count + 1 }, (_, i) => (
              <button
                key={i}
                type="button"
                onClick={() => jumpTo(i)}
                aria-label={i === 0 ? "Start of the tour" : steps[i - 1].title}
                aria-current={i === active ? "step" : undefined}
                className="group flex h-11 w-7 items-center justify-center"
              >
                <span className={cn("h-2 rounded-full bg-current transition-[width,opacity] duration-300 ease-out motion-reduce:transition-none", i === active ? "w-6 opacity-100" : "w-2 opacity-30 group-hover:opacity-60")} />
              </button>
            ))}
          </nav>
        </div>
        {/* The snap points: the start of each step, and the end of the last one. `snap-always` stops a fast scroll from passing one. */}
        {[...Array.from({ length: count + 1 }, (_, i) => i), count + HOLD].map((at) => (
          <span key={at} aria-hidden className="pointer-events-none absolute left-0 h-px w-px snap-start snap-always motion-reduce:hidden" style={{ top: `calc(${at} * var(--step))` }} />
        ))}
      </div>
    </section>
  );
}
