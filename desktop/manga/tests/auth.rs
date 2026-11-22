//! Signing in to a source, against a small site on this machine: the login, the session on every request,
//! the refresh after a 401, the logout, and where the session is never sent.

use akidev_manga::Source;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// What the site has seen and what it will accept.
#[derive(Default)]
struct Site {
    /// The access token the site accepts right now.
    access: Mutex<String>,
    refreshes: Mutex<u32>,
    logouts: Mutex<u32>,
    /// The `authorization` header of each request to `/open`, `None` when there was none.
    open_seen: Mutex<Vec<Option<String>>>,
}

fn reply(status: &str, body: &str) -> String {
    format!(
        "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.len()
    )
}

/// A site that signs people in with the password `secret`, hands out the token `a1`, renews it as `a2`, and
/// answers `/list` only to the token it currently accepts.
async fn serve(site: Arc<Site>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move {
        loop {
            let (mut socket, _) = listener.accept().await.unwrap();
            let site = site.clone();
            tokio::spawn(async move {
                let mut data = Vec::new();
                let mut chunk = [0u8; 4096];
                // Read the head, then as much body as it promises.
                let (head, body) = loop {
                    let read = socket.read(&mut chunk).await.unwrap_or(0);
                    data.extend_from_slice(&chunk[..read]);
                    let text = String::from_utf8_lossy(&data).to_string();
                    if let Some((head, body)) = text.split_once("\r\n\r\n") {
                        let wanted = head
                            .lines()
                            .find_map(|line| {
                                line.to_lowercase()
                                    .strip_prefix("content-length:")
                                    .map(|n| n.trim().parse::<usize>().unwrap_or(0))
                            })
                            .unwrap_or(0);
                        if body.len() >= wanted || read == 0 {
                            break (head.to_string(), body.to_string());
                        }
                    } else if read == 0 {
                        return;
                    }
                };
                let request_line = head.lines().next().unwrap_or("").to_string();
                let authorization = head.lines().find_map(|line| {
                    line.to_lowercase()
                        .starts_with("authorization:")
                        .then(|| line.split_once(':').unwrap().1.trim().to_string())
                });
                let json: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
                let answer = match request_line.split(' ').take(2).collect::<Vec<_>>()[..] {
                    ["POST", "/login"] if json["password"] == "secret" => {
                        *site.access.lock().unwrap() = "a1".into();
                        reply(
                            "200 OK",
                            r#"{"data":{"access_token":"a1","refresh_token":"r1","user":{"name":"Dishant"}}}"#,
                        )
                    }
                    ["POST", "/login"] => reply("401 Unauthorized", r#"{"error":"nope"}"#),
                    ["POST", "/refresh"] if json["refresh_token"] == "r1" => {
                        *site.access.lock().unwrap() = "a2".into();
                        *site.refreshes.lock().unwrap() += 1;
                        reply("200 OK", r#"{"data":{"access_token":"a2"}}"#)
                    }
                    ["POST", "/refresh"] => reply("401 Unauthorized", r#"{"error":"expired"}"#),
                    ["POST", "/logout"] => {
                        *site.logouts.lock().unwrap() += 1;
                        reply("200 OK", "{}")
                    }
                    ["GET", "/list"] => {
                        let wanted = format!("Bearer {}", site.access.lock().unwrap());
                        if authorization.as_deref() == Some(wanted.as_str()) {
                            reply("200 OK", r#"{"items":[{"id":"x","title":"Paid title"}]}"#)
                        } else {
                            reply("401 Unauthorized", "{}")
                        }
                    }
                    ["GET", "/open"] => {
                        site.open_seen.lock().unwrap().push(authorization);
                        reply("200 OK", r#"{"items":[{"id":"x","title":"Open title"}]}"#)
                    }
                    _ => reply("404 Not Found", "{}"),
                };
                let _ = socket.write_all(answer.as_bytes()).await;
            });
        }
    });
    base
}

fn definition(base: &str, extra: Value) -> Source {
    let mut def = json!({
        "schema": 1, "id": "paid", "name": "Paid", "version": 1, "base_url": base,
        "auth": {
            "fields": [
                { "id": "email", "name": "Email" },
                { "id": "password", "name": "Password", "secret": true }
            ],
            "login": {
                "request": { "method": "POST", "url": "{base}/login", "json": { "email": "{email}", "password": "{password}" } },
                "format": "json",
                "vars": { "access": "data.access_token", "refresh": "data.refresh_token", "user": "data.user.name" }
            },
            "refresh": {
                "request": { "method": "POST", "url": "{base}/refresh", "json": { "refresh_token": "{refresh}" } },
                "format": "json",
                "vars": { "access": "data.access_token" }
            },
            "logout": { "method": "POST", "url": "{base}/logout" },
            "headers": { "authorization": "Bearer {access}" }
        },
        "popular": { "request": { "url": "{base}/list" }, "format": "json", "rows": "items", "card": { "id": "id", "title": "title" } },
        "detail": { "request": { "url": "{base}/d/{id}" }, "format": "json", "title": "t" },
        "chapters": { "request": { "url": "{base}/c/{id}" }, "format": "json", "rows": "rows", "chapter": { "id": "id" } },
        "pages": { "request": { "url": "{base}/p/{id}" }, "format": "json", "images": "images" }
    });
    for (key, value) in extra.as_object().unwrap() {
        def[key] = value.clone();
    }
    Source::live(serde_json::from_value(def).unwrap()).unwrap()
}

fn account(password: &str) -> BTreeMap<String, String> {
    BTreeMap::from([
        ("email".to_string(), "me@example.com".to_string()),
        ("password".to_string(), password.to_string()),
    ])
}

async fn titles(source: &Source) -> anyhow::Result<Vec<String>> {
    Ok(source
        .popular(1)
        .await?
        .items
        .into_iter()
        .map(|card| card.title)
        .collect())
}

#[tokio::test]
async fn a_signed_in_person_is_let_in_and_a_stranger_is_not() {
    let site = Arc::new(Site::default());
    let source = definition(&serve(site.clone()).await, json!({}));
    assert!(source.info().has_sign_in && !source.info().signed_in);

    let refused = titles(&source).await.unwrap_err().to_string();
    assert!(refused.contains("401"), "{refused}");

    let user = source.sign_in(&account("secret")).await.unwrap();
    assert_eq!(user.as_deref(), Some("Dishant"));
    assert!(source.info().signed_in);
    assert_eq!(titles(&source).await.unwrap(), ["Paid title"]);
}

#[tokio::test]
async fn a_wrong_password_says_so_and_signs_nobody_in() {
    let source = definition(&serve(Arc::new(Site::default())).await, json!({}));
    let error = source
        .sign_in(&account("wrong"))
        .await
        .unwrap_err()
        .to_string();
    assert!(error.contains("did not accept that sign-in"), "{error}");
    assert!(
        !error.contains("wrong"),
        "the password must not be in an error: {error}"
    );
    assert!(!source.info().signed_in);
}

#[tokio::test]
async fn a_missing_field_is_named_before_anything_is_sent() {
    let source = definition(&serve(Arc::new(Site::default())).await, json!({}));
    let mut typed = account("secret");
    typed.remove("email");
    let error = source.sign_in(&typed).await.unwrap_err().to_string();
    assert_eq!(error, "Email is needed to sign in");
}

#[tokio::test]
async fn a_refused_request_refreshes_the_session_once_and_goes_through() {
    let site = Arc::new(Site::default());
    let source = definition(&serve(site.clone()).await, json!({}));
    source.sign_in(&account("secret")).await.unwrap();

    // The site stops accepting the token the app holds, as when it expires.
    *site.access.lock().unwrap() = "expired".into();
    // ...and will accept the one a refresh gives.
    assert_eq!(titles(&source).await.unwrap(), ["Paid title"]);
    assert_eq!(*site.refreshes.lock().unwrap(), 1);
    assert_eq!(source.session()["access"], "a2");
    // The refresh kept what it did not replace.
    assert_eq!(source.session()["refresh"], "r1");

    // The next request uses the new token and needs no refresh.
    assert_eq!(titles(&source).await.unwrap(), ["Paid title"]);
    assert_eq!(*site.refreshes.lock().unwrap(), 1);
}

#[tokio::test]
async fn two_refused_requests_at_once_refresh_once() {
    let site = Arc::new(Site::default());
    let source = definition(&serve(site.clone()).await, json!({}));
    source.sign_in(&account("secret")).await.unwrap();
    *site.access.lock().unwrap() = "expired".into();

    let (one, two) = tokio::join!(titles(&source), titles(&source));
    assert_eq!(one.unwrap(), ["Paid title"]);
    assert_eq!(two.unwrap(), ["Paid title"]);
    assert_eq!(*site.refreshes.lock().unwrap(), 1);
}

#[tokio::test]
async fn a_session_that_cannot_be_refreshed_ends_and_says_to_sign_in_again() {
    let site = Arc::new(Site::default());
    let source = definition(&serve(site.clone()).await, json!({}));
    // A saved session whose refresh token the site no longer knows.
    source.restore_session(BTreeMap::from([
        ("access".to_string(), "old".to_string()),
        ("refresh".to_string(), "gone".to_string()),
    ]));
    let error = titles(&source).await.unwrap_err().to_string();
    assert!(error.contains("sign in again"), "{error}");
    assert!(!source.info().signed_in);
}

#[tokio::test]
async fn signing_out_tells_the_site_and_forgets_the_session() {
    let site = Arc::new(Site::default());
    let source = definition(&serve(site.clone()).await, json!({}));
    source.sign_in(&account("secret")).await.unwrap();
    source.sign_out().await;
    assert_eq!(*site.logouts.lock().unwrap(), 1);
    assert!(!source.info().signed_in);
    assert!(source.session().is_empty());
}

#[tokio::test]
async fn the_session_is_sent_to_the_base_address_and_never_to_a_mirror() {
    let site = Arc::new(Site::default());
    let mirror = serve(site.clone()).await;
    // The base address cannot be reached, so the mirror answers.
    let source = definition(
        "http://127.0.0.1:1",
        json!({
            "mirrors": [mirror],
            "popular": { "request": { "url": "{base}/open" }, "format": "json", "rows": "items", "card": { "id": "id", "title": "title" } }
        }),
    );
    source.restore_session(BTreeMap::from([("access".to_string(), "a1".to_string())]));
    assert_eq!(titles(&source).await.unwrap(), ["Open title"]);
    assert_eq!(*site.open_seen.lock().unwrap(), [None]);
}

#[tokio::test]
async fn a_sign_in_with_no_request_keeps_what_was_typed_as_the_session() {
    let site = Arc::new(Site::default());
    *site.access.lock().unwrap() = "pasted".into();
    let source = definition(
        &serve(site).await,
        json!({
            "auth": {
                "fields": [{ "id": "access", "name": "Token", "secret": true }],
                "headers": { "authorization": "Bearer {access}" }
            }
        }),
    );
    let typed = BTreeMap::from([("access".to_string(), "pasted".to_string())]);
    source.sign_in(&typed).await.unwrap();
    assert_eq!(titles(&source).await.unwrap(), ["Paid title"]);
}

#[test]
fn a_sign_in_that_could_leave_the_source_is_refused_when_it_loads() {
    let load = |auth: Value| {
        let mut def = json!({
            "schema": 1, "id": "p", "name": "P", "version": 1, "base_url": "https://p.test",
            "popular": { "request": { "url": "{base}/l" }, "format": "json", "rows": "i", "card": { "id": "a", "title": "b" } },
            "detail": { "request": { "url": "{base}/d" }, "format": "json", "title": "t" },
            "chapters": { "request": { "url": "{base}/c" }, "format": "json", "rows": "r", "chapter": { "id": "i" } },
            "pages": { "request": { "url": "{base}/p" }, "format": "json", "images": "i" }
        });
        def["auth"] = auth;
        akidev_manga::source::validate(&serde_json::from_value(def).unwrap())
            .unwrap_err()
            .to_string()
    };
    let field = json!([{ "id": "email", "name": "Email" }]);
    let vars = json!({ "access": "token" });
    assert!(
        load(json!({ "fields": field, "login": { "request": { "url": "https://elsewhere.test/login" }, "vars": vars } }))
            .contains("must start with {base}")
    );
    assert!(load(json!({ "fields": [] })).contains("at least one field"));
    assert!(
        load(json!({ "fields": [{ "id": "email", "name": "A" }, { "id": "email", "name": "B" }] }))
            .contains("listed twice")
    );
    assert!(
        load(json!({ "fields": [{ "id": "page", "name": "A" }] }))
            .contains("not a name the engine uses")
    );
    assert!(
        load(json!({ "fields": field, "login": { "request": { "url": "{base}/login" }, "vars": {} } }))
            .contains("vars say which values")
    );
    assert!(
        load(json!({ "fields": field, "refresh": { "request": { "url": "{base}/r" }, "vars": vars } }))
            .contains("refresh needs auth.login")
    );
}

#[tokio::test]
async fn the_app_is_told_each_time_the_session_changes() {
    let site = Arc::new(Site::default());
    let source = definition(&serve(site.clone()).await, json!({}));
    let told: Arc<Mutex<Vec<Option<String>>>> = Arc::default();
    let log = told.clone();
    source.watch_session(move |session| {
        log.lock()
            .unwrap()
            .push(session.map(|session| session["access"].clone()));
    });

    source.sign_in(&account("secret")).await.unwrap();
    *site.access.lock().unwrap() = "expired".into();
    titles(&source).await.unwrap();
    source.sign_out().await;

    // Signing in, the refresh, signing out. Restoring a saved session tells nobody.
    assert_eq!(
        *told.lock().unwrap(),
        [Some("a1".to_string()), Some("a2".to_string()), None]
    );
    source.restore_session(BTreeMap::from([("access".to_string(), "x".to_string())]));
    assert_eq!(told.lock().unwrap().len(), 3);
}
