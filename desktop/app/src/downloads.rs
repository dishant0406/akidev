//! Saving films and episodes to the library: finding the release to save, and queueing it.

use crate::backend::{Backend, LazyStreamer};
use crate::library::LazyLibrary;
use crate::route::toast;
use crate::runtime;
use akidev_core::model::Release;
use akidev_core::music::Track;
use akidev_core::stream::Plan;
use akidev_library::store::{Item, Kind, SubtitleRef};
use gpui_kit::*;

/// Opens the panel where the source and the subtitles of a download are chosen. `release` is the
/// one to have selected at first, when there is a natural one (what is playing).
pub fn choose(id: String, release: Option<String>, cx: &mut App) {
    let picker = cx
        .global::<crate::download_picker::PickerHandle>()
        .0
        .clone();
    picker.update(cx, |picker, cx| picker.open(id, release, cx));
}

/// Queues several episodes, one after another, and says how many were added.
pub fn save_all(ids: Vec<String>, cx: &mut App) {
    let backend = cx.global::<Backend>();
    let (library, streamer) = (backend.library.clone(), backend.streamer.clone());
    cx.spawn(async move |cx| {
        let counted = runtime::run(async move {
            let (mut added, mut without) = (0, 0);
            for id in &ids {
                match queue(&library, &streamer, id, None).await {
                    Ok(_) => added += 1,
                    // Already saved or queued is fine; anything else means no source yet.
                    Err(why) if why.contains("already") => {}
                    Err(_) => without += 1,
                }
            }
            (added, without)
        })
        .await
        .unwrap_or((0, 0));
        cx.update(|cx| {
            let text = match counted {
                (0, 0) => "Everything here is already in your downloads.".to_string(),
                (added, 0) => format!("Saving {added} episodes for offline"),
                (added, without) => {
                    format!("Saving {added} episodes. {without} have no source yet.")
                }
            };
            toast(text, cx)
        });
    })
    .detach();
}

/// Finds the best release and puts the item in the library's queue. The title, for the toast.
async fn queue(
    library: &LazyLibrary,
    streamer: &LazyStreamer,
    id: &str,
    release: Option<&str>,
) -> Result<String, String> {
    let plan = streamer.get().await?.plan(id).await?;
    let chosen = match release {
        Some(hash) => plan
            .ranked
            .iter()
            .chain(&plan.extra)
            .find(|release| release.info_hash.eq_ignore_ascii_case(hash)),
        // The player here decodes anything, so the releases a browser could not play are fair game.
        None => plan.ranked.first().or(plan.extra.first()),
    }
    .ok_or("No source was found for this yet.")?;
    queue_release(library, &plan, chosen, &[]).await
}

/// Puts `release` of the planned film or episode in the library's queue, with the subtitles to
/// save beside it. The title, for the toast.
pub async fn queue_release(
    library: &LazyLibrary,
    plan: &Plan,
    release: &Release,
    subtitles: &[SubtitleRef],
) -> Result<String, String> {
    let downloads = library.get().await?;
    let media = &plan.detail.media;
    let mut item = Item::new(
        Item::video_id(&plan.id, &release.info_hash),
        Kind::Video,
        plan.id.clone(),
        media.title.clone(),
    );
    item.release = Some(release.info_hash.clone());
    item.magnet = Some(release.magnet.clone());
    item.poster = media.poster.clone();
    item.season = plan.parsed.season;
    item.episode = plan.parsed.episode;
    item.subtitle = subtitle(
        &plan.detail,
        &plan.id,
        plan.parsed.season,
        plan.parsed.episode,
    );
    match downloads.add_with_subtitles(&item, subtitles) {
        Ok(true) => Ok(media.title.clone()),
        Ok(false) => Err("That is already in your downloads.".to_string()),
        Err(error) => Err(format!("{error:#}")),
    }
}

/// "Season 1, episode 5: The name" for an episode, the year for a film.
fn subtitle(
    detail: &akidev_core::model::Detail,
    id: &str,
    season: Option<u32>,
    episode: Option<u32>,
) -> String {
    let Some(episode) = episode else {
        return detail
            .media
            .year
            .map(|year| year.to_string())
            .unwrap_or_default();
    };
    let name = detail
        .episodes
        .iter()
        .find(|candidate| candidate.id == id)
        .and_then(|candidate| candidate.name.clone());
    let place = match season {
        Some(season) => format!("Season {season}, episode {episode}"),
        None => format!("Episode {episode}"),
    };
    match name {
        Some(name) => format!("{place}: {name}"),
        None => place,
    }
}

/// Queues one or more tracks for saving, and says how many were added.
pub fn save_tracks(tracks: Vec<Track>, cx: &mut App) {
    let backend = cx.global::<Backend>();
    let library = backend.library.clone();
    let ytdlp_dir = cx
        .global::<crate::music_player::MusicHub>()
        .ytdlp_dir
        .clone();
    cx.spawn(async move |cx| {
        let counted = runtime::run(async move {
            // The player that finds a track's audio is fetched the first time it is needed.
            akidev_core::music::install_ytdlp(&ytdlp_dir)
                .await
                .map_err(|error| format!("{error:#}"))?;
            let downloads = library.get().await?;
            let mut added = 0;
            for track in &tracks {
                let artists = track
                    .artists
                    .iter()
                    .map(|artist| artist.name.clone())
                    .collect::<Vec<_>>()
                    .join(", ");
                let mut item = Item::new(
                    track.id.clone(),
                    Kind::Music,
                    track.id.clone(),
                    track.title.clone(),
                );
                item.subtitle = artists;
                item.poster = track.cover.clone();
                if downloads.add(&item).map_err(|error| format!("{error:#}"))? {
                    added += 1;
                }
            }
            Ok::<usize, String>(added)
        })
        .await
        .unwrap_or_else(|| Err("Something went wrong.".into()));
        cx.update(|cx| match counted {
            Ok(0) => toast("That is already in your downloads.", cx),
            Ok(1) => toast("Saving the track for offline", cx),
            Ok(added) => toast(format!("Saving {added} tracks for offline"), cx),
            Err(why) => toast(why, cx),
        });
    })
    .detach();
}
