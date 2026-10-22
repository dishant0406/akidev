import Link from "next/link";

import { ATTRIBUTION } from "@/constants/site";

/** One quiet block at the foot of the page: the attribution the providers' licences ask for, and the pages about the app. */
export default function Footer() {
  // On a phone the catalogue bar covers the foot of the screen, so the footer clears it.
  return (
    <footer className="gutter flex flex-col gap-3 pb-28 pt-6 md:pb-10">
      <p className="max-w-3xl text-xs leading-5 text-faint">{ATTRIBUTION}</p>
      <nav className="flex gap-6 text-[13px] text-faint">
        <Link href="/download" className="py-2 transition-colors hover:text-ink">
          Download the app
        </Link>
        <Link href="/privacy" className="py-2 transition-colors hover:text-ink">
          Privacy
        </Link>
        <Link href="/manga" className="py-2 transition-colors hover:text-ink">
          iPhone reader
        </Link>
      </nav>
    </footer>
  );
}
