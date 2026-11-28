// Two faces, both committed and loaded locally (next/font/google would make `next build` depend on
// fonts.googleapis.com). Both are SIL Open Font License; see ATTRIBUTION.md.
//
// Allo carries the interface: one variable file for the weights the UI uses (400 to 600), with its
// optical size fixed at 14 like the desktop and TV apps. Katagami Serif is the title face, one
// variable file for 400 to 600. Both files keep only the Latin letters, the accents real titles use and
// common punctuation, so the two cost 55 KB together.
import localFont from "next/font/local";

export const appFont = localFont({
  src: "../assets/fonts/allo-wght.woff2",
  weight: "400 600",
  style: "normal",
  display: "swap",
  variable: "--font-app",
});

export const titleFont = localFont({
  src: "../assets/fonts/katagami-serif-wght.woff2",
  weight: "400 600",
  style: "normal",
  display: "swap",
  variable: "--font-title",
  // A serif fallback for the moment before the font arrives, sized to match, so the title does not jump.
  adjustFontFallback: "Times New Roman",
});
