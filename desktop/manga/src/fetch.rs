//! Making the requests: a polite rate, the cookies and user agent a challenge handed over, and a
//! stand-in that answers from recorded pages so a definition can be tested without its site.

use akidev_core::http::{client, read_capped};
use anyhow::{Context, bail};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::sync::Mutex;
use std::time::{Duration, Instant};

const MAX_BODY: usize = 8 * 1024 * 1024;

/// A browser user agent: most of these sites turn away anything that says it is a script. A clearance cookie
/// the person pastes in is tied to the user agent it was issued to, so `set_user_agent` can match it.
pub const DEFAULT_USER_AGENT: &str = "Mozilla/5.0 (iPhone; CPU iPhone OS 18_7 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Mobile/15E148";

#[derive(Debug, Clone)]
pub struct Req {
    pub method: String,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub form: Vec<(String, String)>,
    pub json: Option<Value>,
}

impl Req {
    /// The key a recorded answer is filed under.
    pub fn key(&self) -> String {
        format!("{} {}", self.method, self.url)
    }
}

/// The site asked for a check (Cloudflare, a captcha) instead of answering. The app has no browser to pass it
/// in, so it says the site is not supported from here and points to a server that can (see pipeline/MANGA_PLAN.md).
#[derive(Debug)]
pub struct Challenge {
    pub url: String,
    pub status: u16,
}

impl std::fmt::Display for Challenge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} wants a check before it answers (status {})", self.url, self.status)
    }
}

impl std::error::Error for Challenge {}

/// The site could not be reached at all: no network, a name that does not resolve, a refused connection.
#[derive(Debug)]
pub struct Unreachable {
    pub url: String,
    pub reason: String,
}

impl std::fmt::Display for Unreachable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "could not reach {}: {}", self.url, self.reason)
    }
}

impl std::error::Error for Unreachable {}

enum Transport {
    Live,
    /// Recorded answers by `Req::key`, and the requests that were made.
    Recorded { answers: HashMap<String, String>, seen: Mutex<Vec<String>> },
    /// Asks the site for real and keeps every answer, to make fixtures from.
    Recording { taken: Mutex<Vec<(String, String)>> },
}

pub struct Fetcher {
    transport: Transport,
    gap: Duration,
    next_slot: Mutex<Instant>,
    cookies: Mutex<BTreeMap<String, String>>,
    user_agent: Mutex<String>,
}

fn looks_like_challenge(status: u16, body: &str) -> bool {
    if !matches!(status, 403 | 429 | 503) {
        return false;
    }
    let body = body.to_lowercase();
    ["just a moment", "cf-chl", "challenge-platform", "captcha", "attention required", "ddos-guard"].iter().any(|mark| body.contains(mark))
}

impl Fetcher {
    pub fn live(rate_per_second: Option<f64>) -> Fetcher {
        Fetcher::new(Transport::Live, rate_per_second)
    }

    /// A live fetcher that remembers each answer (see `taken`).
    pub fn recording(rate_per_second: Option<f64>) -> Fetcher {
        Fetcher::new(Transport::Recording { taken: Mutex::new(Vec::new()) }, rate_per_second)
    }

    /// What a recording fetcher was answered, as (request key, body) in the order asked, once each.
    pub fn taken(&self) -> Vec<(String, String)> {
        match &self.transport {
            Transport::Recording { taken } => taken.lock().unwrap().clone(),
            _ => Vec::new(),
        }
    }

    pub fn recorded(answers: HashMap<String, String>) -> Fetcher {
        Fetcher::new(Transport::Recorded { answers, seen: Mutex::new(Vec::new()) }, None)
    }

    fn new(transport: Transport, rate_per_second: Option<f64>) -> Fetcher {
        let gap = rate_per_second.filter(|rate| *rate > 0.0).map_or(Duration::ZERO, |rate| Duration::from_secs_f64(1.0 / rate));
        Fetcher { transport, gap, next_slot: Mutex::new(Instant::now()), cookies: Mutex::default(), user_agent: Mutex::new(DEFAULT_USER_AGENT.to_string()) }
    }

    pub fn set_cookies(&self, cookies: impl IntoIterator<Item = (String, String)>) {
        self.cookies.lock().unwrap().extend(cookies);
    }

    pub fn set_user_agent(&self, user_agent: &str) {
        *self.user_agent.lock().unwrap() = user_agent.to_string();
    }

    /// The requests a recorded fetcher was asked for, in order.
    pub fn seen(&self) -> Vec<String> {
        match &self.transport {
            Transport::Recorded { seen, .. } => seen.lock().unwrap().clone(),
            _ => Vec::new(),
        }
    }

    /// Waits for this source's turn: one request per `gap`, however many are asked at once.
    async fn wait_turn(&self) {
        if self.gap.is_zero() {
            return;
        }
        let wait = {
            let mut next = self.next_slot.lock().unwrap();
            let now = Instant::now();
            let slot = (*next).max(now);
            *next = slot + self.gap;
            slot - now
        };
        if !wait.is_zero() {
            tokio::time::sleep(wait).await;
        }
    }

    pub async fn send(&self, req: &Req) -> anyhow::Result<String> {
        match &self.transport {
            Transport::Recorded { answers, seen } => {
                seen.lock().unwrap().push(req.key());
                match answers.get(&req.key()) {
                    Some(body) => Ok(body.clone()),
                    None => bail!("no recorded answer for {} (have: {})", req.key(), answers.keys().cloned().collect::<Vec<_>>().join(", ")),
                }
            }
            Transport::Live | Transport::Recording { .. } => {
                self.wait_turn().await;
                let method = reqwest::Method::from_bytes(req.method.as_bytes()).context("bad method")?;
                let mut builder = client().request(method, &req.url).header("user-agent", self.user_agent.lock().unwrap().clone());
                let cookie = self.cookies.lock().unwrap().iter().map(|(name, value)| format!("{name}={value}")).collect::<Vec<_>>().join("; ");
                if !cookie.is_empty() {
                    builder = builder.header("cookie", cookie);
                }
                for (name, value) in &req.headers {
                    builder = builder.header(name.as_str(), value.as_str());
                }
                if !req.form.is_empty() {
                    builder = builder.form(&req.form);
                }
                if let Some(json) = &req.json {
                    builder = builder.json(json);
                }
                // The address is already in the message, so the error's own copy of it is dropped.
                let response = builder.send().await.map_err(|error| Unreachable { url: req.url.clone(), reason: error.without_url().to_string() })?;
                let status = response.status();
                let body = String::from_utf8_lossy(&read_capped(response, MAX_BODY).await?).into_owned();
                if looks_like_challenge(status.as_u16(), &body) {
                    return Err(Challenge { url: req.url.clone(), status: status.as_u16() }.into());
                }
                if !status.is_success() {
                    bail!("{} answered {}", req.url, status);
                }
                if let Transport::Recording { taken } = &self.transport {
                    let mut taken = taken.lock().unwrap();
                    if !taken.iter().any(|(key, _)| *key == req.key()) {
                        taken.push((req.key(), body.clone()));
                    }
                }
                Ok(body)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_challenge_page_is_told_from_a_plain_error() {
        assert!(looks_like_challenge(403, "<title>Just a moment...</title>"));
        assert!(looks_like_challenge(503, "<div id=\"cf-chl-widget\">"));
        assert!(!looks_like_challenge(404, "Just a moment"));
        assert!(!looks_like_challenge(503, "Service unavailable"));
    }

    #[tokio::test]
    async fn a_recorded_fetcher_answers_by_method_and_address_and_says_what_it_lacks() {
        let fetcher = Fetcher::recorded(HashMap::from([("GET https://a.test/x".to_string(), "body".to_string())]));
        let req = |url: &str| Req { method: "GET".into(), url: url.into(), headers: vec![], form: vec![], json: None };
        assert_eq!(fetcher.send(&req("https://a.test/x")).await.unwrap(), "body");
        let error = fetcher.send(&req("https://a.test/y")).await.unwrap_err().to_string();
        assert!(error.contains("no recorded answer for GET https://a.test/y"), "{error}");
        assert_eq!(fetcher.seen().len(), 2);
    }

    #[tokio::test(start_paused = true)]
    async fn the_rate_spaces_requests() {
        let fetcher = Fetcher::live(Some(2.0));
        let start = tokio::time::Instant::now();
        fetcher.wait_turn().await;
        fetcher.wait_turn().await;
        fetcher.wait_turn().await;
        assert!(start.elapsed() >= Duration::from_millis(1000));
    }
}
