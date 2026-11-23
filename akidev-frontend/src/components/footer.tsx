import Link from "next/link";

import { APP_HOST } from "@/constants/site";
import { getScreens } from "@/lib/api-server";

/** One quiet block at the foot of the page: the attribution the providers' licences ask for, and the pages about the app. */
export default async function Footer() {
  const screens = await getScreens();
  // On a phone the catalogue bar covers the foot of the screen, so the footer clears it.
  return (
    <footer className="gutter flex flex-col gap-3 pb-28 pt-6 md:pb-10">
      <p className="max-w-3xl text-xs leading-5 text-faint">{screens?.about.attribution}</p>
      <nav className="flex gap-6 text-[13px] text-faint">
        <Link href="/download" className="py-2 transition-colors hover:text-ink">
          Download the app
        </Link>
        <Link href="/privacy" className="py-2 transition-colors hover:text-ink">
          Privacy
        </Link>
        <a href={`https://${APP_HOST}`} className="py-2 transition-colors hover:text-ink">
          Phone app
        </a>
      </nav>
    </footer>
  );
}
