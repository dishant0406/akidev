# Aki Manga design

The app follows a reference the owner chose (a webtoon-style reader: bright, rounded, one yellow, soft grey cards,
a floating tab bar) and uses Liquid Glass for everything that floats or is touched. It is its own product and does
**not** follow `design/DESIGN_PLAN.md` (the dark amber look of the website and the desktop app); that was decided on
2026-10-22 when the owner shared the reference.

## What the reference gave us

- **Series page:** a full-width picture with back, save and more over it; small stat chips and a bold white title on a
  dark gradient at its foot; a white sheet with big rounded top corners rising over the picture; two wide pills,
  *Episodes* (yellow when chosen) and *Details*; then one soft grey rounded card per chapter with a thumbnail, a small
  grey line, a bold title, grey chips (date, views) and an icon on the right.
- **Inbox page:** a centred bold title with a round button each side; three wide pills (the chosen one yellow); a list of
  the same grey cards (thumbnail, small label, bold title, date chip).
- **Floating tab bar:** a rounded capsule above the bottom edge with a few line icons; the current tab is a yellow
  circle with a black glyph.
- **Type and colour:** a rounded geometric sans, bold titles; white page, grey cards (about `#F4F4F5`), one bright
  yellow (about `#FFD21F`), black text; generous radii (thumbnails 14, cards 22, sheet 32).

We took the layout and feel, not the artwork. Our pictures are the series covers a source gives.

## Where glass is used, and where it is not

Apple's guidance is that glass belongs to the controls and navigation layer, that glass cannot sample glass (so group
glass in one `GlassEffectContainer`), and that tint should mean something. Applied here:

| Glass | Flat |
| --- | --- |
| The floating tab bar, and its current tab (yellow-tinted glass that flows to the next) | Chapter and update cards (soft grey, as in the reference) |
| Round buttons: back, save, more, refresh, add | The page behind (white, with a soft yellow glow so glass has something to refract) |
| Pills (Chapters/Details, Popular/Latest, Saved/History, Unread/Recent): the chosen one is yellow glass and slides | Text |
| The search field, source chips, "Show more", the main action ("Continue") in yellow glass | |
| Chips and the count badge over a picture | Flat grey chips on a card |
| The reader's top and bottom bars | |

The only tint is yellow, and it marks the current choice or the main action.

## Where the values are

`App/Design.swift` (colours, radii, the glass components `GlassIconButton`, `YellowGlassButton`, `PillTabs`, `Chip`,
`StateCard`, `FlowLayout`), `App/TabBar.swift` (the floating bar), `App/Cards.swift` (`CoverCard`, `InboxRow`,
`ScreenHeader`). Colours adapt to light and dark; the reader is always black.

## Rules

- One yellow. No second accent, no gradients on controls.
- Titles are rounded and bold; metadata is small and grey.
- A pushed screen (a series, a repository) hides the tab bar and has its own back button.
- Anything that scrolls under the tab bar uses `.underTabBar()`.

## Icons

Icons are the Solar set (480 Design, CC BY 4.0), drawn as template images in `App/Assets.xcassets` and shown only through
`AkiIcon`: `name` is the outline, `name-solid` the bold one (used for the selected tab, saved and read states). To add one,
put its Solar name in `scripts/fetch-icons.py` and run it; the credit is in Settings > About.

## Colours

Every colour and corner radius is a token in `App/UI/Tokens.swift` (`Aki.*`): the palette (`yellow`, `canvas`, `card`, `chip`,
`ink`, `dim`, `danger`), what goes over pictures (`onImage`, `veil`, the `fade*` gradients), the shadow, the two background glows
and the reader's backdrops. Screens never write a colour: `scripts/check-ui.sh` fails on `Color(`, `.white`, `.black` and `.gray`
outside `App/UI`. The one exception inside `UI` is the tokens file itself.
