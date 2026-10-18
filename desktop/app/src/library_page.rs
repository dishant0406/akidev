//! The Library screen: what is being saved, what is saved, and the collections.

use crate::backend::Backend;
use crate::design::button::{icon_button, primary_button, secondary_button, tab};
use crate::design::state::empty_state;
use crate::design::tokens::{
    ACCENT, DANGER, GUTTER, LINE, RAISED, TEXT, TEXT_DIM, TEXT_FAINT, TOP_BAR, WASH, WASH_2, color,
    font, radius, size,
};
use crate::route::{Route, navigate};
use crate::runtime;
use akidev_core::music::{Credit, Track};
use akidev_library::download::Progress;
use akidev_library::store::{Collection, Item, Kind, State};
use gpui_kit::assets::IconName;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{Icon, Sizable as _};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use std::time::Duration;

const POSTER_WIDTH: f32 = 56.;
const POSTER_HEIGHT: f32 = 84.;

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Downloading,
    Saved,
    Collections,
}

pub struct LibraryPage {
    tab: Tab,
    items: Vec<Item>,
    collections: Vec<Collection>,
    /// Free space on the drive the library is on, and what the saved items take.
    free: u64,
    saved_bytes: u64,
    /// Why the library could not start, when it could not.
    error: Option<String>,
    /// The item waiting for a second press to be removed.
    removing: Option<String>,
    /// The item whose collections are showing.
    choosing: Option<String>,
    name: Option<Entity<InputState>>,
    _refresh: Task<()>,
}

impl LibraryPage {
    pub fn new(cx: &mut Context<Self>) -> LibraryPage {
        // Starting the downloader here means a visit to the screen is enough to carry on with what
        // was left unfinished.
        let library = cx.global::<Backend>().library.clone();
        cx.spawn(async move |page, cx| {
            // The downloader needs the torrent runtime, not the window's.
            let started = runtime::run(async move { library.get().await }).await;
            if let Some(Err(error)) = started {
                let _ = page.update(cx, |page, cx| {
                    page.error = Some(error);
                    cx.notify();
                });
            }
        })
        .detach();
        let refresh = cx.spawn(async move |page, cx| {
            loop {
                if page.update(cx, |page, cx| page.refresh(cx)).is_err() {
                    break;
                }
                cx.background_executor()
                    .timer(Duration::from_millis(500))
                    .await;
            }
        });
        LibraryPage {
            tab: Tab::Downloading,
            items: Vec::new(),
            collections: Vec::new(),
            free: 0,
            saved_bytes: 0,
            error: None,
            removing: None,
            choosing: None,
            name: None,
            _refresh: refresh,
        }
    }

    /// Reads the list again. Cheap, and done a couple of times a second so progress moves.
    fn refresh(&mut self, cx: &mut Context<Self>) {
        let library = &cx.global::<Backend>().library;
        let Ok(store) = library.store() else { return };
        self.items = store.items().unwrap_or_default();
        self.collections = store.collections().unwrap_or_default();
        self.saved_bytes = self
            .items
            .iter()
            .filter(|item| item.state == State::Ready)
            .map(|item| item.bytes)
            .sum();
        self.free = library.free_space();
        cx.notify();
    }

    fn progress(&self, id: &str, cx: &App) -> Option<Progress> {
        cx.global::<Backend>().library.running()?.progress(id)
    }

    fn pause(&mut self, id: String, cx: &mut Context<Self>) {
        if let Some(library) = cx.global::<Backend>().library.running() {
            let _ = library.pause(&id);
        }
        self.refresh(cx);
    }

    fn resume(&mut self, id: String, cx: &mut Context<Self>) {
        if let Some(library) = cx.global::<Backend>().library.running() {
            let _ = library.resume(&id);
        }
        self.refresh(cx);
    }

    fn remove(&mut self, id: String, cx: &mut Context<Self>) {
        self.removing = None;
        if let Some(library) = cx.global::<Backend>().library.running() {
            runtime::spawn(async move {
                if let Err(error) = library.remove(&id).await {
                    tracing::warn!("[library] could not remove {id}: {error:#}");
                }
            });
        }
        cx.notify();
    }

    fn toggle_in(&mut self, collection: i64, item: String, cx: &mut Context<Self>) {
        let Ok(store) = cx.global::<Backend>().library.store() else {
            return;
        };
        let inside = self
            .collections
            .iter()
            .any(|candidate| candidate.id == collection && candidate.items.contains(&item));
        let _ = if inside {
            store.remove_from_collection(collection, &item)
        } else {
            store.add_to_collection(collection, &item)
        };
        self.refresh(cx);
    }

    fn create_collection(&mut self, cx: &mut Context<Self>) {
        let Some(input) = &self.name else { return };
        let name = input.read(cx).value().trim().to_string();
        if name.is_empty() {
            return;
        }
        if let Ok(store) = cx.global::<Backend>().library.store() {
            let _ = store.add_collection(&name);
        }
        self.refresh(cx);
    }

    fn row(&self, item: &Item, cx: &mut Context<Self>) -> Stateful<Div> {
        let id = item.id.clone();
        let progress = self.progress(&item.id, cx);
        let status = status_text(item, progress.as_ref());
        let fraction = match (&progress, item.state) {
            (Some(progress), State::Active) if progress.total > 0 => {
                Some(progress.done as f32 / progress.total as f32)
            }
            _ => None,
        };
        let poster = match &item.poster {
            Some(url) => img(url.clone())
                .id(SharedString::from(format!("poster-{}", item.id)))
                .size_full()
                .object_fit(ObjectFit::Cover)
                .into_any_element(),
            None => div().size_full().into_any_element(),
        };
        let controls = self.controls(item, cx);
        let ready = item.state == State::Ready;
        let opened = item.clone();
        div()
            .id(SharedString::from(format!("item-{}", item.id)))
            .flex()
            .items_center()
            .gap_4()
            .py_3()
            .border_b_1()
            .border_color(color(LINE))
            .child(
                div()
                    .flex_none()
                    .w(px(POSTER_WIDTH))
                    .h(px(POSTER_HEIGHT))
                    .rounded(px(radius::BADGE))
                    .overflow_hidden()
                    .bg(color(RAISED))
                    .child(poster),
            )
            .child(
                div()
                    .id(SharedString::from(format!("open-{id}")))
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .when(ready, |column| {
                        column
                            .cursor_pointer()
                            .on_click(move |_, _, cx| open(&opened, cx))
                    })
                    .child(
                        div()
                            .truncate()
                            .text_size(px(size::BODY + 1.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(color(TEXT))
                            .child(item.title.clone()),
                    )
                    .child(
                        div()
                            .truncate()
                            .text_size(px(13.))
                            .text_color(color(TEXT_DIM))
                            .child(item.subtitle.clone()),
                    )
                    .children(fraction.map(|fraction| {
                        div()
                            .mt_1()
                            .h(px(3.))
                            .w_full()
                            .rounded_full()
                            .bg(color(TEXT).opacity(WASH_2))
                            .child(
                                div()
                                    .h_full()
                                    .w(relative(fraction.clamp(0., 1.)))
                                    .rounded_full()
                                    .bg(color(ACCENT)),
                            )
                    }))
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(color(if item.state == State::Failed {
                                DANGER
                            } else {
                                TEXT_FAINT
                            }))
                            .child(status),
                    ),
            )
            .child(controls)
    }

    /// The buttons at the end of a row, which depend on where the item is.
    fn controls(&self, item: &Item, cx: &mut Context<Self>) -> Div {
        let id = item.id.clone();
        let mut controls = div().flex().items_center().gap_1();
        match item.state {
            State::Active | State::Queued => {
                let pausing = id.clone();
                controls = controls.child(icon_button(
                    SharedString::from(format!("pause-{id}")),
                    IconName::Pause,
                    cx.listener(move |page, _, _, cx| page.pause(pausing.clone(), cx)),
                ));
            }
            State::Paused | State::Failed => {
                let resuming = id.clone();
                controls = controls.child(icon_button(
                    SharedString::from(format!("resume-{id}")),
                    IconName::Play,
                    cx.listener(move |page, _, _, cx| page.resume(resuming.clone(), cx)),
                ));
            }
            State::Sealing | State::Ready => {}
        }
        if item.state == State::Ready {
            let choosing = id.clone();
            controls = controls.child(icon_button(
                SharedString::from(format!("collections-{id}")),
                IconName::Layers,
                cx.listener(move |page, _, _, cx| {
                    page.choosing = if page.choosing.as_ref() == Some(&choosing) {
                        None
                    } else {
                        Some(choosing.clone())
                    };
                    cx.notify();
                }),
            ));
        }
        if item.state != State::Sealing {
            let asking = self.removing.as_ref() == Some(&id);
            let removing = id.clone();
            controls = controls.child(if asking {
                // Removing a saved copy is for good: a second press says it is meant.
                secondary_button(
                    SharedString::from(format!("confirm-{id}")),
                    "Remove",
                    None,
                    cx.listener(move |page, _, _, cx| page.remove(removing.clone(), cx)),
                )
                .into_any_element()
            } else {
                let removing = id.clone();
                icon_button(
                    SharedString::from(format!("remove-{id}")),
                    IconName::X,
                    cx.listener(move |page, _, _, cx| {
                        page.removing = Some(removing.clone());
                        cx.notify();
                    }),
                )
                .into_any_element()
            });
        }
        controls
    }

    /// The collections an item can be put in, shown under it when its button is pressed.
    fn chooser(&self, item: &Item, cx: &mut Context<Self>) -> Div {
        let mut chips = div().flex().flex_wrap().gap_2().py_2();
        for collection in &self.collections {
            let inside = collection.items.contains(&item.id);
            let (collection_id, item_id) = (collection.id, item.id.clone());
            chips = chips.child(
                div()
                    .id(SharedString::from(format!(
                        "chip-{}-{}",
                        collection.id, item.id
                    )))
                    .flex()
                    .items_center()
                    .gap_1p5()
                    .px_3()
                    .h(px(28.))
                    .rounded_full()
                    .cursor_pointer()
                    .text_size(px(13.))
                    .font_weight(FontWeight::MEDIUM)
                    .bg(color(TEXT).opacity(if inside { WASH_2 } else { WASH }))
                    .text_color(color(if inside { ACCENT } else { TEXT_DIM }))
                    .on_click(cx.listener(move |page, _, _, cx| {
                        page.toggle_in(collection_id, item_id.clone(), cx)
                    }))
                    .when(inside, |chip| {
                        chip.child(Icon::new(IconName::Check).small())
                    })
                    .child(collection.name.clone()),
            );
        }
        if self.collections.is_empty() {
            chips = chips.child(
                div()
                    .text_size(px(13.))
                    .text_color(color(TEXT_FAINT))
                    .child("No collections yet. Make one in the Collections tab."),
            );
        }
        chips
    }

    fn list(&self, wanted: impl Fn(&Item) -> bool, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let mut rows = Vec::new();
        for item in self.items.iter().filter(|item| wanted(item)) {
            rows.push(self.row(item, cx).into_any_element());
            if self.choosing.as_ref() == Some(&item.id) {
                rows.push(self.chooser(item, cx).into_any_element());
            }
        }
        rows
    }

    fn collections_tab(&self, cx: &mut Context<Self>) -> Div {
        let maker = div()
            .flex()
            .items_center()
            .gap_3()
            .child(
                div().flex_1().children(
                    self.name
                        .as_ref()
                        .map(|name| Input::new(name).appearance(true)),
                ),
            )
            .child(primary_button(
                "make-collection",
                "Make collection",
                None,
                cx.listener(|page, _, _, cx| page.create_collection(cx)),
            ));
        let mut tab = div().flex().flex_col().gap_6().child(maker);
        if self.collections.is_empty() {
            return tab.child(empty_state(
                "No collections yet",
                "Group what you have saved your own way: a watchlist for a trip, a few favourites, an album.",
            ));
        }
        for collection in &self.collections {
            let removing = collection.id;
            let members: Vec<&Item> = collection
                .items
                .iter()
                .filter_map(|id| self.items.iter().find(|item| &item.id == id))
                .collect();
            let mut block = div().flex().flex_col().child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .font_family(font::TITLE)
                            .text_size(px(size::ROW_TITLE))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(color(TEXT))
                            .child(format!("{} · {}", collection.name, members.len())),
                    )
                    .child(icon_button(
                        SharedString::from(format!("drop-{}", collection.id)),
                        IconName::X,
                        cx.listener(move |page, _, _, cx| {
                            if let Ok(store) = cx.global::<Backend>().library.store() {
                                let _ = store.remove_collection(removing);
                            }
                            page.refresh(cx);
                        }),
                    )),
            );
            for item in members {
                block = block.child(self.row(item, cx));
            }
            tab = tab.child(block);
        }
        tab
    }
}

/// Plays a saved item: a film or episode on the watch page, a track in the music player.
fn open(item: &Item, cx: &mut App) {
    if item.kind == Kind::Music {
        let track = Track {
            id: item.media_id.clone(),
            title: item.title.clone(),
            artists: vec![Credit {
                id: None,
                name: item.subtitle.clone(),
            }],
            album: None,
            duration: None,
            explicit: false,
            cover: item.poster.clone(),
        };
        crate::music_player::play(vec![track], 0, cx);
    } else {
        navigate(
            Route::Watch {
                id: item.media_id.clone(),
            },
            cx,
        );
    }
}

/// What a row says under its title.
fn status_text(item: &Item, progress: Option<&Progress>) -> String {
    match item.state {
        State::Queued => "Waiting for its turn".into(),
        State::Active => match progress {
            Some(progress) if progress.total > 0 && progress.note == "Downloading" => format!(
                "{} of {} · {}/s",
                size_text(progress.done),
                size_text(progress.total),
                size_text(progress.speed)
            ),
            Some(progress) if !progress.note.is_empty() => progress.note.clone(),
            _ => "Starting".into(),
        },
        State::Paused => "Paused".into(),
        State::Sealing => "Locking it into your library".into(),
        State::Ready => size_text(item.bytes),
        State::Failed => item
            .note
            .clone()
            .unwrap_or_else(|| "It could not be saved.".into()),
    }
}

fn size_text(bytes: u64) -> String {
    const MIB: f64 = 1024. * 1024.;
    let bytes = bytes as f64;
    if bytes >= 1024. * MIB {
        format!("{:.1} GB", bytes / (1024. * MIB))
    } else if bytes >= MIB {
        format!("{:.0} MB", bytes / MIB)
    } else {
        format!("{:.0} KB", bytes / 1024.)
    }
}

impl Render for LibraryPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.name.is_none() {
            self.name =
                Some(cx.new(|cx| InputState::new(window, cx).placeholder("Name a collection")));
        }
        let downloading = |item: &Item| item.state != State::Ready;
        let saved = |item: &Item| item.state == State::Ready;
        let counts = (
            self.items.iter().filter(|item| downloading(item)).count(),
            self.items.iter().filter(|item| saved(item)).count(),
        );
        let body = match self.tab {
            Tab::Downloading => {
                let rows = self.list(downloading, cx);
                if rows.is_empty() {
                    div().child(empty_state(
                        "Nothing is downloading",
                        "Press the download button on an episode or a film in the desktop app, and it shows up here.",
                    ))
                } else {
                    div().flex().flex_col().children(rows)
                }
            }
            Tab::Saved => {
                let rows = self.list(saved, cx);
                if rows.is_empty() {
                    div().child(empty_state(
                        "Nothing saved yet",
                        "What you download plays here with no buffering, even without a connection. It is private to Akidev.",
                    ))
                } else {
                    div().flex().flex_col().children(rows)
                }
            }
            Tab::Collections => self.collections_tab(cx),
        };
        let tabs = div()
            .flex()
            .items_center()
            .gap_1()
            .child(tab(
                "library-downloading",
                &format!("Downloading · {}", counts.0),
                self.tab == Tab::Downloading,
                cx.listener(|page, _, _, cx| {
                    page.tab = Tab::Downloading;
                    cx.notify();
                }),
            ))
            .child(tab(
                "library-saved",
                &format!("Saved · {}", counts.1),
                self.tab == Tab::Saved,
                cx.listener(|page, _, _, cx| {
                    page.tab = Tab::Saved;
                    cx.notify();
                }),
            ))
            .child(tab(
                "library-collections",
                "Collections",
                self.tab == Tab::Collections,
                cx.listener(|page, _, _, cx| {
                    page.tab = Tab::Collections;
                    cx.notify();
                }),
            ));
        div()
            .flex()
            .justify_center()
            .pt(px(TOP_BAR + 28.))
            .pb_16()
            .px(px(GUTTER))
            .child(
                div()
                    .w_full()
                    .max_w(px(820.))
                    .flex()
                    .flex_col()
                    .gap_6()
                    .child(
                        div()
                            .flex()
                            .items_end()
                            .justify_between()
                            .child(
                                div()
                                    .text_size(px(size::DISPLAY))
                                    .font_family(font::TITLE)
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(color(TEXT))
                                    .child("Library"),
                            )
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .text_color(color(TEXT_FAINT))
                                    .child(format!(
                                        "{} saved · {} free",
                                        size_text(self.saved_bytes),
                                        size_text(self.free)
                                    )),
                            ),
                    )
                    .children(self.error.clone().map(|error| {
                        div()
                            .text_size(px(13.))
                            .text_color(color(DANGER))
                            .child(format!("Downloads could not start: {error}"))
                    }))
                    .child(tabs)
                    .child(body),
            )
    }
}
