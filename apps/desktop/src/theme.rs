//! The window theme for the library widgets (inputs, switches). It carries the same values as `design/tokens.rs`.

use gpui_kit::App;
use gpui_kit::component::theme::{Theme, ThemeMode, ThemeSet};
use std::rc::Rc;

/// The interface face and the title face, kept small: Allo in the three weights the screens use and
/// Katagami Sans in two for titles (Latin letters, the accents real titles use and punctuation
/// only). Each weight is its own file with its own name, so the text system can tell them apart. Both
/// are SIL Open Font License. Every font the UI draws is named in `design::tokens::font`.
pub fn fonts(cx: &mut App) {
    use std::borrow::Cow::Borrowed;
    let faces = [
        &include_bytes!("../assets/fonts/Allo-Regular.ttf")[..],
        &include_bytes!("../assets/fonts/Allo-Medium.ttf")[..],
        &include_bytes!("../assets/fonts/Allo-SemiBold.ttf")[..],
        &include_bytes!("../assets/fonts/KatagamiSans-Regular.ttf")[..],
        &include_bytes!("../assets/fonts/KatagamiSans-SemiBold.ttf")[..],
    ];
    if let Err(error) = cx
        .text_system()
        .add_fonts(faces.into_iter().map(Borrowed).collect())
    {
        tracing::warn!("[theme] could not load the fonts: {error}");
    }
}

const THEME: &str = include_str!("../assets/theme.json");

pub fn apply(cx: &mut App) {
    let set: ThemeSet = serde_json::from_str(THEME).expect("theme.json is a valid theme");
    let config = Rc::new(
        set.themes
            .into_iter()
            .next()
            .expect("theme.json holds a theme"),
    );
    Theme::update(cx, |theme| theme.dark_theme = config);
    Theme::change(ThemeMode::Dark, None, cx);
}
