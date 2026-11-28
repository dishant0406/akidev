# Akidev TV: what other TV apps do, and what we take

Research for the Android TV app (`apps/tv-android`). It follows `design/DESIGN_PLAN.md` (warm ink palette, amber only for
state, Katagami Sans for titles and Allo for the rest, translucent chrome).

**Built 2026-11-26:** all four steps of the order of work below (the top pill bar and hero, Continue watching, the title
page with wide episode cards, and the player with Audio & subtitles and Next episode), in `TopBar.kt`, `HomeScreen.kt`,
`DetailScreen.kt`, `PlayerScreen.kt` and `TracksPanel.kt`.

## What was easy and hard to find

- **Found as real TV screens:** Disney+ Hotstar (TV), MovieBox (LG TV web app), Plex (new Android TV UI), Stremio Kai,
  Netflix 2025 redesign, Apple TV app, Netflix player.
- **Found only as launcher icons or phone screens:** JioHotstar (the new name; the TV pictures are still the Disney+ Hotstar
  look), MovieBox on Android TV.
- **Found only as concept art (Dribbble), not the real app:** ZEE5 TV. The real ZEE5 TV app shows a top tab bar, a hero
  carousel with dots and a strip of posters, in purple. Its player was not found.
- Because of this, the player ideas below come from Netflix and Prime Video, not from ZEE5.

## What each app does

**Disney+ Hotstar / JioHotstar (TV).** A thin icon rail on the far left. A hero at the top left: title in wide capitals,
a line "2023 · 1h 52m · English · U/A 16+", two lines of story, then genres "Drama | Crime". One wide "Watch Now" button
and a small square "+" button. A strip of small thumbnails on the right picks the hero. Rows below ("Latest & Trending")
mix tall and wide cards with small gaps. "Continue Watching" uses wide cards with a label like "S1 E3, 21m left".
The next row always peeks in at the bottom edge.

**MovieBox (TV).** A left rail with a language switch and a text list: Home, TV show, Movie, Animation, Most Watched.
A full-screen hero with a metadata line ("2024 · Action, Drama, Thriller") and a "Trending Now" row at the bottom edge.
It is busy: extra menu items for downloads and games. Ads and clutter are what people complain about.

**Plex and Stremio Kai.** Rail on the left. The hero changes when you move focus over a poster, so one big picture
always shows the title you are on. Stremio Kai adds a check mark on watched posters and a small details panel.

**Netflix (2025 redesign).** A centred pill bar at the top: Home, Shows, Movies, Games, My Netflix. The selected pill is
solid white. A big hero with the title art, a line "Show · Thriller · 2025 · 5 seasons · TV-MA", short text and pill buttons.
The focused card grows and shows details under it. Many users disliked the change: more screen, more promotion.

**Apple TV app.** A glass pill bar at the top (Watch Now, Apple TV+, Store, Sports, Library, search). The hero art tints
the whole page. A white "Play" button next to a glass secondary button. A "Featured" row of wide cards with a small label.

**Players (Netflix, Prime Video).** Title at the top centre ("S1:E1 Taste of the King"). In the middle: back 10 seconds,
pause, forward 10. A timeline with the time left at its right end. A row of text actions at the bottom: Episodes,
Audio & Subtitles, Next Episode. A "Skip Intro" pill appears at the bottom right at the right moment. Prime adds X-Ray.

## What we take

1. **Top glass pill bar, not a left rail.** It gives rows the full width, matches the desktop's translucent chrome, and
   both Netflix and Apple use it. Items: Home, Films, Series, Anime, Search, Settings. The selected item gets the
   amber hairline (our rule: amber is for the active tab), not a white pill.
2. **One hero that follows focus.** Move over a poster and the backdrop, the title (Katagami Sans, billboard size), the
   tracked-capital metadata line and two lines of story change. The hero fades left into the canvas as it does now.
   `Media.banner` already carries the picture. This is the single biggest step up from today.
3. **Two buttons in the hero:** primary Play (warm white), secondary glass "More info". No "+" and no extra icons.
4. **Rows.** Tall 2:3 posters for browse. Wide 16:9 cards for Continue Watching, with a thin amber progress line and
   "S1 E3 · 21 min left". Keep the next row peeking at the bottom. The name under a poster goes away (the hero names the
   focused title), which makes each row shorter and calmer.
5. **Focus.** Keep the amber ring and the small scale. Do not grow the card or add a panel under it (Netflix did and people
   disliked it).
6. **Title page.** Same hero pattern. Episodes become a row of wide cards (number, name, length, progress) instead of a
   column of text buttons. Season pills stay.
7. **Player.** Quiet, in our colours: title at the top centre; centre transport only while paused; amber hairline timeline
   with the time left at the right; bottom text actions "Episodes", "Audio & subtitles", "Next episode". Left and Right
   keep the 10 second seek. "Skip intro" only if we get intro times from a source (we have none today).
8. **Watched mark.** A small check on a poster the viewer finished (Stremio Kai).
9. **What we leave out:** profiles, games, promotions, language switch in the menu, a second rail, a coloured brand
   gradient. The art is the colour.

## Order of work

1. Top pill bar and the hero that follows focus (Home).
2. Wide Continue Watching cards (with saved position).
3. Title page with wide episode cards.
4. Player overlay with Audio & subtitles and Next episode. (Episodes in the player: left out; the title page lists them.)

## Open questions

- Tint the page from the artwork (Apple does)? It costs a colour pass on each image, and the TV is a slow 32-bit chip.
  Suggest no for now.
- Do we want a "Skip intro" source? Without one, leave the button out.

## Sources looked at (Google Images, 2026-11-26)

MovieBox for LG Smart TV (Instagram), Cinema Box Android TV (AppBrain), New Android TV UI (r/PleX),
Disney+ Hotstar improved UX (TV and phone), OTF Exclusive: Disney+ Hotstar new UI, JioHotstar on Samsung TVs (SamMobile),
Stremio Kai (r/StremioAddons), Ultra TV (open-source IPTV player), ZEE5 TV and VR concept (Dribbble), ZEE5 on smart TV
(Cashify), Netflix redesign (NYT, The Verge, MacRumors), Apple TV app featured row (9to5Mac), Netflix player and Screen Lock
(Mashable, TechWiser), Prime Video X-Ray, Skip Intro (Applicaster docs).

## Browse (2026-11-28)

The full list of a catalogue uses the same top pill, with the orders as icons and one Genre icon that opens a side panel. The
reasons and the references are in `docs/design.md` ("TV: Browse uses the same bar as the rest of the app"). Before and after:
`design/shots/tv-browse-before.png`, `tv-browse-after.png`, `tv-browse-genres-after.png`, `tv-browse-show-more-after.png`.

