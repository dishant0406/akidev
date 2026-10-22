import type { Metadata } from "next";

import { Page, Section } from "@/components/prose";
import { REPO_URL } from "@/constants/site";

export const metadata: Metadata = {
  title: "Support | Akidev for iPhone",
  description: "Help with the Akidev iPhone reader: adding sources, backups, downloads and troubleshooting.",
};

const link = "text-ink underline decoration-ink/30 underline-offset-4 transition-colors hover:decoration-ink";

export default function MangaSupportPage() {
  return (
    <Page title="Support." lead="Answers to the questions that come up most, and where to ask the rest.">
      <Section title="There is nothing to read">
        <p>
          The app comes with no sources. Open the Sources tab, tap +, and add a repository by its address (for example{" "}
          <code>github.com/owner/repository</code>). Install a source from it and it appears in Discover.
        </p>
      </Section>

      <Section title="A source shows an error">
        <p>
          A site may be down, may have changed, or may ask for a browser check that the app cannot pass. Pull down to try again. If it
          keeps failing, update the source from the Sources tab, or remove it and install it again. A series that is only linked to a
          publisher&apos;s own site has no chapters the app can show.
        </p>
      </Section>

      <Section title="Reading position">
        <p>
          The app keeps your chapter, page and how far down you were, on your phone. Back up from Settings to move it to another phone.
        </p>
      </Section>

      <Section title="Downloads and your own files">
        <p>
          Press and hold a chapter to download it for offline reading. Settings has a way to open a CBZ comic file from your phone.
        </p>
      </Section>

      <Section title="Ask us">
        <p>
          Open an issue at{" "}
          <a href={REPO_URL + "/issues"} className={link} rel="noopener">
            the repository
          </a>{" "}
          and say what you did, what you expected and what the app showed. We read every one.
        </p>
      </Section>
    </Page>
  );
}
