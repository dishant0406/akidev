# Akidev launch film: beat map (draft 1)

120 BPM, so one beat is 0.5 s. 64 beats = 32 s = 16 bars of 4. Time = beat × 0.5. Everything below is in beats;
seconds come from that one number, so a song at 118 or 122 BPM only changes the constant.

How to read a row: **start** is the beat the thing begins on, **settle** is when the spring has come to rest.
The app's spring (critically damped, 0.35 s) takes about 0.7 beat to settle, so a move started on a beat is done
before the next one. Half beats (x.5) carry the overflow where a scene has more actions than beats.
"Carry" is what survives into the next row. An empty carry would be a cut, and there are none.

Copy marked (film) is made up for the film. Everything else is real UI copy (checked in `manga-app/App/*.swift`
and the Android screens) or the Play listing.

Film names used: covers are titled (film) "The Last Lantern" (cover-01, also the strip), "Parcel Run" (02),
"Maple Bridge" (03), "One Umbrella" (04), "Green Glass" (05), "Wheat and Wings" (06), "Night Wok" (07),
"Cold Stars" (08), "The Round Library" (09), "Red Bag" (10). Source: "Demo source". Category: "Weekend" (film).
Nearby phone: "Mira's phone" (film, no OS or brand in the name).

## Open: beats 0–7 (0–4 s)

| B | Cursor | What moves (start → settle) | Carry |
|---|---|---|---|
| 0 | – | Frame 0 = the last frame: "Akidev" in Nunito 900 on white, two yellow glows. The dot of the "i" is yellow (my addition: it is the dot that becomes the pill). | wordmark |
| 1 | – | Letters slide toward the dot of the "i" by one shared factor, scaleX keeps them touching (accordion), 1 → 1.7 | the squeeze |
| 2 | – | Squeeze ends: only the yellow dot is left. Dot pops into a capsule (small bounce) | yellow dot |
| 3 | – | Capsule grows wide. "What do you read?" rises out of a mask line inside it | the pill |
| 4 | – | Pill opens into an iPhone: the screen grows out of the capsule, the bezel, buttons and island grow with it (the app stays on this phone until the close). Inside it, the two onboarding cards, "Manga" and "Books"; body line and "You can change this any time in Settings." rise under them | cards |
| 5 | moves in | Cursor arrives on Manga. Click (ripple ring), 0.97 squeeze | click |
| 5.5 | – | Manga takes the yellow fill; Books steps back (scale 0.96, muted) | yellow card |
| 6 | – | Six iris blades close over the Manga card (hexagonal aperture) | iris |
| 7 | – | Iris snaps open onto the Welcome screen: mint icon in a glass tile pops from zero | glass tile |

## Source: beats 8–15 (4–8 s)

| B | Cursor | What moves | Carry |
|---|---|---|---|
| 8 | – | "Welcome to Akidev" rises from its mask line | title |
| 9 | – | "Akidev is a reader. It comes with no manga." rises under it | line |
| 10 | – | Paste button pops from zero (bounce); "Type an address" and "Only add repositories you trust." slide up | button |
| 11 | click Paste | Field opens out of the button; "github.com/owner/repository" slides into it | field |
| 11.5 | click Continue | Continue squeezes 0.97 | button |
| 12 | – | Repository page pushes in from the right; Welcome moves left 30% | page push |
| 13 | – | One source card lands: "Demo source", Signed note, Install button | card |
| 14 | click Install | Install fills left to right as a bar draws across | bar |
| 15 | – | Tick pops from zero inside the button; camera starts zooming into the button | the zoom |

## Discover to Series: beats 16–23 (8–12 s), the drop lands on 16

| B | Cursor | What moves | Carry |
|---|---|---|---|
| 16 | – | **Drop.** Zoom lands; the button goes to a circle (goo), yellow floods the phone's screen (0.3 s) | yellow flood |
| 17 | – | Flood contracts into the "Read now" button. Discover unfolds from behind it like a paper map: centre, then plus, then corners | the button |
| 17.5 | – | Glass search pill ("Search Demo source") and the Genre chip land | search pill |
| 18 | – | Featured card: cover-01, "Popular on Demo source" glass chip, "The Last Lantern" rises | card |
| 18.5 | – | "Continue reading" row: covers 02, 03, 04 pop from zero one per half beat | covers |
| 19 | click Read now | Squeeze; cover-01 starts to expand out of the card | the cover |
| 20 | – | Cover fills the top: glass back, save and more buttons pop over it; chips "3 chapters", "Ongoing", "Demo source" | hero |
| 20.5 | – | "The Last Lantern" rises from its mask, white and bold | title |
| 21 | – | White sheet rises, 32 radius; pills Chapters (yellow) / Bookmarks / Details slide in | sheet |
| 22 | – | Chapter cards arrive one per half beat: "Unread", "Chapter 3/2/1", date chips (film dates), circle check | cards |
| 23 | – | "Start · Chapter 1" pops from zero in yellow (bounce) | button |

## Reader: beats 24–29 (12–15 s)

| B | Cursor | What moves | Carry |
|---|---|---|---|
| 24 | click Start | Black floods the screen out of the button; the strip rises in; the glass bars drop in | black |
| 24.5 | click play | Auto-scroll: the strip drifts up; the bars slide away | drift |
| 25 | long-press | Ring fills; drift stops (finger pauses it) | pause |
| 25.5 | tap centre | Glass bars drop in (top: back, title pill, play, sliders) and rise (bottom) | bars |
| 26 | click sliders | Reader sheet rises: modes Strip / Paged / Right to left, speeds Slow, Normal, Fast, Rapid, Background Black / Dark grey | sheet |
| 26.5 | click Right to left | Yellow pill slides from Strip to "Right to left" | the pill |
| 26.75 | – | Strip regroups into the double page (two pages push in, right page first) | spread |
| 27 | press the handle | Cursor grabs the sheet's handle (white ripple) | the grab |
| 27.5 | drag down | Sheet follows the cursor and drops away | the cursor |
| 28 | double-tap bubble | Page and camera zoom together to 2× on the lettered bubble (cap 4×) | zoom |
| 29 | swipe | Zoom springs back; the "End of Chapter 1" row pushes in from the left with the next chapter joined on below it | page |

## Library and Updates: beats 30–35 (15–18 s)

| B | Cursor | What moves | Carry |
|---|---|---|---|
| 30 | click back | The black draws back to the column and the reader pushes off to the right onto the series hero; 30.25 click save: bookmark turns solid | the icon |
| 30.5 | – | The series pushes back to Discover; the glass tab bar capsule rises from the bottom edge (Discover, Library, Books, Updates, Sources) | tab bar |
| 31 | click Library | Yellow circle flows (liquid) from Discover to Library; the grid lands, covers pop with unread badges | circle |
| 32 | long-press cover-04 | Ring fills; cover squeezes; menu pops from zero: Open, Mark read, Move to category, Remove | menu |
| 33 | click Move to category | Menu shrinks into a new pill "Weekend" (film) above the grid; "All" pill moves left | pill |
| 34 | click Updates | Circle flows one tab right past Books to Updates; count badge "3" pops on the tab icon | circle |
| 35 | click refresh | Unread / Recent pills slide in; the refresh button turns once; cards land | the tab bar |

## Shelf and Share: beats 36–41 (18–21 s)

| B | Cursor | What moves | Carry |
|---|---|---|---|
| 36 | click Books | Circle flows one tab left to Books; the shelf unfolds: Alice cover, "Alice's Adventures in Wonderland", "2%" | the cover |
| 37 | long-press Alice | Ring fills; menu pops; Share | menu |
| 38 | click Share | Menu stretches up into the sheet: "Send", subtitle the book's title, "Looking nearby…" | sheet |
| 39 | – | "Mira's phone" (film) pops in as a pill | pill |
| 40 | click it | "Sending to Mira's phone" with the bar drawing across | bar |
| 41 | – | Bar full; tick pops from zero; sheet drops | the cover |

## Books: beats 42–51 (21–26 s)

| B | Cursor | What moves | Carry |
|---|---|---|---|
| 42 | click Alice | Cover expands into the page: Paper (#FAFAF8 on #1A1A1A), "Down the Rabbit-Hole", real Lora text | page |
| 43 | press and hold "the" | Ring fills; the word selects | selection |
| 43.5 | drag handles | Handles drag over "the book her sister was reading" | handles |
| 44 | – | Menu pops: four dots (Yellow, Green, Blue, Pink), Note, Copy | menu |
| 44.5 | click Yellow | Highlight paints across the phrase left to right | highlight |
| 45 | click sliders | "Type and theme" sheet rises: Pages / Scroll, Font Lora, Text size, Line spacing | sheet |
| 45.5–48 | – | Theme swatches Auto, Paper, Sepia, Ink, Night, Black pop one per half beat | swatches |
| 48.5 | drag Text size knob | Knob moves right; the page text reflows live on the spring | reflow |
| 49 | click Night | The yellow ring springs from Auto to Night | the ring |
| 49.5–50.1 | – | A dark circle (#1C1C1E) grows out of the swatch and floods the page past the corners (0.3 s); text inside the mask turns #D6D6DA (own mask, no fade) | flood |
| 51 | drag sheet down | Sheet drops away; Night page left | page |

## Listen: beats 52–55 (26–28 s)

| B | Cursor | What moves | Carry |
|---|---|---|---|
| 52 | click headphones | Glass bar pops from zero: previous, play (yellow), next, "1x", close | bar |
| 52.5 | click play | Play becomes pause; a yellow band draws across the page's last sentence | band |
| 53 | – | The page turns itself with a push; the band draws across the first sentence of the new page | page |
| 54 | – | Band moves to the next sentence (the old band retracts, the new one draws) | band |
| 55 | click "1x" | Pill steps to "1.5x"; band moves to "I shall be late!" and stays there through Stage (still being read) | band |

## Stage: beats 56–59 (28–30 s), the breakdown

| B | Cursor | What moves | Carry |
|---|---|---|---|
| 56 | – | Camera pulls back to the whole iPhone (it has been there since beat 4) | phone |
| 56.6 | – | Camera pushes in on the island (my addition, so the pinch reads) | the island |
| 57 | – | A liquid neck pulls a drop up past the top edge; the neck pinches off (goo) | island |
| 57.9 | – | Camera pulls back out; the drop falls back into the island and it bulges once (it stays an iPhone, as you asked); same Night page | phone |
| 59 | – | Glass pills "iOS" and "Android" pop from zero below it | pills |

## Close: beats 60–63 (30–32 s), the beat returns on 60

| B | Cursor | What moves | Carry |
|---|---|---|---|
| 60 | – | "No accounts", "No ads", "No tracking" pop from zero, one per third of a beat | pills |
| 61 | – | Yellow floods out of the phone past the corners and covers it | yellow |
| 62 | – | Black contracts into the yellow pill, then into the "i" dot | dot |
| 63 | – | Letters of "Akidev" spring back out of the dot and land on the next downbeat = frame 0 | wordmark |

## Song we need (for the Mixkit search)

Downbeat at 0:00 (or a clean cut point), around 120 BPM, 16 bars. Drop on bar 5 (beat 16, 8 s). A quiet
breakdown on bar 15 (beats 56–59, 28 s). Full beat back on bar 16 (beat 60, 30 s). No stock track will have exactly this. The plan:
find one with the right feel, then edit it on bar lines (cut or repeat whole bars) so the drop and the breakdown land
here. The map does not change.

## Decisions (you agreed to all three on 2026-11-18)

1. **Frame 0.** You asked for "rises out of a mask line" at the open AND "last frame = first frame". Both can't be
   true at frame 0. My pick: frame 0 is the landed wordmark (so the film loops), and the mask rise happens in reverse
   at the close (beat 63, letters spring out of the dot). The other choice: start on an empty canvas with the rise, and
   the film doesn't loop.
2. **The spread.** The strip is "The Last Lantern" (the copper-haired lighthouse keeper), but the spread shows the
   swordswoman from "Maple Bridge". Switching to Right to left shouldn't change the series. I suggest making the two
   spread pages again with the lighthouse keeper, in black and white. This costs two more Seedream images.
3. **Bubbles.** The strip and the spread have empty bubbles. I'll letter them in Nunito with short placeholder lines
   (film), for example "There's a light out there."

## Stills for review

- **Open:** beat 3.5: the yellow pill with "What do you read?" rising inside it.
- **Reader:** beat 27.45: the spread just landed under the Reader sheet, yellow pill on "Right to left".
- **Books:** beat 49.7: the Night circle rising out of the swatch, text changing colour at its edge.
- **Stage:** beat 57.45: camera close on the iPhone's top edge, the island's drop pulled up past the bezel, its neck pinching.

## A note on the checker

The onetake checker wants long still stretches and uneven shot lengths. This film has something on every beat and no
hold over 1 s, as you asked, so its stillness and rhythm checks will fail on purpose. I'll still run its framing and
pop checks.
