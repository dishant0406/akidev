"use client";

import { useSearchParams } from "next/navigation";

/**
 * Says why the visitor is here when they came from a "Download in the app" button. The page itself
 * stays the same for everyone, so it can be kept at the edge.
 */
export default function WhyHere() {
  if (!useSearchParams().get("for")) return null;
  return (
    <div className="grid gap-3 border-b border-line py-8 md:grid-cols-[11rem_1fr] md:gap-10">
      <h2 className="caps text-accent">Saving offline</h2>
      <p className="max-w-xl text-[15px] leading-7 text-ink">
        Saving to watch offline is a feature of the desktop app. Get it below, open it, and press Download on the title you wanted.
      </p>
    </div>
  );
}
