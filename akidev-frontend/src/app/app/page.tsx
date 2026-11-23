/* eslint-disable @next/next/no-img-element -- local pictures at their exact size: the image optimizer would only resize them */
import { Plus } from "lucide-react";
import type { Metadata } from "next";

import { AppShowcase, type Step } from "@/components/app-showcase";
import { StoreButton } from "@/components/store-button";
import { APP_HOST, APP_STORE_ID, APP_STORE_URL, PLAY_STORE_URL, REPO_URL, SITE_DOMAIN } from "@/constants/site";

// The words people search for are: manga reader, comic reader, EPUB reader, read aloud, and no ads / no account.
// They are used where they are true, once each, in the title, the description, the opening text and the questions.
const TITLE = "Akidev: Manga, Comic and EPUB Reader with Read Aloud";
const DESCRIPTION = "A calm manga, comic and EPUB reader for iPhone and Android, with read-aloud. No accounts, no ads, no tracking. Free on the App Store and Google Play.";
const IMAGE_ALT = "Akidev on an iPhone: three screens of the reader, the library, a series and a book, on a dark background";

// This page is the whole of its own host, so its pictures and its address are on that host.
export const metadata: Metadata = {
  metadataBase: new URL(`https://${APP_HOST}`),
  alternates: { canonical: "/" },
  title: TITLE,
  description: DESCRIPTION,
  robots: { index: true, follow: true, googleBot: { index: true, follow: true, "max-image-preview": "large", "max-snippet": -1 } },
  // Safari on an iPhone shows its own "Open in the App Store" banner for this.
  other: { "apple-itunes-app": `app-id=${APP_STORE_ID}` },
  openGraph: {
    type: "website",
    siteName: "Akidev",
    locale: "en_US",
    url: "/",
    title: TITLE,
    description: DESCRIPTION,
    images: [
      { url: "/phone/og.jpg", width: 1200, height: 630, alt: IMAGE_ALT },
      { url: "/phone/og-square.jpg", width: 1200, height: 1200, alt: IMAGE_ALT },
    ],
  },
  twitter: { card: "summary_large_image", title: TITLE, description: DESCRIPTION, images: [{ url: "/phone/og-twitter.jpg", alt: IMAGE_ALT }] },
};

// The words are the app's own: its screens and its store listing. The pictures are the film's made-up art.
const STEPS: Step[] = [
  {
    id: "sources",
    label: "Sources",
    title: "Add a source. Find something to read.",
    body: "Akidev comes with no manga. You add a source by its address, then see what is popular and new. Add only sources you trust.",
    screen: "/phone/discover.webp",
    alt: "Akidev Discover screen: a search field for a source and a featured series",
  },
  {
    id: "series",
    label: "Series",
    title: "Open a series. Pick up where you stopped.",
    body: "See every chapter and what is still unread. Save a series to your library with one tap.",
    screen: "/phone/series.webp",
    alt: "Akidev series screen: a cover, the title The Last Lantern, and a list of unread chapters",
  },
  {
    id: "reading",
    label: "Reading",
    title: "Read the way you like.",
    body: "Scroll one long strip, turn pages, or read right to left. Set the speed of auto-scroll and the color behind the page.",
    chips: ["Strip", "Paged", "Right to left"],
    screen: "/phone/reader.webp",
    alt: "Akidev reader settings: reading mode Strip, Paged or Right to left, with a double page behind",
  },
  {
    id: "library",
    label: "Library",
    title: "Your library keeps your place.",
    body: "Sort your series into categories. Get an alert when a new chapter arrives. Download chapters to read offline.",
    chips: ["Categories", "History", "New chapter alerts", "Downloads"],
    screen: "/phone/library.webp",
    alt: "Akidev library: a grid of covers with unread counts, a Weekend category and a tab bar",
  },
  {
    id: "books",
    label: "Books",
    title: "Books too. Pages, themes and fonts.",
    body: "Open EPUB files from your phone, or add a catalogue by its address. Choose pages or scrolling. Add highlights and notes, and search the text.",
    chips: ["Auto", "Paper", "Sepia", "Ink", "Night", "Black"],
    screen: "/phone/themes.webp",
    alt: "Akidev book reader: a page of an EPUB book with a Type and theme panel for themes, font and text size",
  },
  {
    id: "aloud",
    label: "Read aloud",
    title: "Let it read to you.",
    body: "A voice that runs on your phone reads the page, and the sentence is marked as you listen. Choose the speed that suits you.",
    screen: "/phone/aloud.webp",
    alt: "Akidev read aloud: a book page with the sentence being read marked in yellow and play controls",
  },
];

const PRIVATE = ["No accounts", "No ads", "No analytics", "No tracking"];

// Plain answers, in the words of the store listing and the app. They are for people first. Each question is one that
// a person types into a search box or asks an assistant.
const FAQ = [
  {
    q: "What is Akidev?",
    a: "A reader app for iPhone and Android. It opens manga, comics and EPUB books, and it can read books aloud.",
  },
  {
    q: "Does Akidev have manga in it?",
    a: "No. Akidev has no content of its own. You add the sources and books you already use. It does not include, offer or endorse any source or catalogue, so add only what you have the right to read.",
  },
  {
    q: "What can I open with it?",
    a: "Manga and comics from the sources you install, your own CBZ files, EPUB files from your phone, and book catalogues (OPDS) that you add by their address.",
  },
  {
    q: "Can I read webtoons and manhwa?",
    a: "Yes. Choose a long strip for webtoons and manhwa, paged reading, or right to left for manga. You can also set the speed of auto-scroll.",
  },
  {
    q: "How does read aloud work?",
    a: "A voice that runs on your phone reads your EPUB book and marks the sentence as you listen.",
  },
  {
    q: "Does Akidev cost money, or need an account?",
    a: "It is free on the App Store and Google Play. It has no accounts, no ads, no analytics and no tracking.",
  },
  {
    q: "Which phones does it run on?",
    a: "iPhones with iOS 26 or later, and Android phones with Android 10 or later.",
  },
];

// What search engines read about the app itself. It repeats only what the page already says.
const APP_DATA = {
  "@context": "https://schema.org",
  "@type": "MobileApplication",
  name: "Akidev",
  description: DESCRIPTION,
  url: `https://${APP_HOST}/`,
  image: `https://${APP_HOST}/phone/app-icon.png`,
  screenshot: STEPS.map((step) => `https://${APP_HOST}${step.screen}`),
  applicationCategory: "ReferenceApplication",
  operatingSystem: "iOS 26 or later, Android 10 or later",
  inLanguage: "en",
  isAccessibleForFree: true,
  offers: { "@type": "Offer", price: "0", priceCurrency: "USD" },
  installUrl: [APP_STORE_URL, PLAY_STORE_URL],
  featureList: ["Manga and comic reader", "EPUB book reader", "Read aloud", "Library with categories and downloads", "No accounts, ads or tracking"],
};

// The landing page has no site around it, so a link to a page of the site names the site's host.
const site = (path: string) => `https://${SITE_DOMAIN}${path}`;
const link = "text-ink underline decoration-ink/30 underline-offset-4 transition-colors hover:decoration-ink";

export default function AppPage() {
  return (
    <>
      {/* "<" is escaped so that nothing in the data can close the script tag. */}
      <script type="application/ld+json" dangerouslySetInnerHTML={{ __html: JSON.stringify(APP_DATA).replace(/</g, "\\u003c") }} />
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

      <section className="gutter mx-auto max-w-6xl border-t border-line py-24 md:py-36 lg:grid lg:grid-cols-[minmax(0,22rem)_1fr] lg:gap-20">
        <div>
          <p className="caps text-[rgb(255_210_31)]">Questions</p>
          <h2 className="mt-4 text-balance font-sans text-[clamp(2rem,4.5vw,3.25rem)] font-extrabold leading-[1.05] tracking-[-0.035em] text-ink">
            Good to know before you install.
          </h2>
        </div>
        <div className="mt-10 divide-y divide-line border-y border-line lg:mt-0">
          {FAQ.map((item) => (
            <details key={item.q} className="group py-5">
              <summary className="flex cursor-pointer list-none items-center justify-between gap-6 text-[17px] font-semibold text-ink [&::-webkit-details-marker]:hidden">
                {item.q}
                <Plus aria-hidden className="size-4 shrink-0 text-dim transition-transform duration-200 ease-spring group-open:rotate-45 motion-reduce:transition-none" />
              </summary>
              <p className="mt-3 max-w-xl text-[15px] leading-7 text-dim">{item.a}</p>
            </details>
          ))}
        </div>
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
