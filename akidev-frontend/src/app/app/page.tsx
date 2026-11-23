/* eslint-disable @next/next/no-img-element -- local pictures at their exact size: the image optimizer would only resize them */
import { Plus } from "lucide-react";
import type { Metadata, Viewport } from "next";

import { HeroStage } from "@/components/hero-stage";
import { ScreenGallery, type Slide } from "@/components/screen-gallery";
import { StoreButton } from "@/components/store-button";
import { APP_HOST, APP_STORE_ID, APP_STORE_URL, PLAY_STORE_URL, REPO_URL, SITE_DOMAIN } from "@/constants/site";

// The words people search for are: manga reader, comic reader, EPUB reader, read aloud, and no ads / no account.
// They are used where they are true, once each, in the title, the description, the opening text and the questions.
const TITLE = "Akidev: Manga, Comic and EPUB Reader with Read Aloud";
const DESCRIPTION = "A calm manga, comic and EPUB reader for iPhone and Android, with read-aloud. No accounts, no ads, no tracking. Free on the App Store and Google Play.";
const IMAGE_ALT = "Akidev on an iPhone, showing a series with its chapters, beside the words: Manga, comics and books in one calm reader";

// The landing page is bright, like the app. The rest of the site is dark.
export const viewport: Viewport = { themeColor: "#ffffff", colorScheme: "light" };

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
const SLIDES: Slide[] = [
  {
    id: "sources",
    title: "Add a source",
    body: "Akidev comes with no manga. Add a source by its address, then see what is popular and new.",
    screen: "/phone/discover.webp",
    alt: "Akidev Discover screen: a search field for a source and a featured series",
  },
  {
    id: "series",
    title: "Open a series",
    body: "See every chapter and what is still unread. Save the series to your library.",
    screen: "/phone/series.webp",
    alt: "Akidev series screen: a cover, the title The Last Lantern, and a list of unread chapters",
  },
  {
    id: "reading",
    title: "Read your way",
    body: "Scroll a long strip, turn pages, or read right to left. Set the speed of auto-scroll.",
    screen: "/phone/reader.webp",
    alt: "Akidev reader settings: reading mode Strip, Paged or Right to left, with a double page behind",
  },
  {
    id: "library",
    title: "Keep your place",
    body: "Categories, history, alerts for new chapters, and downloads for reading offline.",
    screen: "/phone/library.webp",
    alt: "Akidev library: a grid of covers with unread counts, a Weekend category and a tab bar",
  },
  {
    id: "books",
    title: "Books too",
    body: "Open EPUB files. Choose pages or scrolling and one of six themes. Add highlights and notes.",
    screen: "/phone/themes.webp",
    alt: "Akidev book reader: a page of an EPUB book with a Type and theme panel for themes, font and text size",
  },
  {
    id: "aloud",
    title: "Read aloud",
    body: "A voice on your phone reads the book. The sentence is marked as you listen.",
    screen: "/phone/aloud.webp",
    alt: "Akidev read aloud: a book page with the sentence being read marked in yellow and play controls",
  },
];

const OPENS = [
  { name: "Manga and comics", text: "From the sources you install. Read them as a long strip, in pages, or right to left." },
  { name: "Your own files", text: "CBZ comics and EPUB books from your phone." },
  { name: "Book catalogues", text: "Add a catalogue (OPDS) by its address and open its books." },
  {
    name: "Sources",
    text: "Akidev has no content of its own. It does not include, offer or endorse any source or catalogue. Add only what you have the right to read.",
  },
];

const PRIVATE = ["No accounts.", "No ads.", "No analytics.", "No tracking."];

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
  screenshot: SLIDES.map((slide) => `https://${APP_HOST}${slide.screen}`),
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
const h2 = "text-balance font-sans text-[clamp(1.75rem,3.6vw,2.5rem)] font-semibold leading-[1.15] tracking-[-0.02em] text-ink";
const section = "gutter mx-auto max-w-[72rem] py-20 md:py-32";

export default function AppPage() {
  return (
    <>
      {/* "<" is escaped so that nothing in the data can close the script tag. */}
      <script type="application/ld+json" dangerouslySetInnerHTML={{ __html: JSON.stringify(APP_DATA).replace(/</g, "\\u003c") }} />

      <section className="gutter mx-auto max-w-[72rem] pb-16 pt-14 text-center md:pb-24 md:pt-16">
        <img
          src="/phone/app-icon.png"
          alt=""
          width={80}
          height={80}
          className="mx-auto size-20 animate-rise rounded-[22%] shadow-[0_1px_2px_rgb(0_0_0/0.16),0_12px_24px_-8px_rgb(0_0_0/0.3)]"
        />
        <h1
          className="mx-auto mt-8 max-w-[16em] animate-rise text-balance font-sans text-[clamp(2.5rem,6vw,4.5rem)] font-semibold leading-[1.05] tracking-[-0.02em] text-ink"
          style={{ animationDelay: "80ms" }}
        >
          Manga, comics and books in one{" "}
          <mark className="bg-[linear-gradient(transparent_16%,rgb(var(--accent))_16%,rgb(var(--accent))_90%,transparent_90%)] px-[0.14em] text-ink [box-decoration-break:clone] [-webkit-box-decoration-break:clone]">calm</mark> reader.
        </h1>
        <p className="mx-auto mt-5 max-w-[34rem] animate-rise text-pretty text-[17px] leading-[1.6] text-dim md:text-[19px]" style={{ animationDelay: "160ms" }}>
          Akidev is a reader app for iPhone and Android. It has no content of its own. You add the sources and books you already use.
        </p>
        <div className="mx-auto mt-8 grid max-w-[22rem] animate-rise grid-cols-2 gap-3 sm:flex sm:max-w-none sm:justify-center" style={{ animationDelay: "240ms" }}>
          <StoreButton store="ios" />
          <StoreButton store="android" />
        </div>
        <p className="mt-4 animate-rise text-[13px] leading-5 text-faint" style={{ animationDelay: "320ms" }}>
          Free. iPhone needs iOS 26 or later. Android needs Android 10 or later.
        </p>
        <div className="mt-12 animate-rise text-left md:mt-14" style={{ animationDelay: "400ms" }}>
          <HeroStage />
        </div>
      </section>

      <div className="pb-20 md:pb-32">
        <ScreenGallery slides={SLIDES} />
      </div>

      <section className={`${section} border-t border-line`}>
        <h2 className={h2}>What Akidev opens</h2>
        <dl className="mt-10 divide-y divide-line border-y border-line">
          {OPENS.map((item) => (
            <div key={item.name} className="grid gap-2 py-6 md:grid-cols-[16rem_1fr] md:gap-10">
              <dt className="text-[17px] font-semibold leading-6 text-ink">{item.name}</dt>
              <dd className="max-w-xl text-pretty text-[17px] leading-[1.6] text-dim">{item.text}</dd>
            </div>
          ))}
        </dl>
      </section>

      <section className="bg-surface">
        <div className={section}>
          <h2 className="font-sans text-[clamp(2.25rem,6.4vw,4.5rem)] font-semibold leading-[1.06] tracking-[-0.02em] text-ink">
            {PRIVATE.map((line) => (
              <span key={line} className="block">
                {line}
              </span>
            ))}
          </h2>
          <p className="mt-8 max-w-xl text-pretty text-[17px] leading-[1.6] text-dim">
            Everything you read and save stays on your phone. Back it up to a file when you want to move it to another phone. The{" "}
            <a href={site("/manga/privacy")} className={link}>
              privacy policy
            </a>{" "}
            says all of it.
          </p>
        </div>
      </section>

      <section className={`${section} lg:grid lg:grid-cols-[minmax(0,20rem)_1fr] lg:gap-20`}>
        <h2 className={h2}>Questions</h2>
        <div className="mt-10 divide-y divide-line border-y border-line lg:mt-0">
          {FAQ.map((item) => (
            <details key={item.q} className="group py-5">
              <summary className="flex min-h-11 cursor-pointer list-none items-center justify-between gap-6 text-[17px] font-semibold leading-6 text-ink [&::-webkit-details-marker]:hidden">
                {item.q}
                <Plus aria-hidden className="size-4 shrink-0 text-dim transition-transform duration-200 ease-out group-open:rotate-45 motion-reduce:transition-none" />
              </summary>
              <p className="mt-2 max-w-xl text-pretty text-[17px] leading-[1.6] text-dim">{item.a}</p>
            </details>
          ))}
        </div>
      </section>

      <section className="border-t border-line">
        <div className="gutter mx-auto flex max-w-[72rem] flex-col items-center py-20 text-center md:py-32">
          <img
            src="/phone/app-icon.png"
            alt=""
            width={80}
            height={80}
            className="size-20 rounded-[22%] shadow-[0_1px_2px_rgb(0_0_0/0.16),0_12px_24px_-8px_rgb(0_0_0/0.3)]"
          />
          <h2 className="mt-8 font-sans text-[clamp(2.25rem,5vw,3.5rem)] font-semibold leading-[1.06] tracking-[-0.02em] text-ink">Get Akidev.</h2>
          <p className="mt-4 max-w-md text-pretty text-[17px] leading-[1.6] text-dim">Free on the App Store and Google Play.</p>
          <div className="mt-8 grid w-full max-w-[22rem] grid-cols-2 gap-3 sm:flex sm:w-auto sm:max-w-none">
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

      <footer className="gutter mx-auto flex max-w-[72rem] flex-col gap-3 border-t border-line py-10 text-[13px] leading-5 text-faint">
        <p className="max-w-3xl">
          The pictures on these screens are made-up art for this page, not real series. Icons: Solar by 480 Design (CC BY 4.0).
        </p>
        <nav className="flex flex-wrap gap-x-6">
          <a href={site("/manga/support")} className="py-2 transition-colors hover:text-ink">
            Help and answers
          </a>
          <a href={REPO_URL} className="py-2 transition-colors hover:text-ink" rel="noopener">
            Open source
          </a>
        </nav>
      </footer>
    </>
  );
}
