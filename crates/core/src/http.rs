//! The one outbound HTTP client. Every request gets a timeout and a normal user agent,
//! because several of these hosts reject unknown clients.

use std::sync::OnceLock;
use std::time::Duration;

pub const USER_AGENT: &str = "akidev/2.0";

pub fn client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .user_agent(USER_AGENT)
            // A host that does not answer at all (a blocked address, a name that will not resolve)
            // is given up on in seconds, not after the whole timeout.
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(20))
            // A source (a plugin's most of all) must not be able to bounce a request to the viewer's
            // own machine or network, so a redirect to a private address is not followed.
            .redirect(reqwest::redirect::Policy::custom(|attempt| {
                let public = attempt
                    .url()
                    .host_str()
                    .is_some_and(crate::plugin::is_public_host);
                if attempt.previous().len() >= 10 {
                    attempt.error("too many redirects")
                } else if !public {
                    // An error, not a stop: a stopped redirect comes back as a 3xx that callers would read as
                    // an empty answer.
                    attempt.error("redirected to an address that is not public")
                } else {
                    attempt.follow()
                }
            }))
            .build()
            .expect("http client builds")
    })
}

/// Whether `url` is on the same host (and scheme and port) as `host`. A host is checked when a plugin is
/// read, but the path is added after it, and a path like `@127.0.0.1:8443/x` would move the request.
pub fn stays_on(host: &str, url: &str) -> bool {
    match (reqwest::Url::parse(host), reqwest::Url::parse(url)) {
        (Ok(host), Ok(url)) => {
            host.scheme() == url.scheme()
                && host.host_str() == url.host_str()
                && host.port_or_known_default() == url.port_or_known_default()
                && url.username().is_empty()
                && url.password().is_none()
        }
        _ => false,
    }
}

/// Reads a response body, failing once it grows past `max` bytes instead of holding all of
/// whatever a host chooses to send.
pub async fn read_capped(mut response: reqwest::Response, max: usize) -> anyhow::Result<Vec<u8>> {
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        anyhow::ensure!(body.len() + chunk.len() <= max, "the response is too large");
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn a_path_cannot_move_a_request_off_the_host_it_was_added_to() {
        let host = "https://torrents.example.com/api";
        for path in ["/search", "/a/b?x=1", ""] {
            assert!(stays_on(host, &format!("{host}{path}")), "{path}");
        }
        for path in [
            "@127.0.0.1:8443/x",
            ".evil.example/x",
            ":8443/x",
            "@evil.example/x",
        ] {
            let url = format!("https://torrents.example.com{path}");
            assert!(!stays_on("https://torrents.example.com", &url), "{path}");
        }
        assert!(!stays_on(host, "http://torrents.example.com/api"));
        assert!(!stays_on(host, "not a url"));
    }

    #[tokio::test]
    async fn a_redirect_to_a_private_address_is_not_followed() {
        use axum::{Router, response::Redirect, routing::get};
        let hits = Arc::new(AtomicUsize::new(0));
        let counted = hits.clone();
        let target = Router::new().route(
            "/secret",
            get(move || async move {
                counted.fetch_add(1, Ordering::SeqCst);
                "inside"
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let target_at = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, target).await.unwrap() });

        let bouncer = Router::new().route(
            "/go",
            get(move || async move { Redirect::temporary(&format!("http://{target_at}/secret")) }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let bouncer_at = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, bouncer).await.unwrap() });

        let error = client()
            .get(format!("http://{bouncer_at}/go"))
            .send()
            .await
            .unwrap_err();
        assert!(error.is_redirect(), "{error}");
        assert_eq!(hits.load(Ordering::SeqCst), 0);
    }
}
