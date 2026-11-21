import type { Metadata, Viewport } from "next";
import "./globals.css";

import Footer from "@/components/footer";
import PartyFollower from "@/components/party-follower";
import TopBar from "@/components/topbar";
import { Toaster } from "@/components/ui/sonner";
import { SITE_DOMAIN } from "@/constants/site";
import { getScreens } from "@/lib/api-server";
import { appFont, titleFont } from "@/lib/fonts";
import PartyProvider from "@/providers/party";
import QueryProvider from "@/providers/query-provider";
import ScreensProvider from "@/providers/screens";

// The Akidev mark: the vector for browsers that take one, the .ico for those that do not, and
// a 180px PNG for a home screen.
const TITLE = "Akidev | Anime, series and films";
const DESCRIPTION = "Browse and stream anime, series and films from one place.";

export const metadata: Metadata = {
  applicationName: "Akidev",
  metadataBase: new URL(`https://${SITE_DOMAIN}`),
  title: TITLE,
  description: DESCRIPTION,
  icons: {
    icon: [
      { url: "/icon.svg", type: "image/svg+xml" },
      { url: "/favicon.ico", sizes: "any" },
    ],
    apple: "/apple-touch-icon.png",
  },
  openGraph: { type: "website", siteName: "Akidev", title: TITLE, description: DESCRIPTION },
  twitter: { card: "summary", title: TITLE, description: DESCRIPTION },
};

export const viewport: Viewport = { themeColor: "#0b0b0d", colorScheme: "dark" };

// The tabs come from the API, which is not running while the site is built, so no page is built ahead.
export const dynamic = "force-dynamic";

export default async function RootLayout({ children }: { children: React.ReactNode }) {
  // The tabs, sorts and genres, for every page.
  const screens = await getScreens();
  return (
    <html lang="en" className={`${appFont.variable} ${titleFont.variable}`}>
      <body className="min-h-screen max-w-[100vw] overflow-x-hidden bg-canvas font-sans text-ink antialiased">
        <QueryProvider>
          <ScreensProvider screens={screens}>
            <PartyProvider>
              <TopBar />
              <main className="min-h-[70vh]">{children}</main>
              <Footer />
              <PartyFollower />
            </PartyProvider>
          </ScreensProvider>
          <Toaster />
        </QueryProvider>
      </body>
    </html>
  );
}
