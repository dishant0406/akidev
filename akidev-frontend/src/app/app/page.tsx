/* eslint-disable @next/next/no-img-element -- local pictures at their exact size: the image optimizer would only resize them */
import type { Metadata } from "next";

import { AppShowcase, type Step } from "@/components/app-showcase";
import { StoreButton } from "@/components/store-button";
import { APP_HOST, REPO_URL, SITE_DOMAIN } from "@/constants/site";

const TITLE = "Akidev for iPhone and Android | A reader for manga, comics and books";
const DESCRIPTION = "A calm reader for manga, comics and EPUB books, with read-aloud. Free on the App Store and Google Play.";

// This page is the whole of its own host, so its pictures and its address are on that host.
export const metadata: Metadata = {
  metadataBase: new URL(`https://${APP_HOST}`),
  alternates: { canonical: "/" },
  title: TITLE,
  description: DESCRIPTION,
  openGraph: { type: "website", siteName: "Akidev", title: TITLE, description: DESCRIPTION, images: ["/phone/og.jpg"] },
  twitter: { card: "summary_large_image", title: TITLE, description: DESCRIPTION, images: ["/phone/og.jpg"] },
};

// The words are the app's own: its screens and its store listing. The pictures are the film's made-up art.
const STEPS: Step[] = [
  {
    id: "sources",
    label: "Sources",
    title: "Add a source. Find something to read.",
    body: "Akidev comes with no manga. You add a source by its address, then see what is popular and new. Add only sources you trust.",
    screen: "/phone/discover.webp",
  },
  {
    id: "series",
    label: "Series",
    title: "Open a series. Pick up where you stopped.",
    body: "See every chapter and what is still unread. Save a series to your library with one tap.",
    screen: "/phone/series.webp",
  },
  {
    id: "reading",
    label: "Reading",
    title: "Read the way you like.",
    body: "Scroll one long strip, turn pages, or read right to left. Set the speed of auto-scroll and the color behind the page.",
    chips: ["Strip", "Paged", "Right to left"],
    screen: "/phone/reader.webp",
  },
  {
    id: "library",
    label: "Library",
    title: "Your library keeps your place.",
    body: "Sort your series into categories. Get an alert when a new chapter arrives. Download chapters to read offline.",
    chips: ["Categories", "History", "New chapter alerts", "Downloads"],
    screen: "/phone/library.webp",
  },
  {
    id: "books",
    label: "Books",
    title: "Books too. Pages, themes and fonts.",
    body: "Open EPUB files from your phone, or add a catalogue by its address. Choose pages or scrolling. Add highlights and notes, and search the text.",
    chips: ["Auto", "Paper", "Sepia", "Ink", "Night", "Black"],
    screen: "/phone/themes.webp",
  },
  {
    id: "aloud",
    label: "Read aloud",
    title: "Let it read to you.",
    body: "A voice that runs on your phone reads the page, and the sentence is marked as you listen. Choose the speed that suits you.",
    screen: "/phone/aloud.webp",
  },
];

const PRIVATE = ["No accounts", "No ads", "No analytics", "No tracking"];

// The landing page has no site around it, so a link to a page of the site names the site's host.
const site = (path: string) => `https://${SITE_DOMAIN}${path}`;
const link = "text-ink underline decoration-ink/30 underline-offset-4 transition-colors hover:decoration-ink";

export default function AppPage() {
  return (
    <>
      <AppShowcase steps={STEPS} />

      <section className="gutter mx-auto max-w-6xl py-24 md:py-36">
        <p className="caps text-[rgb(255_210_31)]">Private by design</p>
        <h2 className="mt-4 max-w-3xl text-balance font-sans text-[clamp(2rem,5vw,3.75rem)] font-extrabold leading-[1.04] tracking-[-0.035em] text-ink">
          Everything you read and save stays on your phone.
        </h2>
        <ul className="mt-12 grid gap-px overflow-hidden rounded-xl bg-line sm:grid-cols-2 lg:grid-cols-4">
          {PRIVATE.map((item) => (
            <li key={item} className="bg-canvas px-6 py-10 font-sans text-2xl font-bold tracking-[-0.02em] text-ink md:py-14">
              {item}
            </li>
          ))}
        </ul>
        <p className="mt-6 max-w-xl text-[15px] leading-7 text-dim">
          Back it up to a file when you want to move it to another phone. The{" "}
          <a href={site("/manga/privacy")} className={link}>
            privacy policy
          </a>{" "}
          says all of it.
        </p>
      </section>

      <section className="relative overflow-hidden border-t border-line">
        <div aria-hidden className="pointer-events-none absolute left-1/2 top-0 size-[44rem] -translate-x-1/2 -translate-y-1/2 rounded-full bg-[rgb(255_210_31)] opacity-[0.07] blur-[120px]" />
        <div className="gutter relative mx-auto flex max-w-3xl flex-col items-center py-24 text-center md:py-36">
          <img
            src="/phone/app-icon.png"
            alt=""
            width={112}
            height={112}
            className="size-28 rounded-[22%] shadow-[0_24px_60px_-12px_rgb(255_210_31/0.45)] ring-1 ring-ink/10"
          />
          <h2 className="mt-8 font-sans text-[clamp(2.4rem,6vw,4.5rem)] font-extrabold leading-[1.02] tracking-[-0.035em] text-ink">Get Akidev.</h2>
          <p className="mt-4 max-w-md text-pretty text-[17px] leading-7 text-dim">Free on the App Store and Google Play.</p>
          <div className="mt-8 flex flex-wrap justify-center gap-3">
            <StoreButton store="ios" />
            <StoreButton store="android" />
          </div>
          <p className="mt-10 text-[15px] text-dim">
            On a computer?{" "}
            <a href={site("/download")} className={link}>
              Get the desktop app
            </a>
            .
          </p>
        </div>
      </section>

      <section className="gutter mx-auto flex max-w-6xl flex-col gap-3 border-t border-line py-10 text-xs leading-5 text-faint">
        <p className="max-w-3xl">
          Akidev does not include, offer or endorse any source or catalogue. Add only what you have the right to read. The pictures on
          these screens are made-up art for this page, not real series. Icons: Solar by 480 Design (CC BY 4.0).
        </p>
        <nav className="flex flex-wrap gap-x-6 text-[13px]">
          <a href={site("/manga/support")} className="py-2 transition-colors hover:text-ink">
            Help and answers
          </a>
          <a href={REPO_URL} className="py-2 transition-colors hover:text-ink" rel="noopener">
            Open source
          </a>
        </nav>
      </section>
    </>
  );
}
