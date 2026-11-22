//! Settings: how much disk the torrents may use, and how many are kept (two sliders that
//! snap to a few sensible stops, saved as soon as they are let go), and the plugins.

use crate::backend::Backend;
use crate::design::button::ghost_button;
use crate::design::toggle::toggle;
use crate::design::tokens::{
    ACCENT, DANGER, GUTTER, LINE, TEXT, TEXT_DIM, TEXT_FAINT, TOP_BAR, WASH_2, color, font, radius,
    size,
};
use crate::updates::{Shared, Status};
use akidev_core::config::Config;
use akidev_core::plugin::{self, Installed};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;

const GIB: u64 = 1024 * 1024 * 1024;

/// The values a slider can take. Few and round, because nobody needs 37 GiB.
const DISK_STOPS: [u64; 9] = [2, 4, 8, 16, 32, 64, 128, 256, 512];
const TORRENT_STOPS: [u64; 7] = [1, 2, 3, 4, 6, 8, 12];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Limit {
    Disk,
    Torrents,
}

impl Limit {
    fn stops(self) -> &'static [u64] {
        match self {
            Limit::Disk => &DISK_STOPS,
            Limit::Torrents => &TORRENT_STOPS,
        }
    }

    fn value(self, config: &Config) -> u64 {
        match self {
            Limit::Disk => config.max_cache_bytes / GIB,
            Limit::Torrents => config.max_torrents as u64,
        }
    }

    fn set(self, config: &mut Config, value: u64) {
        match self {
            Limit::Disk => config.max_cache_bytes = value * GIB,
            Limit::Torrents => config.max_torrents = value as usize,
        }
    }
}

/// "1 indexer", "2 indexers".
fn counted(count: usize, what: &str) -> String {
    format!("{count} {what}{}", if count == 1 { "" } else { "s" })
}

/// "v1.2, 2 indexers, 1 catalogue": what a plugin brings, leaving out what it does not.
fn plugin_summary(plugin: &Installed) -> String {
    let mut parts = vec![format!("v{}", plugin.version)];
    for (count, what) in [
        (plugin.indexers, "indexer"),
        (plugin.subtitles, "subtitle source"),
        (plugin.catalogues, "catalogue"),
        (plugin.services, "stream service"),
    ] {
        if count > 0 {
            parts.push(counted(count, what));
        }
    }
    parts.join(", ")
}

fn small(text: String, tone: u32) -> Div {
    div()
        .text_size(px(12.))
        .line_height(relative(1.4))
        .text_color(color(tone))
        .child(text)
}

/// A section's name with one line under it.
fn heading(name: &'static str, line: &'static str) -> Div {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .text_size(px(13.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(color(TEXT_DIM))
                .child(name),
        )
        .child(small(line.to_string(), TEXT_FAINT))
}

/// One hairline box holding a text field with its button inside it.
fn field(input: &Entity<InputState>, mask_toggle: bool, button: impl IntoElement) -> Div {
    let input = Input::new(input).appearance(false);
    let input = if mask_toggle {
        input.mask_toggle()
    } else {
        input
    };
    div()
        .flex()
        .items_center()
        .gap_2()
        .pl_3()
        .pr_1()
        .py_1()
        .rounded(px(radius::BUTTON))
        .border_1()
        .border_color(color(LINE))
        .child(div().flex_1().min_w_0().child(input))
        .child(button)
}

pub struct SettingsPage {
    config: Config,
    /// Where each slider was last drawn, so a click can be turned into a stop.
    disk_bar: Rc<Cell<Bounds<Pixels>>>,
    torrents_bar: Rc<Cell<Bounds<Pixels>>>,
    /// The slider being dragged.
    dragging: Option<Limit>,
    /// Where the plugins are kept, and what is there.
    plugin_dir: PathBuf,
    plugins: Vec<Installed>,
    /// The field for the link of a plugin to add. Made when the page is first drawn, which is when
    /// there is a window to make it in.
    link: Option<Entity<InputState>>,
    /// A plugin is being downloaded.
    adding: bool,
    /// What the last add or change came to, and whether it failed.
    note: Option<(String, bool)>,
    /// The field is emptied the next time the page is drawn.
    clear_link: bool,
    /// The field for the key of each stream service a plugin brings, by the service's id. Made when
    /// the page is drawn, like the one above.
    keys: std::collections::BTreeMap<String, Entity<InputState>>,
}

impl SettingsPage {
    pub fn new(cx: &mut Context<Self>) -> SettingsPage {
        // The About row follows the update as it downloads.
        let updates = cx.global::<Shared>().0.clone();
        cx.observe(&updates, |_, _, cx| cx.notify()).detach();
        let plugin_dir = cx
            .global::<Backend>()
            .config_path
            .parent()
            .map(PathBuf::from)
            .unwrap_or_default();
        SettingsPage {
            config: cx.global::<Backend>().config.clone(),
            disk_bar: Rc::new(Cell::new(Bounds::default())),
            torrents_bar: Rc::new(Cell::new(Bounds::default())),
            dragging: None,
            plugins: plugin::load(&plugin_dir),
            plugin_dir,
            link: None,
            adding: false,
            note: None,
            clear_link: false,
            keys: Default::default(),
        }
    }

    fn bar(&self, limit: Limit) -> &Rc<Cell<Bounds<Pixels>>> {
        match limit {
            Limit::Disk => &self.disk_bar,
            Limit::Torrents => &self.torrents_bar,
        }
    }

    /// The index of the stop nearest to `x`, and the one the setting is at.
    fn stop_under(&self, limit: Limit, x: Pixels) -> usize {
        let bar = self.bar(limit).get();
        let stops = limit.stops().len();
        if bar.size.width <= px(0.) {
            return 0;
        }
        let fraction = ((x - bar.origin.x) / bar.size.width).clamp(0., 1.);
        (fraction * (stops - 1) as f32).round() as usize
    }

    fn pick(&mut self, limit: Limit, x: Pixels, cx: &mut Context<Self>) {
        let value = limit.stops()[self.stop_under(limit, x)];
        if limit.value(&self.config) != value {
            limit.set(&mut self.config, value);
            cx.notify();
        }
    }

    /// Keeps what was chosen, and tells the engine what it can use at once.
    fn commit(&mut self, cx: &mut Context<Self>) {
        self.dragging = None;
        let saved = &cx.global::<Backend>().config;
        if *saved != self.config {
            let path = cx.global::<Backend>().config_path.clone();
            match self.config.save(&path) {
                Ok(()) => {
                    cx.global::<Backend>()
                        .streamer
                        .set_config(self.config.clone());
                    cx.global_mut::<Backend>().config = self.config.clone();
                }
                Err(error) => {
                    crate::route::toast(format!("Could not save settings: {error:#}"), cx)
                }
            }
        }
        cx.notify();
    }

    fn toggle_updates(&mut self, cx: &mut Context<Self>) {
        self.config.check_for_updates = !self.config.check_for_updates;
        self.commit(cx);
    }

    /// Downloads the plugin at the link in the field and adds it.
    fn add_plugin(&mut self, cx: &mut Context<Self>) {
        let Some(link) = &self.link else { return };
        let link = link.read(cx).value().trim().to_string();
        if link.is_empty() || self.adding {
            return;
        }
        self.adding = true;
        self.note = None;
        let dir = self.plugin_dir.clone();
        crate::task::load(
            cx,
            async move {
                plugin::install_from_link(&dir, &link)
                    .await
                    .map_err(|error| format!("{error:#}"))
            },
            |page, result, cx| {
                page.adding = false;
                match result {
                    Ok(plugins) => {
                        page.plugins = plugins;
                        page.note = Some(("Plugin added.".into(), false));
                        page.clear_link = true;
                    }
                    Err(error) => page.note = Some((error, true)),
                }
                cx.notify();
            },
        );
        cx.notify();
    }

    /// Keeps the key typed for a stream service, or forgets it when the field is empty.
    fn save_key(&mut self, id: &str, cx: &mut Context<Self>) {
        let Some(input) = self.keys.get(id) else {
            return;
        };
        let typed = input.read(cx).value().trim().to_string();
        if typed.is_empty() {
            self.config.service_keys.remove(id);
        } else {
            self.config.service_keys.insert(id.to_string(), typed);
        }
        self.commit(cx);
    }

    /// Takes in the plugin files that were put in the folder by hand.
    fn reload_plugins(&mut self, cx: &mut Context<Self>) {
        self.plugins = plugin::load(&self.plugin_dir);
        self.note = None;
        cx.notify();
    }

    fn switch_plugin(&mut self, id: &str, on: bool, cx: &mut Context<Self>) {
        match plugin::set_enabled(&self.plugin_dir, id, on) {
            Ok(plugins) => self.plugins = plugins,
            Err(error) => self.note = Some((format!("{error:#}"), true)),
        }
        cx.notify();
    }

    fn remove_plugin(&mut self, id: &str, cx: &mut Context<Self>) {
        match plugin::remove(&self.plugin_dir, id) {
            Ok(plugins) => self.plugins = plugins,
            Err(error) => self.note = Some((format!("{error:#}"), true)),
        }
        cx.notify();
    }

    /// The plugins: the ones that are added, and a field to add another from a link.
    fn plugins_section(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Div {
        let input = self
            .link
            .get_or_insert_with(|| {
                cx.new(|cx| {
                    InputState::new(window, cx).placeholder("https://example.com/plugin.json")
                })
            })
            .clone();
        if self.clear_link {
            self.clear_link = false;
            input.update(cx, |input, cx| input.set_value("", window, cx));
        }
        let rows = self.plugins.iter().map(|each| {
            let (id, on) = (each.id.clone(), each.enabled);
            let (toggle_id, remove_id) = (id.clone(), id.clone());
            let detail = match &each.problem {
                Some(problem) => small(problem.clone(), DANGER),
                None => small(plugin_summary(each), TEXT_FAINT),
            };
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap_6()
                .py_3()
                .border_t_1()
                .border_color(color(LINE))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(px(2.))
                        .child(
                            div()
                                .text_size(px(14.))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(color(TEXT))
                                .child(each.name.clone()),
                        )
                        .child(detail),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_3()
                        .child(ghost_button(
                            SharedString::from(format!("plugin-remove-{remove_id}")),
                            "Remove",
                            cx.listener(move |page, _, _, cx| page.remove_plugin(&remove_id, cx)),
                        ))
                        .child(toggle(
                            SharedString::from(format!("plugin-{toggle_id}")),
                            on,
                            cx.listener(move |page, _, _, cx| page.switch_plugin(&id, !on, cx)),
                        )),
                )
        });
        let folder = self.plugin_dir.join("plugins").display().to_string();
        let folder = match std::env::var("HOME") {
            Ok(home) => folder.replacen(&home, "~", 1),
            Err(_) => folder,
        };
        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(heading(
                "Plugins",
                "Add sources from people you trust. Akidev does not check what they find.",
            ))
            .child(div().flex().flex_col().children(rows))
            .child(field(
                &input,
                false,
                ghost_button(
                    "plugin-add",
                    if self.adding { "Adding…" } else { "Add" },
                    cx.listener(|page, _, _, cx| page.add_plugin(cx)),
                ),
            ))
            .children(
                self.note.as_ref().map(|(text, failed)| {
                    small(text.clone(), if *failed { DANGER } else { TEXT_DIM })
                }),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_6()
                    .child(small(folder, TEXT_FAINT))
                    .child(ghost_button(
                        "plugin-reload",
                        "Reload",
                        cx.listener(|page, _, _, cx| page.reload_plugins(cx)),
                    )),
            )
    }

    /// The stream services the plugins bring, each with a field for its key. Nothing is drawn when no
    /// plugin brings one.
    fn services_section(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Div {
        let services = akidev_core::sources::current().services.clone();
        if services.is_empty() {
            return div();
        }
        let mut rows = Vec::new();
        for service in &services {
            let id = service.id.clone();
            let has_key = self
                .config
                .service_keys
                .get(&id)
                .is_some_and(|key| !key.is_empty());
            let label = service.key_label.clone();
            let input = self
                .keys
                .entry(id.clone())
                .or_insert_with(|| {
                    cx.new(|cx| {
                        InputState::new(window, cx)
                            .placeholder(label.clone().unwrap_or_default())
                            .masked(true)
                    })
                })
                .clone();
            let save_id = id.clone();
            let status = match (&label, has_key) {
                (None, _) => "No key needed",
                (Some(_), true) => "Key saved",
                (Some(_), false) => "No key",
            };
            let mut row = div()
                .flex()
                .flex_col()
                .gap_2()
                .py_3()
                .border_t_1()
                .border_color(color(LINE))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .text_size(px(14.))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(color(TEXT))
                                .child(service.name.clone()),
                        )
                        .child(small(
                            status.to_string(),
                            if has_key { ACCENT } else { TEXT_FAINT },
                        )),
                );
            if label.is_some() {
                row = row.child(field(
                    &input,
                    true,
                    ghost_button(
                        SharedString::from(format!("service-key-{id}")),
                        "Save",
                        cx.listener(move |page, _, _, cx| page.save_key(&save_id, cx)),
                    ),
                ));
                if has_key {
                    row = row.child(small(
                        "Save an empty field to remove the key.".into(),
                        TEXT_FAINT,
                    ));
                }
            }
            rows.push(row);
        }
        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(heading(
                "Stream services",
                "A service plays a title when it has it, and torrents play it when it does not. Keys stay on this computer.",
            ))
            .child(div().flex().flex_col().children(rows))
    }

    fn restore(&mut self, cx: &mut Context<Self>) {
        self.config = Config {
            cache_dir: self.config.cache_dir.clone(),
            // Restoring the limits leaves the update choice as it was.
            check_for_updates: self.config.check_for_updates,
            analytics_id: self.config.analytics_id.clone(),
            // The keys are what the viewer typed, not a limit: restoring the limits leaves them.
            service_keys: self.config.service_keys.clone(),
            ..Config::default()
        };
        self.commit(cx);
    }

    /// One setting: its name, its value large, a track to move it along and what it does.
    fn slider(
        &self,
        limit: Limit,
        name: &'static str,
        unit: &'static str,
        note: &'static str,
        cx: &mut Context<Self>,
    ) -> Div {
        let stops = limit.stops();
        let current = stops
            .iter()
            .enumerate()
            .min_by_key(|(_, stop)| stop.abs_diff(limit.value(&self.config)))
            .map_or(0, |(index, _)| index);
        let at = |index: usize| index as f32 / (stops.len() - 1) as f32;
        let shown = at(current);
        let bounds = self.bar(limit).clone();

        let track = div()
            .id(match limit {
                Limit::Disk => "disk",
                Limit::Torrents => "torrents",
            })
            .relative()
            .w_full()
            .h(px(28.))
            .cursor_pointer()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |page, event: &MouseDownEvent, _, cx| {
                    page.dragging = Some(limit);
                    page.pick(limit, event.position.x, cx);
                }),
            )
            .on_mouse_move(cx.listener(move |page, event: &MouseMoveEvent, _, cx| {
                if page.dragging == Some(limit) && event.pressed_button == Some(MouseButton::Left) {
                    page.pick(limit, event.position.x, cx);
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|page, _, _, cx| page.commit(cx)),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|page, _, _, cx| page.commit(cx)),
            )
            .child(
                canvas(move |area, _, _| bounds.set(area), |_, _, _, _| {})
                    .absolute()
                    .size_full(),
            )
            .child(
                div()
                    .absolute()
                    .top(px(13.))
                    .w_full()
                    .h(px(2.))
                    .rounded_full()
                    .bg(color(TEXT).opacity(WASH_2)),
            )
            .child(
                div()
                    .absolute()
                    .top(px(13.))
                    .w(relative(shown))
                    .h(px(2.))
                    .rounded_full()
                    .bg(color(ACCENT)),
            )
            .children((0..stops.len()).map(|index| {
                div()
                    .absolute()
                    .top(px(10.))
                    .left(relative(at(index)))
                    .w(px(1.))
                    .h(px(8.))
                    .bg(color(TEXT).opacity(if index <= current { 0. } else { 0.22 }))
            }))
            .child(
                div()
                    .absolute()
                    .top(px(7.))
                    .left(relative(shown))
                    .ml(px(-7.))
                    .size(px(14.))
                    .rounded_full()
                    .bg(color(TEXT)),
            );

        let labels = div()
            .relative()
            .w_full()
            .h(px(16.))
            .children(stops.iter().enumerate().map(|(index, stop)| {
                div()
                    .absolute()
                    .left(relative(at(index)))
                    .ml(px(-24.))
                    .w(px(48.))
                    .text_center()
                    .text_size(px(11.))
                    .text_color(if index == current {
                        color(TEXT)
                    } else {
                        color(TEXT_FAINT)
                    })
                    .child(stop.to_string())
            }));

        div()
            .flex()
            .flex_col()
            .child(
                div()
                    .text_size(px(13.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(color(TEXT_DIM))
                    .child(name),
            )
            .child(
                div()
                    .flex()
                    .items_baseline()
                    .gap_2()
                    .pt_1()
                    .pb_4()
                    .child(
                        div()
                            .text_size(px(64.))
                            .line_height(px(68.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(color(TEXT))
                            .child(stops[current].to_string()),
                    )
                    .child(
                        div()
                            .text_size(px(20.))
                            .text_color(color(TEXT_DIM))
                            .child(unit),
                    ),
            )
            .child(track)
            .child(labels)
            .child(
                div()
                    .pt_4()
                    .text_size(px(13.))
                    .text_color(color(TEXT_FAINT))
                    .child(note),
            )
    }
}

impl SettingsPage {
    /// The version, and quietly beside it whether there is a newer one.
    fn about(&self, cx: &mut Context<Self>) -> Div {
        let updates = cx.global::<Shared>().0.clone();
        let status = updates.read(cx).status(cx);
        let press = {
            let updates = updates.clone();
            move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                updates.update(cx, |updates, cx| updates.activate(cx))
            }
        };
        let quiet =
            |text: String, tone: u32| div().text_size(px(13.)).text_color(color(tone)).child(text);
        // The button's padding hangs out so its text lines up with the toggle above.
        let side = div().flex().items_center().gap_1().mr(px(-8.));
        let side = match status {
            Status::UpToDate => side.child(quiet("Up to date".into(), TEXT_FAINT)),
            Status::Available(version) => side
                .child(quiet(format!("{version} available"), TEXT_DIM))
                .child(ghost_button("about-update", "Update", press)),
            Status::Busy(text) => side.child(quiet(text, TEXT_DIM)),
            Status::Failed(sentence) => side.child(quiet(sentence, DANGER)).child(ghost_button(
                "about-retry",
                "Retry",
                press,
            )),
            Status::Page => side.child(ghost_button("about-page", "Open the download page", press)),
        };
        div()
            .flex()
            .items_center()
            .justify_between()
            .gap_8()
            .h(px(28.))
            .child(
                div()
                    .text_size(px(13.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(color(TEXT_DIM))
                    .child(concat!("Akidev ", env!("CARGO_PKG_VERSION"))),
            )
            .child(side)
    }
}

impl Render for SettingsPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let plugins = self.plugins_section(window, cx);
        let services = self.services_section(window, cx);
        let started = &cx.global::<Backend>().started_with;
        let restart = self.config.max_cache_bytes != started.max_cache_bytes
            || self.config.max_torrents != started.max_torrents;
        let custom = self.config.max_cache_bytes != Config::default().max_cache_bytes
            || self.config.max_torrents != Config::default().max_torrents;

        div()
            .flex()
            .justify_center()
            .pt(px(TOP_BAR + 28.))
            .pb_16()
            .px(px(GUTTER))
            .child(
                div()
                    .w_full()
                    .max_w(px(640.))
                    .flex()
                    .flex_col()
                    .gap_12()
                    .child(
                        div()
                            .text_size(px(size::DISPLAY))
                            .font_family(font::TITLE)
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(color(TEXT))
                            .child("Settings"),
                    )
                    .child(self.slider(
                        Limit::Disk,
                        "Disk for torrents",
                        "GiB",
                        "The most the drive is asked to hold while you watch. It is all deleted when you quit.",
                        cx,
                    ))
                    .child(self.slider(
                        Limit::Torrents,
                        "Torrents kept ready",
                        "at once",
                        "Opening another one past this drops the one you watched longest ago.",
                        cx,
                    ))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap_8()
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(
                                        div()
                                            .text_size(px(13.))
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(color(TEXT_DIM))
                                            .child("Check for updates"),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(13.))
                                            .text_color(color(TEXT_FAINT))
                                            .child("Looks for a newer Akidev every few hours. Nothing is installed until you press Update."),
                                    ),
                            )
                            .child(toggle(
                                "check-updates",
                                self.config.check_for_updates,
                                cx.listener(|page, _, _, cx| page.toggle_updates(cx)),
                            )),
                    )
                    .child(plugins)
                    .child(services)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .h(px(20.))
                            .child(div().when(custom, |side| {
                                side.child(
                                    div()
                                        .id("restore")
                                        .text_size(px(13.))
                                        .text_color(color(TEXT_DIM))
                                        .cursor_pointer()
                                        .hover(|style| style.text_color(color(TEXT)))
                                        .on_click(cx.listener(|page, _, _, cx| page.restore(cx)))
                                        .child("Restore defaults"),
                                )
                            }))
                            .child(div().when(restart, |side| {
                                side.flex()
                                    .items_center()
                                    .gap_2()
                                    .text_size(px(13.))
                                    .text_color(color(TEXT_DIM))
                                    .child(div().size(px(6.)).rounded_full().bg(color(ACCENT)))
                                    .child("Takes effect the next time Akidev starts")
                            })),
                    )
                    .child(self.about(cx)),
            )
    }
}
