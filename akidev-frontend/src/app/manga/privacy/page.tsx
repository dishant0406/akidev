import type { Metadata } from "next";

import { Page, Section } from "@/components/prose";
import { REPO_URL } from "@/constants/site";

export const metadata: Metadata = {
  title: "Privacy policy | Akidev for iPhone",
  description: "What the Akidev iPhone reader does and does not do with your data: nothing is collected.",
};

const link = "text-ink underline decoration-ink/30 underline-offset-4 transition-colors hover:decoration-ink";

export default function MangaPrivacyPage() {
  return (
    <Page
      title="Privacy."
      lead="The Akidev iPhone app collects nothing about you. It has no accounts, no analytics, no advertising and no tracking. This is all of it."
    >
      <Section title="What we collect">
        <p>
          Nothing. The app does not send anything about you, your device or your reading to Akidev or to anyone we work with. We run no
          server for the app.
        </p>
      </Section>

      <Section title="What stays on your phone">
        <p>
          Your library, categories, reading progress, settings, downloaded chapters and the sources you installed are kept on your own
          device, in the app&apos;s storage. A backup is a file you create and keep wherever you choose; the app never uploads it.
          Deleting the app deletes all of it.
        </p>
      </Section>

      <Section title="Sources and repositories">
        <p>
          To show you anything, the app contacts the sites that the sources you installed describe, and the repositories you added to
          get those sources (a GitHub address, for example). Like any website they see your connection&apos;s address and what you
          asked for, under their own privacy policies. We do not operate them and do not see those requests.
        </p>
        <p>The app adds no repository and no source by itself. Only add ones you trust.</p>
      </Section>

      <Section title="Crash reports">
        <p>
          The app has no crash reporting of its own. If you chose to share analytics or TestFlight feedback with developers in iOS
          settings, Apple may pass us an anonymous crash report under Apple&apos;s terms.
        </p>
      </Section>

      <Section title="Children">
        <p>
          The app does not knowingly collect information from anyone. Because it reads whatever sources you add, some of what you can
          open may be for adults; sources marked 18+ stay hidden until you turn them on in Settings.
        </p>
      </Section>

      <Section title="Contact">
        <p>
          Akidev is open source. Questions or concerns can be raised as an issue at{" "}
          <a href={REPO_URL} className={link} rel="noopener">
            the repository
          </a>
          .
        </p>
      </Section>
    </Page>
  );
}
