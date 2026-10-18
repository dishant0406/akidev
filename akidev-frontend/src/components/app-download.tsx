"use client";

import { Download } from "lucide-react";
import { useEffect, useState } from "react";

import { Button } from "@/components/button";

/** How long the app has to take over the window before the download page is shown instead. */
const APP_OPENS_WITHIN = 1500;

const getAppPath = (id: string) => `/download?for=${encodeURIComponent(id)}`;

/**
 * "Download in the app": the website does not save anything. On a computer it opens the desktop
 * app at this title, where the download is chosen; if the app does not take over the window, the
 * page that offers the app is shown. A phone or tablet is sent to that page at once.
 */
export default function AppDownload({ id }: { id: string }) {
  const [computer, setComputer] = useState(true);
  useEffect(() => {
    setComputer(window.matchMedia("(hover: hover) and (pointer: fine)").matches);
  }, []);

  if (!computer) {
    return (
      <Button look="secondary" href={getAppPath(id)} icon={<Download className="size-4" />}>
        Get the app to download
      </Button>
    );
  }

  const open = () => {
    // The system hands the window to the app (blur, or the page hidden); if that does not happen the app is not there.
    const opened = { value: false };
    const taken = () => {
      opened.value = true;
    };
    window.addEventListener("blur", taken, { once: true });
    document.addEventListener("visibilitychange", taken, { once: true });
    window.location.href = `akidev://download?id=${encodeURIComponent(id)}`;
    window.setTimeout(() => {
      window.removeEventListener("blur", taken);
      document.removeEventListener("visibilitychange", taken);
      if (!opened.value) window.location.href = getAppPath(id);
    }, APP_OPENS_WITHIN);
  };

  return (
    <Button look="secondary" onClick={open} icon={<Download className="size-4" />}>
      Download in the app
    </Button>
  );
}
