# Akidev design plan

Goal: the web app and the desktop app should look like one product with its own taste, not a
template. This is the research, the diagnosis and the order of work. Nothing here is built yet.

## What is wrong now (from the code and the live site)

1. **Every control is the same grey pill.** One `secondary` look (white at 12 % on the canvas, `rounded-md`,
   15 px semibold) is used for "Load more", "Try again", "Trailer", "More info" and more. It says
   nothing, so it reads as a default. (`button.tsx`, `design/button.rs`)
2. **States are bare.** "No playable source found / Try again", "No sources found", "No results" are
   a bold line, a grey line and a grey button, centred in empty space. No place, no voice, no next
   step that helps (retrying a dead source rarely works). (`states.tsx`, `sources.tsx`, `watch.rs:1579`)
3. **No type identity.** Inter at default weights everywhere, the font most associated with generated UIs.
   Hierarchy is only size and bold.
4. **Ad-hoc surfaces.** Five corner radii in use (`full 34, lg 20, md 11, xl 5, 2xl 3`), about nine different
   white-opacity fills (`ink/[0.07]`, `0.08`, `0.12`, `0.14`, `0.28`, `white/10`, `white/20`...), and about
   25 hard-coded colours in the desktop app outside `tokens.rs`. Nothing is a token, so nothing is consistent.
5. **Netflix layout, no signature.** Billboard, rows of posters, a pill nav. A stranger could not tell it
   from any streaming clone. Lucide icons in all 13 components add to that.
6. **Accent barely used.** Amber `#e6b450` is a good colour and is only a 2 px underline and a few words.

## What the research says

- The tells of generated UI (impeccable.style, saasui.design, UX Planet): Inter/Geist by default, flat
  hierarchy, identical rounded cards, hairline border plus wide shadow, Lucide-only icons, purple/blue
  glows, pill CTAs with arrows, happy-path-only design (never-seen empty, error and loading states).
- Dribbble "streaming app dark minimal" results are mostly phone mock-ups with neon gradients and glow.
  That is the look to avoid; shipped products are better references.
- Apple TV: pure black, one white primary button, a translucent "glass" secondary, ghost for cancel, accent
  colour only where unavoidable, 4 px radius for badges / 8 buttons / 12 tiles / 16 popups, no weight above
  700, spring easing, 200 ms hovers, few rows above the fold, progress bar white so the player stays quiet.
- Raycast: depth from layered shadow with an inset highlight rather than grey steps; keycap-styled shortcuts;
  accent that punctuates instead of dominating.
- A24 (seen live): a white page, tiny tracked-out capitals and a serif wordmark. Identity from type and
  restraint, with no decoration at all.
- Letterboxd and AniList (seen live): both are dense and utilitarian; their personality is tiny uppercase
  section labels and tight metadata. Good for data, but neither feels cinematic.
- Load more vs infinite scroll (Baymard via Smashing Magazine): for catalogue grids, a load-more button
  beats pagination and infinite scroll for comparison and reaching the footer, so keep the button but make
  it informative.
- Empty/error states (Primer, SAP Fiori): one-line reason, at most three lines of context, a next step that
  can actually help; offer retry only when it can plausibly succeed.

## Identity: "Aki" (秋, autumn)

The name already holds the idea, and the amber accent is autumn light. The direction is a **Japanese film
title card**: ink-black frame, one warm ember colour, quiet serif titles, tiny tracked capitals for metadata.

- **Colour:** keep the near-black canvas but warm it slightly (`#0c0b0a` family), keep amber as the single
  accent, and use it for *state* (playing, active, progress, focus), not decoration. Primary button stays
  warm white. No gradients, no glow.
- **Type:** a display serif for titles on billboards, title pages and state headlines (candidates to test on
  dark: Fraunces, Instrument Serif, Newsreader), and a sans for UI that is not Inter (candidates: Geist is
  also a tell, so General Sans, Satoshi or Hanken Grotesk). Metadata in small tracked capitals; numbers
  tabular. Cap weights at 600, as Apple does.
- **Signature details:** the 秋 mark as the favicon/loading mark; a thin ember hairline as the progress and
  active indicator everywhere; film-credit style metadata ("1994 · FILM · 142 MIN"); title-card layout for
  every state screen (a big serif line, a small grey line, actions as text links); keycap chips for
  shortcuts (Ctrl K, Z/X, M) in the search button and player.
- **Buttons (three, and only three):** primary = solid warm white, 8 px, no arrow; secondary = hairline
  outline, transparent, fills on hover; text action = underlined on hover with a keycap hint where one
  exists. Icon buttons lose the grey circle until hovered. One radius scale: 4 / 8 / 12 / 16.
- **Motion:** one easing curve (spring, `cubic-bezier(0.32,0.72,0,1)`), 150–200 ms, transform and opacity
  only; no scale on posters except a 1.02 lift.
- **Icons:** keep a library but draw the few that carry brand (play, source, subtitles, party) ourselves, in
  one 1.5 px stroke.

## Plan, in order

1. **Tokens, shared by both apps.** One spec (radius, opacity ladder of three fills, type scale, easing,
   amber rules) written once; mirror it in `tailwind.config.ts` / `globals.css` and `design/tokens.rs`. Replace
   every ad-hoc opacity, radius and hard-coded colour. (Mechanical, low risk; the bulk of the "consistent" feel.)
2. **Type.** Pick fonts by rendering 3 candidates on a real title page and billboard, on both web and GPUI
   (needs the font files bundled; GPUI loads fonts from assets). Apply the display serif to titles only.
3. **Components.** Rebuild `Button`/`IconButton`/`Tab` and the Rust `design/button.rs` to the three
   buttons; add `Keycap`, `Hairline`, and one `StateCard` (title-card layout) replacing `Centred`, `Note` and
   the desktop equivalents.
4. **States first** (the worst offenders, and quick wins): load more → "24 of 480" with an ember hairline
   progress and "Show 24 more", loading on scroll end optionally; no playable source → a title card that says
   what was tried and offers "Choose a source" / "Try another episode" as real next steps instead of a lone
   retry; source panel rows redesigned as a table-like list (resolution, size, seeders as quiet columns,
   "Instant" as an amber word); empty and error states for search, browse, music, party.
5. **Screens**, one at a time, web and desktop together so they never drift: title page (billboard +
   metadata), home rows (fewer, larger, 16:9 for continue watching), browse filters, player chrome, party panel,
   music, settings, command palette.
6. **Review pass with the detector** (`npx impeccable detect` on the web app) and a screenshot sweep of every
   screen and state at 3 widths before each release.

## Decisions I need from you

- Is "Aki / title card / ember on ink" the direction, or do you want something less editorial (for example
  sharper and more technical, like Raycast)?
- Are new fonts fine? They add about 100–200 KB on the web and need bundling in the desktop app.
- Web and desktop together in each step (recommended), or web first?
