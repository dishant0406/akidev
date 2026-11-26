# Akidev

## Design

Every screen, on the website and in the desktop app, follows `design/DESIGN_PLAN.md`: read it before
changing how anything looks, moves or reads. It says what the interface is meant to feel like and which
patterns to keep (one warm ink palette, amber only for state such as progress, active and focus, three
button looks, state cards for empty and error screens, translucent chrome).

The values live in code and are the source of truth for numbers:

- Web: `apps/web/src/app/globals.css` and `tailwind.config.ts`, with the components in
  `apps/web/src/components`.
- Desktop: `apps/desktop/src/design/` (`tokens.rs`, `button.rs`, `state.rs`).

Reuse those components and tokens instead of adding colours or button styles. Use the `apple-design` skill
for motion, responsiveness and interaction details. The two apps should look like one product: when a
screen changes in one, check the other.

The manga app (`apps/manga-ios/` and `apps/manga-android/`, with `crates/manga*`) is a separate product with its own look, described in
`design/manga.md`: it does not follow `design/DESIGN_PLAN.md`.
