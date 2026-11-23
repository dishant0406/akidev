"use client";

import { APP_STORE_URL, PLAY_STORE_URL } from "@/constants/site";
import { track } from "@/lib/analytics";

const STORES = {
  ios: { small: "Download on the", name: "App Store", href: APP_STORE_URL },
  android: { small: "Get it on", name: "Google Play", href: PLAY_STORE_URL },
};

/** The Apple mark (Simple Icons, CC0). */
function AppleMark() {
  return (
    <svg viewBox="0 0 24 24" className="size-7 shrink-0" fill="currentColor" aria-hidden>
      <path d="M12.152 6.896c-.948 0-2.415-1.078-3.96-1.04-2.04.027-3.91 1.183-4.961 3.014-2.117 3.675-.546 9.103 1.519 12.09 1.013 1.454 2.208 3.09 3.792 3.039 1.52-.065 2.09-.987 3.935-.987 1.831 0 2.35.987 3.96.948 1.637-.026 2.676-1.48 3.676-2.948 1.156-1.688 1.636-3.325 1.662-3.415-.039-.013-3.182-1.221-3.22-4.857-.026-3.04 2.48-4.494 2.597-4.559-1.429-2.09-3.623-2.324-4.39-2.376-2-.156-3.675 1.09-4.61 1.09zM15.53 3.83c.843-1.012 1.4-2.427 1.245-3.83-1.207.052-2.662.805-3.532 1.818-.78.896-1.454 2.338-1.273 3.714 1.338.104 2.715-.688 3.559-1.701" />
    </svg>
  );
}

/** A play triangle cut into four facets, drawn in one colour. */
function PlayMark() {
  return (
    <svg viewBox="0 0 24 24" className="size-7 shrink-0" aria-hidden>
      <path
        fill="currentColor"
        d="M5.2 2.6c-.5.3-.8.8-.8 1.5v15.8c0 .7.3 1.2.8 1.5.5.3 1.1.2 1.7-.1l13.6-7.9c.5-.3.8-.8.8-1.3s-.3-1-.8-1.3L6.9 2.7c-.6-.3-1.2-.4-1.7-.1z"
      />
      <path d="M4.5 3.4 13.1 12l-8.6 8.6M13.1 12l2.9-2.9M13.1 12l2.9 2.9" fill="none" stroke="rgb(255 210 31)" strokeWidth=".7" strokeLinejoin="round" />
    </svg>
  );
}

/** A link to the app in one of its two stores. It tells the site's own counter which store was asked for. */
export function StoreButton({ store }: { store: keyof typeof STORES }) {
  const { small, name, href } = STORES[store];
  return (
    <a
      href={href}
      target="_blank"
      rel="noopener"
      onClick={() => track("download_clicked", { platform: store, version: null })}
      className="group inline-flex items-center justify-center gap-2.5 rounded-xl bg-[rgb(255_210_31)] py-3 pl-3 pr-3.5 sm:gap-3 sm:pl-4 sm:pr-6 text-[#141414] shadow-[0_10px_30px_-10px_rgb(255_210_31/0.55)] transition-[transform,box-shadow] duration-200 ease-spring hover:-translate-y-0.5 hover:shadow-[0_16px_40px_-10px_rgb(255_210_31/0.7)] active:scale-[0.97] motion-reduce:transition-none"
    >
      {store === "ios" ? <AppleMark /> : <PlayMark />}
      <span className="flex flex-col text-left leading-none">
        <span className="text-[11px] font-medium opacity-70">{small}</span>
        <span className="mt-1 text-[17px] font-bold sm:text-[19px] tracking-[-0.01em]">{name}</span>
      </span>
    </a>
  );
}
