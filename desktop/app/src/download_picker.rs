//! The panel where a download is set up: which source (release) to save, and which subtitles to
//! keep with it. It opens from every download button of a film or an episode.

use crate::backend::Backend;
use crate::captions::{self, Group};
use crate::design::button::primary_button;
use crate::design::tokens::{ACCENT, LINE, TEXT, TEXT_DIM, TEXT_FAINT, WASH, WASH_2, color};
use crate::downloads;
use crate::panels::{self, SourceAction, SourcesView};
use crate::route::toast;
use akidev_core::stream::{Choice, Plan};
use akidev_core::subtitles::{self, Subtitle};
use akidev_library::store::SubtitleRef;
use gpui_kit::assets::IconName;
use gpui_kit::component::{Icon, Sizable as _};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use std::rc::Rc;
use std::time::Duration;

/// How long the subtitle sites get to answer.
const SUBTITLES_WAIT: Duration = Duration::from_secs(8);

/// Lets any screen open the panel.
pub struct PickerHandle(pub Entity<DownloadPicker>);

impl Global for PickerHandle {}

enum Sources {
    Loading,
    Failed(String),
    Ready(Box<Plan>, Vec<Choice>),
}

pub struct DownloadPicker {
    open: bool,
    /// Counts the times the panel was opened, so an answer for an earlier one is ignored.
    session: u64,
    id: String,
    sources: Sources,
    /// The info hash of the release chosen.
    selected: Option<String>,
    /// The subtitles found, or `None` while the sites are being asked.
    subtitles: Option<Vec<Group>>,
    /// The languages (by code) whose subtitles are kept.
    languages: Vec<String>,
}

impl DownloadPicker {
    pub fn new() -> DownloadPicker {
        DownloadPicker {
            open: false,
            session: 0,
            id: String::new(),
            sources: Sources::Loading,
            selected: None,
            subtitles: None,
            languages: Vec::new(),
        }
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn close(&mut self, cx: &mut Context<Self>) {
        self.open = false;
        self.session += 1;
        cx.notify();
    }

    /// Opens the panel for the film or episode `id`, with `release` selected if it is among the sources.
    pub fn open(&mut self, id: String, release: Option<String>, cx: &mut Context<Self>) {
        self.open = true;
        self.session += 1;
        self.id = id;
        self.selected = release;
        self.languages.clear();
        self.load(cx);
        cx.notify();
    }

    /// Asks for the sources and the subtitles, each as it comes.
    fn load(&mut self, cx: &mut Context<Self>) {
        let session = self.session;
        self.sources = Sources::Loading;
        self.subtitles = None;
        let streamer = cx.global::<Backend>().streamer.clone();
        let id = self.id.clone();
        crate::task::load(
            cx,
            async move { streamer.get().await?.plan(&id).await },
            move |picker, planned, cx| {
                if picker.session != session {
                    return;
                }
                match planned {
                    Ok(plan) => picker.planned(plan, cx),
                    Err(reason) => picker.sources = Sources::Failed(reason),
                }
                cx.notify();
            },
        );
    }

    fn planned(&mut self, plan: Plan, cx: &mut Context<Self>) {
        let choices = plan.choices();
        // What was playing stays chosen if it is listed; otherwise the best one is.
        let listed = |hash: &String| choices.iter().any(|c| c.id.eq_ignore_ascii_case(hash));
        if !self.selected.as_ref().is_some_and(listed) {
            self.selected = choices.first().map(|choice| choice.id.clone());
        }
        let session = self.session;
        let query = subtitles::query_for(&plan.detail, &plan.parsed);
        crate::task::load(
            cx,
            async move { subtitles::find(&query, SUBTITLES_WAIT).await },
            move |picker, found, cx| {
                if picker.session != session {
                    return;
                }
                let groups = captions::group(found);
                // The language the viewer last chose is kept unless they say otherwise.
                let remembered = cx.global::<Backend>().subtitle_language.language();
                if let Some(language) = remembered
                    && let Some(group) = groups
                        .iter()
                        .find(|group| captions::same_language(&group.code, language))
                {
                    picker.languages.push(group.code.clone());
                }
                picker.subtitles = Some(groups);
                cx.notify();
            },
        );
        self.sources = Sources::Ready(Box::new(plan), choices);
    }

    fn toggle_language(&mut self, code: &str, cx: &mut Context<Self>) {
        match self.languages.iter().position(|chosen| chosen == code) {
            Some(at) => {
                self.languages.remove(at);
            }
            None => self.languages.push(code.to_string()),
        }
        cx.notify();
    }

    /// Queues the chosen release with the chosen subtitles, and closes the panel.
    fn download(&mut self, cx: &mut Context<Self>) {
        let (Sources::Ready(plan, _), Some(hash)) = (&self.sources, self.selected.clone()) else {
            return;
        };
        let Some(release) = plan
            .ranked
            .iter()
            .chain(&plan.extra)
            .find(|release| release.info_hash.eq_ignore_ascii_case(&hash))
            .cloned()
        else {
            return;
        };
        let plan = (**plan).clone();
        let wanted = self.subtitle_refs(&release.title);
        let library = cx.global::<Backend>().library.clone();
        self.close(cx);
        cx.spawn(async move |_, cx| {
            let queued = crate::runtime::run(async move {
                downloads::queue_release(&library, &plan, &release, &wanted).await
            })
            .await
            .unwrap_or_else(|| Err("Something went wrong.".into()));
            cx.update(|cx| match queued {
                Ok(title) => toast(format!("Saving {title} for offline"), cx),
                Err(why) => toast(why, cx),
            });
        })
        .detach();
    }

    /// The subtitle of each chosen language that suits `release` best.
    fn subtitle_refs(&self, release: &str) -> Vec<SubtitleRef> {
        let Some(groups) = &self.subtitles else {
            return Vec::new();
        };
        groups
            .iter()
            .filter(|group| self.languages.contains(&group.code))
            .filter_map(|group| {
                let mut options = group.options.clone();
                subtitles::prefer_release(&mut options, release);
                let best: &Subtitle = options
                    .iter()
                    .find(|option| !option.hearing_impaired)
                    .or(options.first())?;
                Some(SubtitleRef {
                    lang: best.lang.clone(),
                    language: best.language.clone(),
                    url: best.url.clone(),
                })
            })
            .collect()
    }

    /// The row of languages under the sources.
    fn languages_row(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let label = div()
            .text_size(px(11.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(color(TEXT_FAINT))
            .child("SUBTITLES TO KEEP");
        let body = match &self.subtitles {
            None => div()
                .text_size(px(13.))
                .text_color(color(TEXT_FAINT))
                .child("Looking for subtitles…"),
            Some(groups) if groups.is_empty() => div()
                .text_size(px(13.))
                .text_color(color(TEXT_FAINT))
                .child("None found. The file's own subtitles are always kept."),
            Some(groups) => div()
                .flex()
                .flex_wrap()
                .gap_2()
                .children(groups.iter().map(|group| {
                    let chosen = self.languages.contains(&group.code);
                    let code = group.code.clone();
                    div()
                        .id(SharedString::from(format!("keep-{}", group.code)))
                        .flex()
                        .items_center()
                        .gap_1p5()
                        .px_3()
                        .h(px(28.))
                        .rounded_full()
                        .cursor_pointer()
                        .text_size(px(13.))
                        .font_weight(FontWeight::MEDIUM)
                        .bg(color(TEXT).opacity(if chosen { WASH_2 } else { WASH }))
                        .text_color(color(if chosen { ACCENT } else { TEXT_DIM }))
                        .on_click(
                            cx.listener(move |picker, _, _, cx| picker.toggle_language(&code, cx)),
                        )
                        .when(chosen, |chip| {
                            chip.child(Icon::new(IconName::Check).small())
                        })
                        .child(group.language.clone())
                })),
        };
        // The sources are the point of the panel: the languages are a short strip that scrolls.
        div()
            .id("keep-languages")
            .flex_none()
            .max_h(px(132.))
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_2()
            .px_4()
            .py_3()
            .border_t_1()
            .border_color(color(LINE))
            .child(label)
            .child(body)
    }
}

impl Render for DownloadPicker {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let picker = cx.entity();
        let view = match &self.sources {
            Sources::Loading => SourcesView::Loading,
            Sources::Failed(reason) => SourcesView::Failed(reason),
            Sources::Ready(_, choices) => SourcesView::Ready(choices),
        };
        let act = Rc::new(move |action: SourceAction, cx: &mut App| {
            picker.update(cx, |picker, cx| {
                match action {
                    SourceAction::Pick(hash) => picker.selected = Some(hash),
                    SourceAction::Retry => picker.load(cx),
                }
                cx.notify();
            })
        });
        let ready = matches!(self.sources, Sources::Ready(..)) && self.selected.is_some();
        let close = cx.listener(|picker, _, _, cx| picker.close(cx));
        let body = div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .child(panels::sources_body(
                view,
                self.selected.as_deref(),
                "Selected",
                act,
            ))
            .child(self.languages_row(cx))
            .child(
                div()
                    .flex_none()
                    .p_4()
                    .border_t_1()
                    .border_color(color(LINE))
                    .child(
                        primary_button(
                            "download-chosen",
                            "Download",
                            Some(IconName::ArrowDownToLine),
                            cx.listener(|picker, _, _, cx| picker.download(cx)),
                        )
                        .w_full()
                        .when(!ready, |button| button.opacity(0.4)),
                    ),
            );
        panels::shell("Download", close, body)
    }
}
