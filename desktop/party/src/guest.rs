//! A device that joins a party and follows its host.

use crate::peer::{Open, Peer, Sender, Wire};
use crate::protocol::{Member, Message, Readiness};
use crate::signal;
use crate::sync::{Clock, Target, mono_ms, target_of};
use anyhow::bail;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

/// How long to keep asking for the host's answer: the host answers as soon as it sees the offer,
/// unless its own connection is bad.
const ANSWER_WITHIN: Duration = Duration::from_secs(60);
/// The host says something (a state, an answer to a ping) at least this often, so a silence this
/// long means it is not there.
const HOST_SILENT_AFTER: Duration = Duration::from_secs(8);
/// This many silences in a row (each `HOST_SILENT_AFTER` long) and the host is counted as lost.
const LOST_AFTER_SILENCES: u32 = 4;
/// How often the clocks are compared. The first few are quick, so a good reading comes early.
const PING_EVERY: Duration = Duration::from_secs(5);
const QUICK_PINGS: u64 = 5;

#[derive(Debug, Clone)]
pub enum GuestEvent {
    /// What the party should be playing now. Sent when the host changes it and every few seconds.
    Target(Target),
    Members(Vec<Member>),
    /// Nothing has come from the host for a while (`true`), or it is heard again (`false`).
    HostSilent(bool),
    /// The host ended the party.
    Ended,
    /// The connection to the host is gone without a goodbye (a network change, the host's device
    /// asleep or its page reloaded). The party may still be there: join again with the same code.
    Lost,
}

pub struct Guest {
    sender: Sender,
    pinger: JoinHandle<()>,
    reader: JoinHandle<()>,
}

impl Guest {
    /// Joins the party `code` on `api`. `name` is how this device appears to the others.
    pub async fn join(
        api: &str,
        code: &str,
        name: &str,
    ) -> anyhow::Result<(Guest, mpsc::UnboundedReceiver<GuestEvent>)> {
        Guest::join_on(api, code, name, "0.0.0.0:0", session()).await
    }

    /// Like `join`, on the local address `bind` (see `Host::create_on`), as the device `id`: a host
    /// that sees an id again drops the connection it had from it, so two guests in one process
    /// need an id each.
    pub async fn join_on(
        api: &str,
        code: &str,
        name: &str,
        bind: &str,
        id: &str,
    ) -> anyhow::Result<(Guest, mpsc::UnboundedReceiver<GuestEvent>)> {
        let api = api.trim_end_matches('/');
        let code = code.trim().to_uppercase();
        let ice = signal::ice(api, &code).await?;
        let (peer, offer) = Peer::offering(ice, bind).await?;
        let answered = async {
            let id = signal::join(api, &code, &offer).await?;
            let answer = wait_for_answer(api, &code, &id).await?;
            peer.accept_answer(&answer).await
        }
        .await;
        if let Err(error) = answered {
            peer.close().await;
            return Err(error);
        }
        let Open { sender, wire } = peer.open().await?;
        sender
            .send(&Message::Hello {
                name: name.to_string(),
                id: Some(id.to_string()),
            })
            .await;

        let (events, receiver) = mpsc::unbounded_channel();
        let clock = std::sync::Arc::new(std::sync::Mutex::new(Clock::default()));
        let pinger = tokio::spawn({
            let sender = sender.clone();
            async move {
                let mut id = 0u64;
                loop {
                    sender
                        .send(&Message::Ping {
                            id,
                            sent: mono_ms(),
                        })
                        .await;
                    let wait = if id < QUICK_PINGS {
                        Duration::from_millis(400)
                    } else {
                        PING_EVERY
                    };
                    id += 1;
                    tokio::time::sleep(wait).await;
                }
            }
        });
        let reader = tokio::spawn(read(wire, events, clock));
        Ok((
            Guest {
                sender,
                pinger,
                reader,
            },
            receiver,
        ))
    }

    /// Asks the host to play, pause or seek. The host does it, and everyone, this device
    /// included, hears what it is now.
    pub async fn ask(&self, message: Message) {
        if matches!(
            message,
            Message::Play | Message::Pause | Message::Seek { .. }
        ) {
            self.sender.send(&message).await;
        }
    }

    /// Tells the host how this device is doing with the item whose id is `id`: ready for it,
    /// still loading it, or unable to play it.
    pub async fn status(&self, state: Readiness, id: Option<String>) {
        self.sender.send(&Message::Status { state, id }).await;
    }

    pub async fn leave(&self) {
        self.sender.send(&Message::Bye).await;
    }
}

impl Drop for Guest {
    fn drop(&mut self) {
        self.pinger.abort();
        self.reader.abort();
    }
}

async fn wait_for_answer(api: &str, code: &str, guest: &str) -> anyhow::Result<String> {
    let started = std::time::Instant::now();
    while started.elapsed() < ANSWER_WITHIN {
        if let Some(answer) = signal::take_answer(api, code, guest).await? {
            return Ok(answer);
        }
    }
    bail!("the host did not answer")
}

/// A random id for this run of the app, the same for every time it joins, so a host that sees the
/// same guest come back knows it is the same one and drops the old connection.
fn session() -> &'static str {
    static SESSION: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    SESSION.get_or_init(|| {
        use std::hash::{BuildHasher, Hasher};
        // A fresh random number for each run: the hasher's keys are chosen at random.
        let random = std::collections::hash_map::RandomState::new().build_hasher();
        format!("{:016x}", random.finish())
    })
}

async fn read(
    mut wire: mpsc::UnboundedReceiver<Wire>,
    events: mpsc::UnboundedSender<GuestEvent>,
    clock: std::sync::Arc<std::sync::Mutex<Clock>>,
) {
    // What the host said before the clocks were known is kept, and acted on at the first reading:
    // without one, how long ago it was said cannot be told.
    let mut waiting: Option<Message> = None;
    let mut silent = false;
    let mut quiet = 0u32;
    let ended = loop {
        let heard = match tokio::time::timeout(HOST_SILENT_AFTER, wire.recv()).await {
            Ok(heard) => heard,
            Err(_) => {
                // Silent for good: the connection is half open, so it is treated as lost and joined again.
                quiet += 1;
                if quiet >= LOST_AFTER_SILENCES {
                    break false;
                }
                if !silent {
                    silent = true;
                    let _ = events.send(GuestEvent::HostSilent(true));
                }
                continue;
            }
        };
        let Some(Wire::Message(message)) = heard else {
            break false;
        };
        quiet = 0;
        if silent {
            silent = false;
            let _ = events.send(GuestEvent::HostSilent(false));
        }
        let state = match message {
            state @ Message::State { .. } => Some(state),
            Message::Members { members } => {
                if events.send(GuestEvent::Members(members)).is_err() {
                    return;
                }
                continue;
            }
            Message::Pong { sent, at, .. } => {
                clock.lock().unwrap().sample(sent, at, mono_ms());
                waiting.take()
            }
            Message::Bye => break true,
            _ => continue,
        };
        let Some(Message::State {
            item,
            playing,
            position,
            at,
            choosing,
        }) = state
        else {
            continue;
        };
        let clock = clock.lock().unwrap();
        if !clock.is_known() {
            waiting = Some(Message::State {
                item,
                playing,
                position,
                at,
                choosing,
            });
            continue;
        }
        let target = target_of(item, playing, position, at, choosing, &clock, mono_ms());
        if events.send(GuestEvent::Target(target)).is_err() {
            return;
        }
    };
    let _ = events.send(if ended {
        GuestEvent::Ended
    } else {
        GuestEvent::Lost
    });
}
