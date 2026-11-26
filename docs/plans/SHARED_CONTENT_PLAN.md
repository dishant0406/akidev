# Shared content plan: one copy and one set of tokens for both phone apps

Goal: the iPhone app and the Android app say the same words and use the same colors. Today each app keeps its own copy of both. This plan puts one source of truth in `content/manga/` and generates the Swift and Kotlin code from it. Status (2026-11-26): steps 1 to 4 are done on the branch `repo-structure` (typed constants, one JSON file per area). Step 5 (store text) is not done. Settle the `Ios`/`Android` key pairs one by one.

## What exists today

- Colors, radii and other tokens: `apps/manga-ios/App/UI/Tokens.swift` (68 lines) and `apps/manga-android/design/src/main/kotlin/live/akidev/manga/design/AkiTokens.kt` (117 lines). Same values, written twice by hand.
- Copy: inline in the code on both sides. The iPhone app has about 165 text literals in `Text(`, `Label(`, `Button(` and `navigationTitle(`. The Android app has no `strings.xml`; its text is inline in Kotlin too (about 70 found by a first search, the real count comes from the first extraction run). Commit `4f528b8` already made the vocabulary the same in both apps, so most strings match.
- Icons are already shared: `content/manga/fetch-icons.py` writes both apps. This is the model to copy.
- Store text and screenshots: `content/manga/store/android/`. The iPhone app has no copy of them in the repo.

## Target

```
content/manga/
├── tokens.json         colors (light and dark), radii, spacing, type sizes
├── copy/en.json        every user-facing string, by key
├── icons/              (later) the icon list that fetch-icons.py reads
├── store/              listing text and screenshots, one folder per store
├── generate.py         reads tokens.json and copy/en.json, writes the code below
├── fetch-icons.py      unchanged
└── demo-server.py      unchanged
```

`generate.py` writes two files per app. Both are marked "Generated: do not edit", like `AkiIconSet.kt` is today.

- iPhone: `apps/manga-ios/App/UI/Generated/Tokens.swift` and `Copy.swift`
- Android: `apps/manga-android/design/.../Generated/AkiTokens.kt` and `Copy.kt`

The hand-written `Tokens.swift` and `AkiTokens.kt` keep only what is platform code (the adaptive color helper, the glass effects). They read the generated values.

## Decision needed: the shape of the copy

Option A (my pick): typed constants. `Copy.series.chapters` in both languages. A string with a value becomes a small function: `Copy.library.count(n)`. Plain code, easy to read, the compiler finds a missing key. No translation support.

Option B: the native formats. `generate.py` writes `Localizable.xcstrings` and `strings.xml`. Translation-ready and plural-aware. But every call site changes to `String(localized:)` and `stringResource(...)`, placeholders differ (`%@` against `%s`), and plurals need extra rules. More moving parts.

The app is English only today. Option A is simpler. Moving to option B later is a change inside `generate.py` and the one call-site pattern.

## Order of work

1. **Tokens.** Write `tokens.json` from the two token files, write the generator for tokens, and replace the hand-written values with the generated ones. Check: both apps build, and a screenshot of one screen on each looks the same as before.
2. **Copy extraction, iPhone.** List every literal, give each a key by screen (`home.search.placeholder`), write `copy/en.json`, generate `Copy.swift`, replace the literals. Check: build, and `git grep` finds no literal left.
3. **Copy, Android.** The same, using the same keys. A string that differs between the apps is a bug to settle once: pick one wording.
4. **A guard.** `check-copy.sh`, next to the existing `check-ui.sh`: it fails on a text literal in a screen file. Run it in the release scripts.
5. **Store text.** Move the Play listing text into `copy/` so the App Store listing can use it, and add an iPhone `store/ios/` folder.

Each step is its own commit. Steps 2 and 3 are the large ones: touch one screen folder at a time and build after each.

## Risks

- A string with parts (a name inside a sentence) must become one message with a value, not pieces glued together. See the writing rules in `CLAUDE.md`.
- Some text comes from the server or the sources, not from the app. It does not go in `copy/`.
- The generated files must be committed, so a build does not need Python. Add a check that `generate.py` gives the same files as the committed ones.
