//! The device that holds the party. Guests connect to it; it tells them what is playing.

use crate::peer::{Open, Peer, Sender, Wire};
use crate::protocol::{Item, Member, Message, Readiness};
use crate::signal::{self, is_gone};
use crate::sync::{Playback, now_ms};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;

/// What a guest asked for. The host does it, as if a person at the host had.
#[derive(Debug, Clone, PartialEq)]
pub enum HostEvent {
    Play,
    Pause,
    Seek(f64),
    /// Someone joined or left, or said they are having trouble.
    Members(Vec<Member>),
    /// The party server is not answering (or has lost the party and will not take it back), so no
    /// one new can join for now.
    Unreachable,
    /// The party server answers again.
    Reachable,
}

struct GuestSlot {
    sender: Sender,
    name: String,
    /// The guest's own id for this run of the app, when it has one.
    session: Option<String>,
    /// Set to drop this guest (it came back on a new connection).
    kick: watch::Sender<bool>,
    /// What the guest last said about how it is doing, and about which item.
    state: Readiness,
    about: Option<String>,
}

struct Shared {
    api: String,
    code: String,
    token: String,
    name: String,
    guests: Mutex<HashMap<String, GuestSlot>>,
    /// What was last published, so a guest who joins late is told at once.
    last: Mutex<Option<Message>>,
    events: mpsc::UnboundedSender<HostEvent>,
    bind: String,
    /// Set when the party ends, so what looks after each guest lets go.
    ending: watch::Sender<bool>,
}

pub struct Host {
    pub code: String,
    shared: Arc<Shared>,
    offers: JoinHandle<()>,
}

/// A guest says something (a ping) every few seconds; one that has been silent this long is gone,
/// whether or not the connection has noticed.
const GUEST_SILENT_AFTER: Duration = Duration::from_secs(15);
/// A guest that has not said who it is within this long is not one of ours.
const HELLO_WITHIN: Duration = Duration::from_secs(10);
/// What one guest may send: a burst, and then this many a second. Nobody pressing buttons gets near.
const BURST: f64 = 40.0;
const PER_SECOND: f64 = 20.0;
/// A guest that goes on sending after being told to stop is dropped once it is this far over.
const FLOOD: u32 = 200;
/// What the host said about playing is not believed for a joiner after this long.
const STALE_AFTER: Duration = Duration::from_secs(10);
/// How long the host waits between asking for guests when the server does not answer, at most.
const LONGEST_WAIT: Duration = Duration::from_secs(10);
/// Not reaching the server for this long is worth telling the host about.
const UNREACHABLE_AFTER: u32 = 3;

impl Host {
    /// Opens a party on `api`. `name` is how the host appears to the guests.
    pub async fn create(
        api: &str,
        name: &str,
    ) -> anyhow::Result<(Host, mpsc::UnboundedReceiver<HostEvent>)> {
        Host::create_on(api, name, "0.0.0.0:0").await
    }

    /// Like `create`, on the local address `bind` (an address and port, `0` for any) instead of
    /// every interface. The tests use it to connect two devices on one machine.
    pub async fn create_on(
        api: &str,
        name: &str,
        bind: &str,
    ) -> anyhow::Result<(Host, mpsc::UnboundedReceiver<HostEvent>)> {
        let created = signal::create(api).await?;
        let (events, receiver) = mpsc::unbounded_channel();
        let shared = Arc::new(Shared {
            api: api.trim_end_matches('/').to_string(),
            code: created.code.clone(),
            token: created.token,
            name: name.to_string(),
            guests: Mutex::default(),
            last: Mutex::default(),
            events,
            bind: bind.to_string(),
            ending: watch::channel(false).0,
        });
        let offers = tokio::spawn(take_offers(shared.clone()));
        Ok((
            Host {
                code: created.code,
                shared,
                offers,
            },
            receiver,
        ))
    }

    /// What is playing now. Called when it changes and every couple of seconds while it plays.
    pub async fn publish(&self, playback: Playback) {
        let message = Message::State {
            item: playback.item,
            playing: playback.playing,
            position: playback.position,
            at: now_ms(),
            choosing: playback.choosing,
        };
        let changed = {
            let mut last = self.shared.last.lock().unwrap();
            let before = last.as_ref().and_then(item_key);
            let changed = before != item_key(&message);
            *last = Some(message.clone());
            changed
        };
        send_all(&self.shared, &message).await;
        // A guest is loading whatever the host starts until it says it is ready for it, so a new
        // item changes who is.
        if changed {
            share_members(&self.shared).await;
        }
    }

    /// Ends the party: guests are told, and the party is closed on the server.
    pub async fn end(&self) {
        self.offers.abort();
        send_all(&self.shared, &Message::Bye).await;
        let _ = self.shared.ending.send(true);
        signal::end(&self.shared.api, &self.shared.code, &self.shared.token).await;
    }
}

impl Drop for Host {
    fn drop(&mut self) {
        self.offers.abort();
    }
}

fn senders(shared: &Shared) -> Vec<Sender> {
    shared
        .guests
        .lock()
        .unwrap()
        .values()
        .map(|slot| slot.sender.clone())
        .collect()
}

async fn send_all(shared: &Shared, message: &Message) {
    for sender in senders(shared) {
        sender.send(message).await;
    }
}

/// What a `State` message is playing, as a guest names it in `Status`: the id, and the release.
fn item_key(message: &Message) -> Option<String> {
    match message {
        Message::State { item, .. } => item.as_ref().map(Item::key),
        _ => None,
    }
}

/// The plain id of what a `State` message is playing.
fn item_id(message: &Message) -> Option<String> {
    match message {
        Message::State { item, .. } => item.as_ref().map(|item| item.id().to_string()),
        _ => None,
    }
}

fn members(shared: &Shared) -> Vec<Member> {
    let (playing, plain) = {
        let last = shared.last.lock().unwrap();
        (
            last.as_ref().and_then(item_key),
            last.as_ref().and_then(item_id),
        )
    };
    let guests = shared.guests.lock().unwrap();
    let mut list = vec![Member {
        name: shared.name.clone(),
        host: true,
        state: Readiness::Ready,
    }];
    list.extend(guests.values().map(|slot| Member {
        name: slot.name.clone(),
        host: false,
        state: guest_state(
            slot.state,
            slot.about.as_deref(),
            playing.as_deref(),
            plain.as_deref(),
        ),
    }));
    list
}

/// How a guest is doing with what the host is playing now. What it said about an earlier item says
/// nothing about this one: it is still getting to it. `playing` is the item's key (id and release)
/// and `plain` its id: a device from before releases were shared names the id alone, which counts.
fn guest_state(
    said: Readiness,
    about: Option<&str>,
    playing: Option<&str>,
    plain: Option<&str>,
) -> Readiness {
    match playing {
        Some(key) if about != Some(key) && about != plain => Readiness::Loading,
        _ => said,
    }
}

/// Tells the guests and the host's own screen who is in the party.
async fn share_members(shared: &Shared) {
    let members = members(shared);
    send_all(
        shared,
        &Message::Members {
            members: members.clone(),
        },
    )
    .await;
    let _ = shared.events.send(HostEvent::Members(members));
}

/// Asks the server for guests, one after another, and connects each as it comes. It goes on for as
/// long as the party does: a server that is restarting or a network that is down is waited out, and
/// a party the server forgot is opened again under its code.
async fn take_offers(shared: Arc<Shared>) {
    let mut failures = 0u32;
    let mut told = false;
    loop {
        let active: Vec<String> = shared.guests.lock().unwrap().keys().cloned().collect();
        let asked = signal::offers(&shared.api, &shared.code, &shared.token, &active).await;
        match asked {
            Ok(offers) => {
                failures = 0;
                if std::mem::take(&mut told) {
                    let _ = shared.events.send(HostEvent::Reachable);
                }
                for offer in offers {
                    tokio::spawn(accept(shared.clone(), offer));
                }
                continue;
            }
            Err(error) if is_gone(&error) => {
                match signal::register(&shared.api, &shared.code, &shared.token).await {
                    Ok(true) => continue,
                    // The code is someone else's now: no one new can join this party.
                    Ok(false) => {
                        let _ = shared.events.send(HostEvent::Unreachable);
                        break;
                    }
                    Err(error) => {
                        tracing::debug!("[party] opening the party again failed: {error:#}")
                    }
                }
            }
            Err(error) => tracing::debug!("[party] asking for guests failed: {error:#}"),
        }
        failures += 1;
        if failures >= UNREACHABLE_AFTER && !told {
            told = true;
            let _ = shared.events.send(HostEvent::Unreachable);
        }
        let wait = (Duration::from_secs(2) * 2u32.pow(failures.min(4) - 1)).min(LONGEST_WAIT);
        tokio::time::sleep(wait).await;
    }
}

/// Answers one guest's offer and looks after that guest until they are gone.
async fn accept(shared: Arc<Shared>, offer: signal::Offer) {
    let guest = offer.guest;
    let result = connect(&shared, &guest, &offer.sdp).await;
    match result {
        Ok(open) => look_after(&shared, &guest, open).await,
        Err(error) => tracing::debug!("[party] a guest could not connect: {error:#}"),
    }
    // Whatever happened, the place is free again.
    let left = shared.guests.lock().unwrap().remove(&guest).is_some();
    signal::guest_gone(&shared.api, &shared.code, &shared.token, &guest).await;
    if left {
        share_members(&shared).await;
    }
}

async fn connect(shared: &Shared, guest: &str, offer: &str) -> anyhow::Result<Open> {
    // Asked for each time: the relay's login lasts a few hours, and a party can last longer.
    let ice = signal::ice(&shared.api, &shared.code).await?;
    let (peer, answer) = Peer::answering(ice, &shared.bind, offer).await?;
    if let Err(error) =
        signal::answer(&shared.api, &shared.code, &shared.token, guest, &answer).await
    {
        peer.close().await;
        return Err(error);
    }
    peer.open().await
}

/// What one guest may send. A guest that sends faster than any person presses buttons has the
/// excess dropped, and one that keeps on is dropped itself.
struct Allowance {
    tokens: f64,
    at: Instant,
    over: u32,
}

enum Verdict {
    Fine,
    Skip,
    Flood,
}

impl Allowance {
    fn new() -> Allowance {
        Allowance {
            tokens: BURST,
            at: Instant::now(),
            over: 0,
        }
    }

    fn take(&mut self) -> Verdict {
        let now = Instant::now();
        self.tokens =
            (self.tokens + now.duration_since(self.at).as_secs_f64() * PER_SECOND).min(BURST);
        self.at = now;
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            self.over = 0;
            return Verdict::Fine;
        }
        self.over += 1;
        if self.over > FLOOD {
            Verdict::Flood
        } else {
            Verdict::Skip
        }
    }
}

async fn look_after(shared: &Shared, guest: &str, mut open: Open) {
    let mut ending = shared.ending.subscribe();
    if *ending.borrow() {
        return;
    }
    let (kick, mut kicked) = watch::channel(false);
    shared.guests.lock().unwrap().insert(
        guest.to_string(),
        GuestSlot {
            sender: open.sender.clone(),
            name: "Guest".to_string(),
            session: None,
            kick,
            state: Readiness::Ready,
            about: None,
        },
    );
    // A guest who joins late is told what is playing, as it is now, before anything else.
    let last = shared.last.lock().unwrap().clone();
    if let Some(state) = last {
        open.sender.send(&as_of_now(state)).await;
    }
    share_members(shared).await;

    let mut allowance = Allowance::new();
    let mut introduced = false;
    let mut heard = Instant::now();
    loop {
        let patience = if introduced {
            GUEST_SILENT_AFTER
        } else {
            HELLO_WITHIN
        };
        let wire = tokio::select! {
            wire = open.wire.recv() => wire,
            _ = ending.wait_for(|ended| *ended) => None,
            _ = kicked.wait_for(|kicked| *kicked) => None,
            () = tokio::time::sleep_until((heard + patience).into()) => None,
        };
        let Some(Wire::Message(message)) = wire else {
            break;
        };
        heard = Instant::now();
        match allowance.take() {
            Verdict::Fine => {}
            Verdict::Skip => continue,
            Verdict::Flood => break,
        }
        match message {
            Message::Hello { name, id } => {
                // An id is a short random string; anything longer is not one.
                let id = id.filter(|id| id.len() <= 64);
                introduced = true;
                {
                    let mut guests = shared.guests.lock().unwrap();
                    // The same guest on a new connection: the old one is let go.
                    if let Some(id) = &id {
                        for (other, slot) in guests.iter() {
                            if other != guest && slot.session.as_ref() == Some(id) {
                                let _ = slot.kick.send(true);
                            }
                        }
                    }
                    if let Some(slot) = guests.get_mut(guest) {
                        slot.name = clean_name(&name);
                        slot.session = id;
                    }
                }
                share_members(shared).await;
            }
            Message::Status { state, id } => {
                if let Some(slot) = shared.guests.lock().unwrap().get_mut(guest) {
                    slot.state = state;
                    slot.about = id;
                }
                share_members(shared).await;
            }
            Message::Ping { id, sent } => {
                open.sender
                    .send(&Message::Pong {
                        id,
                        sent,
                        at: now_ms(),
                    })
                    .await;
            }
            Message::Play => {
                let _ = shared.events.send(HostEvent::Play);
            }
            Message::Pause => {
                let _ = shared.events.send(HostEvent::Pause);
            }
            Message::Seek { position } if position.is_finite() => {
                let _ = shared.events.send(HostEvent::Seek(position.max(0.0)));
            }
            Message::Bye => break,
            _ => {}
        }
    }
}

/// A published state, moved on to now: a playing one has been playing since it was said.
fn as_of_now(state: Message) -> Message {
    match state {
        Message::State {
            item,
            playing: true,
            position,
            at,
            choosing,
        } => {
            let now = now_ms();
            let late = now.saturating_sub(at);
            // A host that said "playing" and has been silent for this long is not playing any more
            // (it froze, or slept): a joiner is not sent into the film on its word.
            if late > STALE_AFTER.as_millis() as u64 {
                return Message::State {
                    item,
                    playing: false,
                    position,
                    at: now,
                    choosing,
                };
            }
            Message::State {
                item,
                playing: true,
                position: position + late as f64 / 1000.0,
                at: now,
                choosing,
            }
        }
        other => other,
    }
}

/// A name from a guest is shown to everyone, so it is kept short and plain.
fn clean_name(name: &str) -> String {
    let name: String = name.chars().filter(|c| !c.is_control()).take(24).collect();
    let name = name.trim();
    if name.is_empty() {
        "Guest".to_string()
    } else {
        name.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_guest_is_loading_what_it_has_not_yet_said_it_is_ready_for() {
        use Readiness::{Loading, Ready, Trouble};
        // Nothing playing: whatever the guest said stands.
        assert_eq!(guest_state(Ready, None, None, None), Ready);
        // Something is playing and the guest has said nothing, or said it of the one before.
        assert_eq!(guest_state(Ready, None, Some("b"), Some("b")), Loading);
        assert_eq!(guest_state(Ready, Some("a"), Some("b"), Some("b")), Loading);
        // Said of this one.
        assert_eq!(guest_state(Ready, Some("b"), Some("b"), Some("b")), Ready);
        assert_eq!(
            guest_state(Loading, Some("b"), Some("b"), Some("b")),
            Loading
        );
        assert_eq!(
            guest_state(Trouble, Some("b"), Some("b"), Some("b")),
            Trouble
        );
        // The same film on another release is a new thing to get to.
        assert_eq!(
            guest_state(Ready, Some("m#x"), Some("m#y"), Some("m")),
            Loading
        );
        assert_eq!(
            guest_state(Ready, Some("m#y"), Some("m#y"), Some("m")),
            Ready
        );
        // A device from before releases were shared says the plain id, which is enough.
        assert_eq!(guest_state(Ready, Some("m"), Some("m#y"), Some("m")), Ready);
    }

    #[test]
    fn a_state_told_to_a_late_guest_is_moved_on_to_now() {
        let said = Message::State {
            item: None,
            playing: true,
            position: 10.0,
            at: now_ms() - 5_000,
            choosing: None,
        };
        let Message::State { position, .. } = as_of_now(said) else {
            panic!("still a state")
        };
        assert!((position - 15.0).abs() < 0.5);
        let paused = Message::State {
            item: None,
            playing: false,
            position: 10.0,
            at: 1,
            choosing: None,
        };
        assert_eq!(as_of_now(paused.clone()), paused);
    }

    #[test]
    fn a_guests_name_is_kept_short_and_plain() {
        assert_eq!(clean_name("  Sam \n"), "Sam");
        assert_eq!(clean_name(""), "Guest");
        assert_eq!(clean_name(&"x".repeat(100)).chars().count(), 24);
    }

    #[test]
    fn a_guest_that_sends_faster_than_a_person_has_the_excess_dropped_and_then_is_dropped() {
        let mut allowance = Allowance::new();
        // A burst is allowed.
        for _ in 0..BURST as u32 {
            assert!(matches!(allowance.take(), Verdict::Fine));
        }
        // After that, with no time passing, it is skipped, and in the end the guest goes.
        let mut skipped = 0;
        let flooded = loop {
            match allowance.take() {
                Verdict::Skip => skipped += 1,
                Verdict::Flood => break true,
                Verdict::Fine => {}
            }
            if skipped > FLOOD * 2 {
                break false;
            }
        };
        assert!(flooded);
    }

    #[test]
    fn a_playing_state_the_host_stopped_repeating_is_not_believed_for_a_joiner() {
        let stale = Message::State {
            item: None,
            playing: true,
            position: 10.0,
            at: now_ms() - 60_000,
            choosing: None,
        };
        let Message::State {
            playing, position, ..
        } = as_of_now(stale)
        else {
            panic!("still a state")
        };
        assert!(!playing);
        assert_eq!(position, 10.0);
    }
}
