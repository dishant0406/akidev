//! The messages devices send each other, as JSON text on one data channel. The web app has the
//! same list in `akidev-frontend/src/lib/party/protocol.ts`; change them together.

use serde::{Deserialize, Serialize};

/// Something to play. Every device finds its own stream from the id, so no address travels.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Item {
    /// A film or an episode: the media id, like `series-tt0903747-s1e5`, and the release the
    /// host is playing (its info hash), so every device plays the same source. A guest that
    /// cannot find that release plays the best one it can.
    Video {
        id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        release: Option<String>,
    },
    /// A music track: the video id on YouTube Music, with what a guest shows before it has loaded.
    Music {
        id: String,
        title: String,
        artist: String,
        #[serde(default)]
        cover: Option<String>,
    },
}

impl Item {
    /// The media id or the track id, whichever this is.
    pub fn id(&self) -> &str {
        match self {
            Item::Video { id, .. } | Item::Music { id, .. } => id,
        }
    }

    /// What `Status` names: the id, and for a video the release too, so a guest that was ready
    /// for the film is still loading it when the host switches to another release.
    pub fn key(&self) -> String {
        match self {
            Item::Video {
                id,
                release: Some(release),
            } => format!("{id}#{release}"),
            other => other.id().to_string(),
        }
    }
}

/// How a guest is doing with what the party is playing.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Readiness {
    /// Can play it now.
    Ready,
    /// Still finding a source or buffering: a film from a torrent can take a while.
    Loading,
    /// Could not find or play it.
    Trouble,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Member {
    pub name: String,
    pub host: bool,
    pub state: Readiness,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum Message {
    /// Guest to host, first: who is joining.
    Hello {
        name: String,
        /// A random id for this run of the app or page, the same each time it joins. A host that
        /// sees it again drops the old connection, so a guest that comes back is not there twice.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        id: Option<String>,
    },
    /// Host to guests: what is playing. `at` is the host's clock in milliseconds since 1970,
    /// when `position` (seconds) was read.
    State {
        item: Option<Item>,
        playing: bool,
        position: f64,
        at: u64,
        /// `source`, `episode` or `subtitles` while the host has that panel open; guests pause
        /// and say so on screen.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        choosing: Option<String>,
    },
    /// Host to guests: who is in the party.
    Members {
        members: Vec<Member>,
    },
    /// Guest to host: asks the host to do what a person at the host would. The host does it and
    /// tells everyone with a new `State`. What plays is the host's choice, so there is no request
    /// to open something.
    Play,
    Pause,
    Seek {
        position: f64,
    },
    /// Guest to host: how the guest is doing with the item whose id is `id` (a guest that has not
    /// yet said anything about the item the host is playing is counted as loading it).
    Status {
        state: Readiness,
        id: Option<String>,
    },
    /// Guest to host, and the answer: measures the way and the difference between the two clocks.
    Ping {
        id: u64,
        sent: u64,
    },
    Pong {
        id: u64,
        sent: u64,
        at: u64,
    },
    /// Either way: leaving. From the host it ends the party.
    Bye,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The web app reads and writes exactly this text.
    #[test]
    fn messages_look_the_way_the_web_app_expects() {
        let state = Message::State {
            item: Some(Item::Video {
                id: "movie-tt1".into(),
                release: None,
            }),
            playing: true,
            position: 12.5,
            at: 1_700_000_000_000,
            choosing: None,
        };
        assert_eq!(
            serde_json::to_string(&state).unwrap(),
            r#"{"t":"state","item":{"kind":"video","id":"movie-tt1"},"playing":true,"position":12.5,"at":1700000000000}"#
        );
        assert_eq!(
            serde_json::to_string(&Message::Pause).unwrap(),
            r#"{"t":"pause"}"#
        );
        assert_eq!(
            serde_json::to_string(&Message::Seek { position: 3.0 }).unwrap(),
            r#"{"t":"seek","position":3.0}"#
        );
        assert_eq!(
            serde_json::to_string(&Message::Status {
                state: Readiness::Loading,
                id: Some("movie-tt1".into())
            })
            .unwrap(),
            r#"{"t":"status","state":"loading","id":"movie-tt1"}"#
        );
        assert_eq!(
            serde_json::to_string(&Member {
                name: "Sam".into(),
                host: false,
                state: Readiness::Trouble
            })
            .unwrap(),
            r#"{"name":"Sam","host":false,"state":"trouble"}"#
        );
        let music: Message = serde_json::from_str(
            r#"{"t":"state","item":{"kind":"music","id":"abc","title":"A","artist":"B"},"playing":false,"position":0.0,"at":1}"#,
        )
        .unwrap();
        assert_eq!(
            music,
            Message::State {
                item: Some(Item::Music {
                    id: "abc".into(),
                    title: "A".into(),
                    artist: "B".into(),
                    cover: None
                }),
                playing: false,
                position: 0.0,
                at: 1,
                choosing: None
            }
        );
    }

    #[test]
    fn the_release_and_what_the_host_is_choosing_travel_with_the_state() {
        let state = Message::State {
            item: Some(Item::Video {
                id: "movie-tt1".into(),
                release: Some("abc123".into()),
            }),
            playing: false,
            position: 4.0,
            at: 7,
            choosing: Some("source".into()),
        };
        let text = serde_json::to_string(&state).unwrap();
        assert_eq!(
            text,
            r#"{"t":"state","item":{"kind":"video","id":"movie-tt1","release":"abc123"},"playing":false,"position":4.0,"at":7,"choosing":"source"}"#
        );
        assert_eq!(serde_json::from_str::<Message>(&text).unwrap(), state);
        // A state from a device that predates both fields still reads.
        let old: Message = serde_json::from_str(
            r#"{"t":"state","item":{"kind":"video","id":"movie-tt1"},"playing":true,"position":1.0,"at":1}"#,
        )
        .unwrap();
        let Message::State { item, choosing, .. } = old else {
            panic!("a state")
        };
        assert_eq!(choosing, None);
        assert_eq!(item.unwrap().key(), "movie-tt1");
    }

    #[test]
    fn a_hello_may_carry_the_guests_own_id_and_one_without_still_reads() {
        let hello = Message::Hello {
            name: "Sam".into(),
            id: Some("a1b2".into()),
        };
        assert_eq!(
            serde_json::to_string(&hello).unwrap(),
            r#"{"t":"hello","name":"Sam","id":"a1b2"}"#
        );
        // What a device from before the id said.
        let old: Message = serde_json::from_str(r#"{"t":"hello","name":"Sam"}"#).unwrap();
        assert_eq!(
            old,
            Message::Hello {
                name: "Sam".into(),
                id: None
            }
        );
    }

    #[test]
    fn a_status_names_the_release_too() {
        let a = Item::Video {
            id: "m".into(),
            release: Some("x".into()),
        };
        let b = Item::Video {
            id: "m".into(),
            release: Some("y".into()),
        };
        assert_ne!(a.key(), b.key());
        assert_eq!(a.id(), b.id());
    }
}
