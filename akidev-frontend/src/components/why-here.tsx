"use client";

import { useSearchParams } from "next/navigation";

/**
 * Says why the visitor is here when they came from a "Download in the app" button. The page itself
 * stays the same for everyone, so it can be kept at the edge.
 */
export default function WhyHere() {
  if (!useSearchParams().get("for")) return null;
  return (
    <p className="max-w-prose rounded-lg bg-wash-2 px-4 py-3 text-[15px] text-ink">
      Saving to watch offline is a feature of the desktop app. Get it below, open it, and press Download on the title you wanted.
    </p>
  );
}
