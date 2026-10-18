//! The meeting point of a watch party.
//!
//! A party's host is a device (the desktop app or a browser tab). Guests connect to it directly,
//! with WebRTC, and everything that is played travels on that connection. All this server does is
//! pass the first message each way, the offer and the answer, so two devices that cannot see each
//! other can find each other. It never sees what is playing, and a party is forgotten as soon as
//! its host stops asking for guests.
//!
//! A guest posts its offer; the host takes the offers waiting for it (a request that waits until
//! there is one), and posts an answer to each; the guest takes its answer the same way.

use crate::config::Settings;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
use tokio::sync::Notify;

/// Letters and digits that are not easily mistaken for each other.
const ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
const CODE_LENGTH: usize = 6;
/// Parties held at once, and guests in one. Every guest is a connection to the host's device.
pub const MAX_PARTIES: usize = 5000;
pub const MAX_GUESTS: usize = 20;
/// An offer or answer is a few kilobytes; this is far more than any real one.
pub const MAX_SDP: usize = 32 * 1024;
/// A party whose host has not asked for guests for this long is gone. A host asks continuously.
const HOST_SILENT_FOR: Duration = Duration::from_secs(120);
/// Parties one client may have open at once. A party is a code a person shares with a few friends.
const PER_CLIENT: usize = 5;
/// No party outlives this, whatever its host does.
const LONGEST: Duration = Duration::from_secs(12 * 60 * 60);
/// How long a request for the next offer or answer waits before saying there is none yet.
pub const WAIT: Duration = Duration::from_secs(25);
/// A guest's place is held for the host this long before the host has been given the offer.
const UNTAKEN_FOR: Duration = Duration::from_secs(60);
/// A connection takes a while to be made, so the host's list of its guests is not believed about a
/// place it took less than this long ago.
const SETTLING: Duration = Duration::from_secs(60);
/// How long a relay login is good for when it is asked for, and how long one is handed out again
/// before a new one is asked for. The first is much longer than the second, so a login given out
/// at the end of its time still has hours left.
const RELAY_LOGIN_FOR_SECONDS: u64 = 12 * 60 * 60;
const RELAY_LOGIN_REUSED_FOR: Duration = Duration::from_secs(30 * 60);
/// When Cloudflare does not answer, the last login is still handed out for this long: it is good for
/// 12 hours, so one made a few hours ago works.
const RELAY_LOGIN_STALE_FOR: Duration = Duration::from_secs(10 * 60 * 60);

pub struct Offer {
    pub guest: String,
    pub sdp: String,
}

/// A place a guest holds in a party.
struct Issued {
    at: Instant,
    /// The host has been given the guest's offer, so the host looks after the place from here.
    taken: bool,
    /// When the host was given the offer.
    taken_at: Option<Instant>,
}

struct Party {
    /// Who opened it (the client's address), so one client cannot hold thousands.
    owner: String,
    host_token: String,
    created: Instant,
    host_seen: Instant,
    offers: Vec<Offer>,
    answers: HashMap<String, String>,
    /// The guests who have a place, by id.
    issued: HashMap<String, Issued>,
    /// Woken when this party gets an offer or an answer or ends. Waiting requests then look again.
    changed: Arc<Notify>,
}

impl Party {
    /// A guest whose offer the host never received (the reply to the host's request was lost)
    /// would hold a place for good, so places the host has not taken in a minute are given back.
    /// The host says which guests it has a connection to. A place it holds the offer for but has no
    /// connection for any more (the call that gives it back was lost) is given back. Places the host
    /// took a moment ago are left: their connection is still being made.
    fn keep_only(&mut self, active: &[String], now: Instant) {
        self.issued.retain(|guest, place| {
            let settled = place
                .taken_at
                .is_some_and(|at| now.duration_since(at) > SETTLING);
            !place.taken || !settled || active.contains(guest)
        });
        let issued = &self.issued;
        self.answers.retain(|guest, _| issued.contains_key(guest));
    }

    fn forget_untaken(&mut self, now: Instant) {
        self.issued
            .retain(|_, place| place.taken || now.duration_since(place.at) < UNTAKEN_FOR);
        let issued = &self.issued;
        self.offers
            .retain(|offer| issued.contains_key(&offer.guest));
    }
}

#[derive(Default)]
pub struct Parties {
    held: Mutex<HashMap<String, Party>>,
}

pub struct Created {
    pub code: String,
    pub host_token: String,
}

/// Why the host could not (re)open its party.
#[derive(Debug, PartialEq)]
pub enum NotRegistered {
    /// The code or the token is not one this server makes.
    Malformed,
    /// The code belongs to another party.
    Taken,
    /// This client has too many open, or the server is holding as many as it will.
    TooMany,
}

/// Why the host's answer was refused.
#[derive(Debug, PartialEq)]
pub enum Refused {
    /// The party is over, or never was.
    NoParty,
    NotTheHost,
    /// No guest has that id (any more).
    NoGuest,
}

fn random_text(length: usize, alphabet: &[u8]) -> String {
    let mut bytes = vec![0u8; length];
    getrandom::fill(&mut bytes).expect("the system has randomness");
    bytes
        .iter()
        .map(|byte| alphabet[*byte as usize % alphabet.len()] as char)
        .collect()
}

fn owned_by(held: &HashMap<String, Party>, owner: &str) -> usize {
    held.values().filter(|party| party.owner == owner).count()
}

fn new_party(owner: &str, host_token: String, now: Instant) -> Party {
    Party {
        owner: owner.to_string(),
        host_token,
        created: now,
        host_seen: now,
        offers: Vec::new(),
        answers: HashMap::new(),
        issued: HashMap::new(),
        changed: Arc::new(Notify::new()),
    }
}

fn forget_the_gone(held: &mut HashMap<String, Party>, now: Instant) {
    held.retain(|_, party| {
        now.duration_since(party.host_seen) < HOST_SILENT_FOR
            && now.duration_since(party.created) < LONGEST
    });
}

impl Parties {
    /// Starts a party, or `None` when the server is holding as many as it will, or `owner` has
    /// several open already.
    pub fn create(&self, owner: &str) -> Option<Created> {
        let now = Instant::now();
        let mut held = self.held.lock().unwrap();
        forget_the_gone(&mut held, now);
        if held.len() >= MAX_PARTIES || owned_by(&held, owner) >= PER_CLIENT {
            return None;
        }
        let code = loop {
            let code = random_text(CODE_LENGTH, ALPHABET);
            if !held.contains_key(&code) {
                break code;
            }
        };
        let host_token = random_text(32, b"abcdefghijklmnopqrstuvwxyz0123456789");
        held.insert(code.clone(), new_party(owner, host_token.clone(), now));
        Some(Created { code, host_token })
    }

    /// Opens the party `code` for the host that holds `token`, or says it is open. A host whose party
    /// the server forgot (it was restarted, or the host's device slept for a while) opens it again
    /// under the same code, so the guests' code still works.
    pub fn register(&self, owner: &str, code: &str, token: &str) -> Result<(), NotRegistered> {
        let well_made = code.len() == CODE_LENGTH
            && code.bytes().all(|byte| ALPHABET.contains(&byte))
            && token.len() == 32
            && token.bytes().all(|byte| byte.is_ascii_alphanumeric());
        if !well_made {
            return Err(NotRegistered::Malformed);
        }
        let now = Instant::now();
        let mut held = self.held.lock().unwrap();
        forget_the_gone(&mut held, now);
        if let Some(party) = held.get_mut(code) {
            if party.host_token != token {
                return Err(NotRegistered::Taken);
            }
            party.host_seen = now;
            return Ok(());
        }
        if held.len() >= MAX_PARTIES || owned_by(&held, owner) >= PER_CLIENT {
            return Err(NotRegistered::TooMany);
        }
        held.insert(code.to_string(), new_party(owner, token.to_string(), now));
        Ok(())
    }

    pub fn exists(&self, code: &str) -> bool {
        let mut held = self.held.lock().unwrap();
        forget_the_gone(&mut held, Instant::now());
        held.contains_key(code)
    }

    /// A guest's offer. Returns the guest's id, or `None` when there is no such party or it is full.
    pub fn join(&self, code: &str, sdp: String) -> Option<String> {
        let now = Instant::now();
        let mut held = self.held.lock().unwrap();
        forget_the_gone(&mut held, now);
        let party = held.get_mut(code)?;
        party.forget_untaken(now);
        if party.issued.len() >= MAX_GUESTS {
            return None;
        }
        let guest = random_text(12, b"abcdefghijklmnopqrstuvwxyz0123456789");
        party.issued.insert(
            guest.clone(),
            Issued {
                at: now,
                taken: false,
                taken_at: None,
            },
        );
        party.offers.push(Offer {
            guest: guest.clone(),
            sdp,
        });
        party.changed.notify_waiters();
        Some(guest)
    }

    /// A guest left or failed to connect, so there is room for another. Says nothing twice.
    pub fn guest_gone(&self, code: &str, guest: &str) {
        if let Some(party) = self.held.lock().unwrap().get_mut(code) {
            party.issued.remove(guest);
            party.answers.remove(guest);
            party.offers.retain(|offer| offer.guest != guest);
        }
    }

    pub fn is_host(&self, code: &str, token: &str) -> bool {
        self.held
            .lock()
            .unwrap()
            .get(code)
            .is_some_and(|party| party.host_token == token)
    }

    /// The offers waiting for the host, waiting up to `WAIT` for the first. `None` means the
    /// token is wrong or the party is gone. `active` is the guests the host has a connection to, when
    /// it says (places it holds for anyone else are given back).
    pub async fn take_offers(
        &self,
        code: &str,
        token: &str,
        active: Option<&[String]>,
    ) -> Option<Vec<Offer>> {
        let deadline = tokio::time::Instant::now() + WAIT;
        loop {
            let changed = self.changes_of(code, token)?;
            let woken = changed.notified();
            tokio::pin!(woken);
            // Registered before looking, so an offer that arrives in between is not missed.
            woken.as_mut().enable();
            {
                let now = Instant::now();
                let mut held = self.held.lock().unwrap();
                forget_the_gone(&mut held, now);
                let party = held.get_mut(code).filter(|p| p.host_token == token)?;
                party.host_seen = now;
                party.forget_untaken(now);
                if let Some(active) = active {
                    party.keep_only(active, now);
                }
                if !party.offers.is_empty() {
                    for offer in &party.offers {
                        if let Some(place) = party.issued.get_mut(&offer.guest) {
                            place.taken = true;
                            place.taken_at = Some(now);
                        }
                    }
                    return Some(std::mem::take(&mut party.offers));
                }
            }
            if tokio::time::timeout_at(deadline, woken).await.is_err() {
                return Some(Vec::new());
            }
        }
    }

    /// What wakes the requests waiting on a party, if the token is its host's.
    fn changes_of(&self, code: &str, token: &str) -> Option<Arc<Notify>> {
        let held = self.held.lock().unwrap();
        let party = held.get(code).filter(|party| party.host_token == token)?;
        Some(party.changed.clone())
    }

    /// The host's answer to a guest's offer.
    pub fn answer(&self, code: &str, token: &str, guest: &str, sdp: String) -> Result<(), Refused> {
        let mut held = self.held.lock().unwrap();
        forget_the_gone(&mut held, Instant::now());
        let party = held.get_mut(code).ok_or(Refused::NoParty)?;
        if party.host_token != token {
            return Err(Refused::NotTheHost);
        }
        if !party.issued.contains_key(guest) {
            return Err(Refused::NoGuest);
        }
        party.answers.insert(guest.to_string(), sdp);
        party.changed.notify_waiters();
        Ok(())
    }

    /// The answer to a guest's offer, waiting up to `WAIT` for it. `None` means the party is gone,
    /// or no such guest asked.
    pub async fn take_answer(&self, code: &str, guest: &str) -> Option<Option<String>> {
        let deadline = tokio::time::Instant::now() + WAIT;
        loop {
            let changed = {
                let held = self.held.lock().unwrap();
                let party = held.get(code).filter(|p| p.issued.contains_key(guest))?;
                party.changed.clone()
            };
            let woken = changed.notified();
            tokio::pin!(woken);
            woken.as_mut().enable();
            {
                let mut held = self.held.lock().unwrap();
                forget_the_gone(&mut held, Instant::now());
                let party = held
                    .get_mut(code)
                    .filter(|p| p.issued.contains_key(guest))?;
                if let Some(answer) = party.answers.remove(guest) {
                    return Some(Some(answer));
                }
            }
            if tokio::time::timeout_at(deadline, woken).await.is_err() {
                return Some(None);
            }
        }
    }

    /// Ends a party. False when the token is wrong.
    pub fn end(&self, code: &str, token: &str) -> bool {
        let mut held = self.held.lock().unwrap();
        if held.get(code).is_none_or(|party| party.host_token != token) {
            return false;
        }
        if let Some(party) = held.remove(code) {
            party.changed.notify_waiters();
        }
        true
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct IceServer {
    pub urls: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credential: Option<String>,
}

/// What Cloudflare's TURN service answers when asked for a login.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RelayLogin {
    ice_servers: Vec<IceServer>,
}

/// The servers a device uses to find a path to the other one. With Cloudflare's TURN service set up
/// (`CF_TURN_KEY_ID` and `CF_TURN_TOKEN`) that is its STUN and TURN servers with a login that works
/// for a few hours, so a device that cannot reach the other directly is relayed. Without it, or when
/// Cloudflare does not answer, it is the public STUN servers and no relay.
pub async fn ice_servers(settings: &Settings) -> Vec<IceServer> {
    if !relay_is_set_up(settings) {
        return stun_only(settings);
    }
    let whose = format!("{}/{}", settings.turn_api, settings.turn_key_id);
    if let Some(servers) = reused_login(&whose) {
        return servers;
    }
    match fetch_login(settings).await {
        Ok(servers) => {
            login_cache()
                .lock()
                .unwrap()
                .insert(whose, (Instant::now(), servers.clone()));
            servers
        }
        Err(error) => match stale_login(&whose) {
            Some(servers) => {
                tracing::warn!(
                    "party: Cloudflare did not answer, using the last relay login: {error:#}"
                );
                servers
            }
            None => {
                tracing::warn!("party: no relay login from Cloudflare, using STUN only: {error:#}");
                stun_only(settings)
            }
        },
    }
}

/// Whether a relay login is set up at all: the server says so once when it starts.
pub fn relay_is_set_up(settings: &Settings) -> bool {
    !settings.turn_key_id.is_empty() && !settings.turn_token.is_empty()
}

fn stun_only(settings: &Settings) -> Vec<IceServer> {
    if settings.stun_urls.is_empty() {
        return Vec::new();
    }
    vec![IceServer {
        urls: settings.stun_urls.clone(),
        username: None,
        credential: None,
    }]
}

/// The last login for each key it was asked for with, and when it was made.
type LoginCache = Mutex<HashMap<String, (Instant, Vec<IceServer>)>>;

fn login_cache() -> &'static LoginCache {
    static CACHE: OnceLock<LoginCache> = OnceLock::new();
    CACHE.get_or_init(Default::default)
}

fn reused_login(whose: &str) -> Option<Vec<IceServer>> {
    let cache = login_cache().lock().unwrap();
    let (made, servers) = cache.get(whose)?;
    (made.elapsed() < RELAY_LOGIN_REUSED_FOR).then(|| servers.clone())
}

fn stale_login(whose: &str) -> Option<Vec<IceServer>> {
    let cache = login_cache().lock().unwrap();
    let (made, servers) = cache.get(whose)?;
    (made.elapsed() < RELAY_LOGIN_STALE_FOR).then(|| servers.clone())
}

async fn fetch_login(settings: &Settings) -> anyhow::Result<Vec<IceServer>> {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    let client = CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .expect("an HTTP client")
    });
    let login: RelayLogin = client
        .post(format!(
            "{}/v1/turn/keys/{}/credentials/generate-ice-servers",
            settings.turn_api, settings.turn_key_id
        ))
        .bearer_auth(&settings.turn_token)
        .json(&serde_json::json!({ "ttl": RELAY_LOGIN_FOR_SECONDS }))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    anyhow::ensure!(!login.ice_servers.is_empty(), "Cloudflare sent no servers");
    Ok(login.ice_servers)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn an_offer_reaches_the_host_and_the_answer_reaches_the_guest() {
        let parties = Parties::default();
        let party = parties.create("a").unwrap();
        let guest = parties.join(&party.code, "offer".into()).unwrap();

        let offers = parties
            .take_offers(&party.code, &party.host_token, None)
            .await
            .unwrap();
        assert_eq!(offers.len(), 1);
        assert_eq!(
            (offers[0].guest.as_str(), offers[0].sdp.as_str()),
            (guest.as_str(), "offer")
        );

        assert_eq!(
            parties.answer(&party.code, &party.host_token, &guest, "answer".into()),
            Ok(())
        );
        assert_eq!(
            parties.take_answer(&party.code, &guest).await,
            Some(Some("answer".to_string()))
        );
    }

    #[tokio::test]
    async fn the_host_waits_for_an_offer_that_has_not_come_yet() {
        let parties = std::sync::Arc::new(Parties::default());
        let party = parties.create("a").unwrap();
        let waiting = {
            let (parties, code, token) = (parties.clone(), party.code.clone(), party.host_token);
            tokio::spawn(async move { parties.take_offers(&code, &token, None).await })
        };
        tokio::time::sleep(Duration::from_millis(50)).await;
        parties.join(&party.code, "late".into()).unwrap();
        let offers = waiting.await.unwrap().unwrap();
        assert_eq!(offers[0].sdp, "late");
    }

    #[tokio::test]
    async fn another_partys_news_does_not_wake_a_waiting_host() {
        let parties = std::sync::Arc::new(Parties::default());
        let (ours, other) = (parties.create("a").unwrap(), parties.create("a").unwrap());
        let waiting = {
            let (parties, code, token) = (parties.clone(), ours.code.clone(), ours.host_token);
            tokio::spawn(async move { parties.take_offers(&code, &token, None).await })
        };
        tokio::time::sleep(Duration::from_millis(50)).await;
        parties.join(&other.code, "not ours".into()).unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(!waiting.is_finished());
        waiting.abort();
    }

    #[test]
    fn only_the_host_can_answer_or_end_a_party() {
        let parties = Parties::default();
        let party = parties.create("a").unwrap();
        let guest = parties.join(&party.code, "offer".into()).unwrap();
        assert_eq!(
            parties.answer(&party.code, "wrong", &guest, "x".into()),
            Err(Refused::NotTheHost)
        );
        assert!(!parties.end(&party.code, "wrong"));
        assert!(parties.exists(&party.code));
        assert!(parties.end(&party.code, &party.host_token));
        assert!(!parties.exists(&party.code));
        assert!(parties.join(&party.code, "offer".into()).is_none());
        assert_eq!(
            parties.answer(&party.code, &party.host_token, &guest, "x".into()),
            Err(Refused::NoParty)
        );
    }

    #[tokio::test]
    async fn an_answer_for_a_guest_that_never_asked_is_refused() {
        let parties = Parties::default();
        let party = parties.create("a").unwrap();
        assert_eq!(
            parties.answer(&party.code, &party.host_token, "nobody", "x".into()),
            Err(Refused::NoGuest)
        );
        // Nor does waiting for one hold a request.
        assert_eq!(parties.take_answer(&party.code, "nobody").await, None);
    }

    #[test]
    fn a_party_has_room_for_so_many_guests() {
        let parties = Parties::default();
        let party = parties.create("a").unwrap();
        let mut guests = Vec::new();
        for _ in 0..MAX_GUESTS {
            guests.push(parties.join(&party.code, "offer".into()).unwrap());
        }
        assert!(parties.join(&party.code, "offer".into()).is_none());
        parties.guest_gone(&party.code, &guests[0]);
        // Saying so twice does not give a second place back.
        parties.guest_gone(&party.code, &guests[0]);
        assert!(parties.join(&party.code, "offer".into()).is_some());
        assert!(parties.join(&party.code, "offer".into()).is_none());
    }

    #[test]
    fn a_place_the_host_never_heard_of_is_given_back() {
        let parties = Parties::default();
        let party = parties.create("a").unwrap();
        for _ in 0..MAX_GUESTS {
            parties.join(&party.code, "offer".into()).unwrap();
        }
        for place in parties
            .held
            .lock()
            .unwrap()
            .get_mut(&party.code)
            .unwrap()
            .issued
            .values_mut()
        {
            place.at = Instant::now() - UNTAKEN_FOR - Duration::from_secs(1);
        }
        assert!(parties.join(&party.code, "offer".into()).is_some());
    }

    #[test]
    fn a_party_whose_host_went_quiet_is_forgotten() {
        let parties = Parties::default();
        let party = parties.create("a").unwrap();
        parties
            .held
            .lock()
            .unwrap()
            .get_mut(&party.code)
            .unwrap()
            .host_seen = Instant::now() - HOST_SILENT_FOR - Duration::from_secs(1);
        assert!(!parties.exists(&party.code));
    }

    /// A stand-in for Cloudflare: answers a login for the key `good` with the token `secret`, and
    /// counts how often it was asked.
    async fn cloudflare() -> (String, Arc<std::sync::atomic::AtomicUsize>) {
        use axum::extract::{Path, State};
        use axum::http::{HeaderMap, StatusCode};
        use axum::routing::post;
        use std::sync::atomic::{AtomicUsize, Ordering};
        let asked = Arc::new(AtomicUsize::new(0));
        let app = axum::Router::new()
            .route(
                "/v1/turn/keys/{key}/credentials/generate-ice-servers",
                post(
                    |State(asked): State<Arc<AtomicUsize>>,
                     Path(key): Path<String>,
                     headers: HeaderMap,
                     body: String| async move {
                        asked.fetch_add(1, Ordering::SeqCst);
                        let ok = key == "good"
                            && headers.get("authorization").and_then(|v| v.to_str().ok())
                                == Some("Bearer secret");
                        if !ok || !body.contains("43200") {
                            return (StatusCode::UNAUTHORIZED, String::new());
                        }
                        (
                            StatusCode::CREATED,
                            r#"{"iceServers":[{"urls":["stun:stun.example:3478"]},{"urls":["turn:turn.example:3478?transport=udp","turns:turn.example:5349?transport=tcp"],"username":"user","credential":"pass"}]}"#.to_string(),
                        )
                    },
                ),
            )
            .with_state(asked.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move { axum::serve(listener, app).await });
        (address, asked)
    }

    fn with_relay(api: &str, key: &str, token: &str) -> Settings {
        let mut settings = Settings::from_env();
        settings.stun_urls = vec!["stun:fallback.example:3478".into()];
        (settings.turn_api, settings.turn_key_id, settings.turn_token) =
            (api.into(), key.into(), token.into());
        settings
    }

    #[tokio::test]
    async fn with_the_relay_set_up_a_device_is_given_its_servers_and_a_login_that_is_reused() {
        let (api, asked) = cloudflare().await;
        let settings = with_relay(&api, "good", "secret");
        let servers = ice_servers(&settings).await;
        assert_eq!(servers.len(), 2);
        assert_eq!(servers[1].username.as_deref(), Some("user"));
        assert_eq!(servers[1].credential.as_deref(), Some("pass"));
        assert!(servers[1].urls[0].starts_with("turn:"));
        // The next device is given the same login, without asking Cloudflare again.
        assert_eq!(ice_servers(&settings).await, servers);
        assert_eq!(asked.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn when_cloudflare_stops_answering_the_last_login_is_still_handed_out() {
        let (api, _) = cloudflare().await;
        let good = ice_servers(&with_relay(&api, "good", "secret")).await;
        // The login is old enough to be asked for again, and now Cloudflare refuses.
        login_cache()
            .lock()
            .unwrap()
            .get_mut(&format!("{api}/good"))
            .unwrap()
            .0 = Instant::now() - RELAY_LOGIN_REUSED_FOR - Duration::from_secs(1);
        let later = ice_servers(&with_relay(&api, "good", "wrong-token")).await;
        assert_eq!(later, good);
    }

    #[tokio::test]
    async fn when_cloudflare_refuses_or_is_not_set_up_it_is_stun_only() {
        let (api, _) = cloudflare().await;
        let refused = ice_servers(&with_relay(&api, "good", "wrong-token")).await;
        assert_eq!(refused.len(), 1);
        assert_eq!(refused[0].urls, vec!["stun:fallback.example:3478"]);
        assert!(refused[0].username.is_none());
        // Nowhere to ask.
        let nowhere = ice_servers(&with_relay("http://127.0.0.1:1", "other", "secret")).await;
        assert_eq!(nowhere.len(), 1);
        // Not set up at all.
        let unset = ice_servers(&with_relay(&api, "", "")).await;
        assert_eq!(unset.len(), 1);
    }

    #[test]
    fn a_host_reopens_a_party_the_server_forgot_under_the_same_code() {
        let parties = Parties::default();
        let code = "K7QF92";
        let token = "abcdefghijklmnopqrstuvwxyz012345";
        // The server has no such party yet (it was restarted): the host opens it again.
        assert_eq!(parties.register("a", code, token), Ok(()));
        assert!(parties.is_host(code, token));
        assert!(parties.join(code, "offer".into()).is_some());
        // Saying so again, with the same token, changes nothing.
        assert_eq!(parties.register("a", code, token), Ok(()));
        // Another token cannot take the code over.
        assert_eq!(
            parties.register("b", code, "zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz"),
            Err(NotRegistered::Taken)
        );
        // Nor can something that is not a code or a token.
        assert_eq!(
            parties.register("a", "abc", token),
            Err(NotRegistered::Malformed)
        );
        assert_eq!(
            parties.register("a", "K7QF9I", token),
            Err(NotRegistered::Malformed)
        );
        assert_eq!(
            parties.register("a", "K7QF93", "short"),
            Err(NotRegistered::Malformed)
        );
    }

    #[test]
    fn one_client_holds_only_so_many_parties() {
        let parties = Parties::default();
        for _ in 0..PER_CLIENT {
            assert!(parties.create("busy").is_some());
        }
        assert!(parties.create("busy").is_none());
        assert!(parties.create("someone else").is_some());
        assert_eq!(
            parties.register("busy", "K7QF92", "abcdefghijklmnopqrstuvwxyz012345"),
            Err(NotRegistered::TooMany)
        );
    }

    #[tokio::test]
    async fn places_the_host_has_no_connection_for_are_given_back_but_new_ones_are_kept() {
        let parties = Parties::default();
        let party = parties.create("a").unwrap();
        let kept = parties.join(&party.code, "offer".into()).unwrap();
        let ghost = parties.join(&party.code, "offer".into()).unwrap();
        // The host takes both offers.
        parties
            .take_offers(&party.code, &party.host_token, None)
            .await
            .unwrap();
        // It has a connection only to `kept`, but both were taken a moment ago, so both stay.
        let active = vec![kept.clone()];
        parties
            .take_offers(&party.code, &party.host_token, Some(&active))
            .await
            .unwrap();
        assert_eq!(party_issued(&parties, &party.code), 2);
        // Once the connections should have been made, the ghost goes.
        for place in parties
            .held
            .lock()
            .unwrap()
            .get_mut(&party.code)
            .unwrap()
            .issued
            .values_mut()
        {
            place.taken_at = Some(Instant::now() - SETTLING - Duration::from_secs(1));
        }
        parties
            .take_offers(&party.code, &party.host_token, Some(&active))
            .await
            .unwrap();
        assert_eq!(party_issued(&parties, &party.code), 1);
        assert_ne!(ghost, kept);
    }

    fn party_issued(parties: &Parties, code: &str) -> usize {
        parties.held.lock().unwrap().get(code).unwrap().issued.len()
    }
}
