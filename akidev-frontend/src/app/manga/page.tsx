import type { Metadata } from "next";
import Link from "next/link";

import { Page, Section } from "@/components/prose";
import { REPO_URL } from "@/constants/site";

export const metadata: Metadata = {
  title: "Akidev for iPhone | A reader for manga, manhwa and webcomics",
  description: "A fast, native iPhone reader for manga, manhwa and webcomics. It has no content of its own: you add the sources you trust.",
};

const link = "text-ink underline decoration-ink/30 underline-offset-4 transition-colors hover:decoration-ink";

export default function MangaPage() {
  return (
    <Page
      title="Akidev for iPhone."
      lead="A reader for manga, manhwa and webcomics, made for the phone: long strips that scroll like butter, pages that turn, and a library that remembers where you stopped. It ships with no content of its own."
    >
      <Section title="What it is">
        <p>
          A native iOS 26 app. Pick a source, browse what is popular and new, open a series and read. Your library, categories, reading
          position and downloads live on your phone.
        </p>
      </Section>

      <Section title="Reading">
        <p>
          Long strip, left to right or right to left pages, pinch to zoom, and the next chapter loaded before you reach it. It remembers the
          chapter, the page and how far down it you were, down to the screen.
        </p>
      </Section>

      <Section title="Your library">
        <p>
          Save series, sort them into shelves, see unread counts and get a feed of new chapters. Download chapters to read offline. Back
          everything up to a file and restore it on another phone. Open your own CBZ files too.
        </p>
      </Section>

      <Section title="Sources">
        <p>
          Akidev is a renderer. A source is a small plain-text description of how a site is read, kept in a repository you add by its
          address, and nothing in it is code. The app comes with none and recommends none: you choose the repositories you trust, and read
          only what you have the right to read.
        </p>
        <p>
          <Link href="/manga/support" className={link}>
            Help and answers
          </Link>
          {" · "}
          <Link href="/manga/privacy" className={link}>
            Privacy
          </Link>
          {" · "}
          <a href={REPO_URL} className={link} rel="noopener">
            Open source
          </a>
        </p>
      </Section>

      <Section title="Credits">
        <p>
          Icons: Solar by 480 Design (CC BY 4.0). Screenshots on the App Store show Pepper&amp;Carrot by David Revoy (CC BY 4.0) and
          public-domain comics from the Internet Archive.
        </p>
      </Section>
    </Page>
  );
}
