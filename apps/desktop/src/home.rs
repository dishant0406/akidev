//! The home screen: a featured banner and rows from every catalogue.

use crate::backend::Backend;
use crate::hero::Hero;
use crate::progress::Entry;
use crate::row::{MediaRow, RowSpec};
use crate::task::load;
use akidev_core::model::{Filters, Kind, Media, Sort};
use akidev_core::providers;
use akidev_core::screens::HomeRow;
use akidev_core::search::interleave;
use gpui_kit::*;

/// The front of one catalogue, for the banner.
async fn featured_from(kind: Kind) -> Vec<Media> {
    let filters = Filters {
        sort: Some(Sort::Trending),
        page: 1,
        per_page: 6,
        ..Default::default()
    };
    providers::list(kind, &filters)
        .await
        .map(|page| page.results)
        .unwrap_or_default()
}

/// The rows of the home screen, as the sources document lists them.
fn rows() -> Vec<RowSpec> {
    specs(akidev_core::sources::screens().home)
}

fn specs(home: Vec<HomeRow>) -> Vec<RowSpec> {
    home.into_iter()
        .map(|row| RowSpec {
            title: row.title,
            kind: row.kind,
            provider: row.provider,
            style: row.style,
            filters: Filters {
                sort: Some(row.sort),
                genre: row.genre,
                page: 1,
                per_page: 24,
                ..Default::default()
            },
        })
        .collect()
}

/// The catalogues the banner draws from, one for each tab.
fn kinds() -> Vec<Kind> {
    akidev_core::sources::screens()
        .tabs
        .iter()
        .map(|tab| tab.kind)
        .collect()
}

/// Starts asking for what the home screen shows, before there is a window to show it in. The
/// screen then finds the requests already on their way instead of making them itself.
pub fn prefetch() {
    for kind in kinds() {
        crate::runtime::spawn(featured_from(kind));
    }
    for spec in rows() {
        crate::runtime::spawn(async move {
            providers::list_row(spec.kind, spec.provider.as_deref(), &spec.filters).await
        });
    }
}

pub struct HomePage {
    hero: Entity<Hero>,
    /// What the viewer was part way through when the "Continue watching" row was last built.
    watching: Vec<Entry>,
    continuing: Option<Entity<MediaRow>>,
    /// The rows the page was built from, so a plugin that is switched on or off changes them.
    home: Vec<HomeRow>,
    rows: Vec<Entity<MediaRow>>,
}

impl HomePage {
    pub fn new(cx: &mut Context<Self>) -> HomePage {
        let hero = cx.new(|_| Hero::new());
        let watching = cx.global::<Backend>().progress.recent().to_vec();
        let continuing = Self::continuing(&watching, cx);
        let home = akidev_core::sources::screens().home;
        let rows = Self::build_rows(&home, cx);

        // The banner draws from the front of each catalogue, so it features all three.
        let hero_handle = hero.clone();
        load(
            cx,
            async {
                let lists = futures::future::join_all(
                    kinds()
                        .into_iter()
                        .map(|kind| async move { featured_from(kind).await }),
                )
                .await;
                interleave(lists, 9)
                    .into_iter()
                    .filter(|media: &Media| media.banner.is_some())
                    .collect::<Vec<_>>()
            },
            move |_, featured, cx| {
                hero_handle.update(cx, |hero, cx| hero.set_items(featured, cx));
            },
        );
        HomePage {
            hero,
            watching,
            continuing,
            home,
            rows,
        }
    }

    fn build_rows(home: &[HomeRow], cx: &mut Context<Self>) -> Vec<Entity<MediaRow>> {
        specs(home.to_vec())
            .into_iter()
            .map(|spec| cx.new(|cx| MediaRow::loading(spec, cx)))
            .collect()
    }

    fn continuing(watching: &[Entry], cx: &mut Context<Self>) -> Option<Entity<MediaRow>> {
        (!watching.is_empty()).then(|| cx.new(|_| MediaRow::continuing(watching)))
    }
}

impl Render for HomePage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // The page is kept while the viewer watches something and comes back, so the row of what
        // they are part way through is brought up to date whenever it is shown.
        if cx.global::<Backend>().progress.recent() != self.watching.as_slice() {
            self.watching = cx.global::<Backend>().progress.recent().to_vec();
            self.continuing = Self::continuing(&self.watching, cx);
        }
        let home = akidev_core::sources::screens().home;
        if home != self.home {
            self.rows = Self::build_rows(&home, cx);
            self.home = home;
        }
        div()
            .pb_16()
            .child(self.hero.clone())
            .children(self.continuing.clone())
            .children(self.rows.iter().cloned())
    }
}
