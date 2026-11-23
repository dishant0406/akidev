"use client";

/* eslint-disable @next/next/no-img-element -- local pictures at their exact size: the image optimizer would only resize them */
import { useEffect, useRef, useState, type PointerEvent } from "react";

import { StoreButton } from "@/components/store-button";
import { cn } from "@/lib/utils";

export type Step = { id: string; label: string; title: string; body: string; chips?: string[]; screen: string };

/** How far the phone turns for each part of the page: [around the upright axis, around the sideways axis], in degrees. */
const TURN: [number, number][] = [[-14, 5], [10, 3], [-9, 4], [11, 3], [-10, 4], [8, 3], [-8, 4]];
/** The most the phone follows the pointer, in degrees. */
const FOLLOW = [7, 5];

const yellow = "text-[rgb(255_210_31)]";

/**
 * One phone that stays on screen while the text beside it scrolls. The screen on the phone follows the text: a
 * recording of the app first, then a picture of each part. The phone turns a little for each part and follows the
 * pointer on a spring, so it can be pushed again at any moment and carries its speed.
 */
export function AppShowcase({ steps }: { steps: Step[] }) {
  const [active, setActive] = useState(0);
  const [wide, setWide] = useState(false);
  const stepEls = useRef<(HTMLElement | null)[]>([]);
  const phone = useRef<HTMLDivElement>(null);
  const sheen = useRef<HTMLDivElement>(null);
  const video = useRef<HTMLVideoElement>(null);
  const reduced = useRef(false);
  const spring = useRef({ pos: [0, 0], vel: [0, 0], target: [0, 0], pointer: [0, 0], frame: 0, last: 0 });

  // A wide screen puts the phone beside the text; a narrow one puts it above.
  useEffect(() => {
    const query = window.matchMedia("(min-width: 1024px)");
    const read = () => setWide(query.matches);
    read();
    query.addEventListener("change", read);
    reduced.current = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    return () => query.removeEventListener("change", read);
  }, []);

  // The part that sits in a thin band across the viewport is the active one.
  useEffect(() => {
    const observer = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) if (entry.isIntersecting) setActive(Number((entry.target as HTMLElement).dataset.step));
      },
      { rootMargin: wide ? "-45% 0px -45% 0px" : "-55% 0px -30% 0px" },
    );
    stepEls.current.forEach((el) => el && observer.observe(el));
    return () => observer.disconnect();
  }, [wide]);

  // The recording runs only while it is the screen on show.
  useEffect(() => {
    if (active === 0 && !reduced.current) video.current?.play().catch(() => {});
    else video.current?.pause();
  }, [active]);

  const draw = () => {
    const { pos } = spring.current;
    if (phone.current) phone.current.style.transform = `perspective(1800px) rotateY(${pos[0]}deg) rotateX(${pos[1]}deg)`;
    if (sheen.current) sheen.current.style.backgroundPosition = `${50 + pos[0] * 4}% 0`;
  };

  // A critically damped spring (no bounce, settles in about half a second). It starts from where the phone is now.
  const run = () => {
    const s = spring.current;
    if (reduced.current) {
      s.pos = [...s.target];
      draw();
      return;
    }
    if (s.frame) return;
    s.last = performance.now();
    const tick = (now: number) => {
      const dt = Math.min(0.032, (now - s.last) / 1000);
      s.last = now;
      const w = (2 * Math.PI) / 0.5;
      let moving = false;
      for (let i = 0; i < 2; i++) {
        s.vel[i] += (w * w * (s.target[i] - s.pos[i]) - 2 * w * s.vel[i]) * dt;
        s.pos[i] += s.vel[i] * dt;
        if (Math.abs(s.target[i] - s.pos[i]) > 0.01 || Math.abs(s.vel[i]) > 0.01) moving = true;
      }
      draw();
      s.frame = moving ? requestAnimationFrame(tick) : 0;
    };
    s.frame = requestAnimationFrame(tick);
  };

  const aim = () => {
    const s = spring.current;
    const scale = wide ? 1 : 0.55;
    s.target = [TURN[active][0] * scale + s.pointer[0], TURN[active][1] * scale + s.pointer[1]];
    run();
  };
  useEffect(aim, [active, wide]); // eslint-disable-line react-hooks/exhaustive-deps
  useEffect(() => () => cancelAnimationFrame(spring.current.frame), []);

  const follow = (event: PointerEvent<HTMLDivElement>) => {
    if (event.pointerType !== "mouse" || reduced.current) return;
    const box = event.currentTarget.getBoundingClientRect();
    const x = ((event.clientX - box.left) / box.width - 0.5) * 2;
    const y = ((event.clientY - box.top) / box.height - 0.5) * 2;
    spring.current.pointer = [x * FOLLOW[0], -y * FOLLOW[1]];
    aim();
  };
  const leave = () => {
    spring.current.pointer = [0, 0];
    aim();
  };

  const layer = (index: number) =>
    cn(
      "absolute inset-0 h-full w-full object-cover transition-[opacity,transform] duration-[650ms] ease-spring motion-reduce:!translate-y-0 motion-reduce:duration-200",
      index === active ? "translate-y-0 opacity-100" : index < active ? "-translate-y-[3%] opacity-0" : "translate-y-[3%] opacity-0",
    );

  return (
    <div className="gutter mx-auto grid max-w-6xl lg:grid-cols-[minmax(0,1fr)_auto] lg:gap-x-16">
      {/* The stage: the phone, and the light it throws. */}
      <div
        onPointerMove={follow}
        onPointerLeave={leave}
        className="sticky top-0 z-20 -mx-5 h-[36svh] bg-canvas max-lg:overflow-hidden md:-mx-10 lg:top-0 lg:z-0 lg:col-start-2 lg:row-start-1 lg:mx-0 lg:h-svh lg:self-start lg:bg-transparent"
      >
        <div aria-hidden className="pointer-events-none absolute left-1/2 top-1/2 h-[110%] w-[220%] -translate-x-1/2 -translate-y-1/2 lg:w-[300%]">
          {["/phone/app-poster.webp", ...steps.map((step) => step.screen)].map((src, index) => (
            <img
              key={src}
              src={src}
              alt=""
              className={cn("absolute inset-0 h-full w-full object-cover blur-[90px] saturate-150 transition-opacity duration-700", index === active ? "opacity-40" : "opacity-0")}
            />
          ))}
        </div>
        <div aria-hidden className="pointer-events-none absolute inset-x-0 bottom-0 z-10 h-20 bg-gradient-to-t from-canvas to-transparent lg:hidden" />
        <div aria-hidden className="relative flex h-full items-start justify-center pt-3 [--b:calc(var(--w)*0.036)] [--h:min(52svh,470px)] [--w:calc(var(--h)*0.4278)] lg:items-center lg:pt-0 lg:[--h:min(76svh,780px)]">
          <div
            ref={phone}
            className="relative will-change-transform"
            style={{
              width: "calc(var(--w) + 2 * var(--b))",
              height: "calc(var(--h) + 2 * var(--b))",
              padding: "var(--b)",
              borderRadius: "calc(var(--w) * 0.14 + var(--b))",
              background: "#050506",
              boxShadow: "inset 0 0 0 1.5px #2A2A30, 0 0 0 calc(var(--w) * 0.0077) #5A5A62, 0 50px 120px -20px rgb(0 0 0 / 0.75)",
            }}
          >
            {/* The side buttons: action and volume on the left, power on the right. */}
            {[[110, 32, 0], [175, 62, 0], [250, 62, 0], [205, 100, 1]].map(([top, height, right]) => (
              <span
                key={top}
                className="absolute bg-[#4A4A50]"
                style={{
                  top: `calc(var(--h) * ${top} / 919 + var(--b))`,
                  height: `calc(var(--h) * ${height} / 919)`,
                  width: "calc(var(--w) * 0.012)",
                  borderRadius: "2px",
                  ...(right ? { right: "calc(var(--w) * -0.012)" } : { left: "calc(var(--w) * -0.012)" }),
                }}
              />
            ))}
            <div className="relative h-full w-full overflow-hidden bg-[#0D0D0E]" style={{ borderRadius: "calc(var(--w) * 0.14)" }}>
              <video ref={video} className={layer(0)} src="/phone/app-loop.mp4" poster="/phone/app-poster.webp" muted loop playsInline />
              {steps.map((step, index) => (
                <img key={step.id} src={step.screen} alt="" width={924} height={2160} decoding="async" className={layer(index + 1)} />
              ))}
              <div
                ref={sheen}
                className="pointer-events-none absolute inset-0 mix-blend-soft-light"
                style={{ background: "linear-gradient(115deg, transparent 35%, rgb(255 255 255 / 0.4) 50%, transparent 65%)", backgroundSize: "300% 100%", backgroundPosition: "50% 0" }}
              />
            </div>
          </div>
        </div>
      </div>

      {/* The text: the first part is the page's opening, then one part for each screen. */}
      <div className="relative z-10 pb-[12svh] lg:col-start-1 lg:row-start-1 lg:pb-0">
        <section ref={(el) => void (stepEls.current[0] = el)} data-step={0} className="flex min-h-[70svh] items-start pb-6 pt-8 lg:min-h-svh lg:items-center lg:py-0">
          <div className={cn("max-w-xl transition-opacity duration-500", active === 0 ? "opacity-100" : "opacity-30")}>
            <p className="caps flex animate-rise items-center gap-2.5 text-dim">
              <img src="/phone/app-icon.png" alt="" width={28} height={28} className="size-7 rounded-[22%]" />
              Akidev for iPhone and Android
            </p>
            <h1
              className="mt-5 animate-rise text-balance font-sans text-[clamp(2.3rem,6.4vw,4.75rem)] font-extrabold leading-[1.02] tracking-[-0.035em] text-ink"
              style={{ animationDelay: "80ms" }}
            >
              Manga, comics and books. <span className={cn("inline-block", yellow)}>One calm reader.</span>
            </h1>
            <p className="mt-5 animate-rise text-pretty text-[17px] leading-7 text-dim" style={{ animationDelay: "160ms" }}>
              Akidev has no content of its own. You add the sources and books you already use, and it gives you one place to read them.
            </p>
            <div className="mt-6 grid animate-rise grid-cols-2 gap-3 sm:mt-8 sm:flex sm:flex-wrap" style={{ animationDelay: "240ms" }}>
              <StoreButton store="ios" />
              <StoreButton store="android" />
            </div>
            <p className="mt-5 animate-rise text-[13px] leading-5 text-faint" style={{ animationDelay: "320ms" }}>
              Free. iPhone needs iOS 26 or later. Android needs Android 10 or later.
            </p>
          </div>
        </section>

        {steps.map((step, index) => (
          <section
            key={step.id}
            ref={(el) => void (stepEls.current[index + 1] = el)}
            data-step={index + 1}
            className="flex min-h-[62svh] items-start py-8 lg:min-h-svh lg:items-center lg:py-0"
          >
            <div className={cn("max-w-md transition-opacity duration-500", active === index + 1 ? "opacity-100" : "opacity-30")}>
              <p className={cn("caps", yellow)}>
                {String(index + 1).padStart(2, "0")} · {step.label}
              </p>
              <h2 className="mt-4 text-balance font-sans text-[clamp(1.8rem,4vw,3.1rem)] font-extrabold leading-[1.05] tracking-[-0.03em] text-ink">
                {step.title}
              </h2>
              <p className="mt-4 text-pretty text-[17px] leading-7 text-dim">{step.body}</p>
              {step.chips && (
                <ul className="mt-6 flex flex-wrap gap-2">
                  {step.chips.map((chip) => (
                    <li key={chip} className="rounded-full bg-wash px-3.5 py-1.5 text-[13px] font-semibold text-ink ring-1 ring-ink/10">
                      {chip}
                    </li>
                  ))}
                </ul>
              )}
            </div>
          </section>
        ))}
      </div>
    </div>
  );
}
