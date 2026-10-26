//! The manga engine's HTTP client. It is its own (rustls, TLS 1.2 and 1.3) because the workspace client uses the
//! system's native TLS, which on Apple platforms cannot speak TLS 1.3, and sites such as MangaDex now require it.

use std::sync::{Once, OnceLock};
use std::time::Duration;

/// A client builder, once rustls has been given its crypto library: every client in the engine starts from here.
pub fn client_builder() -> reqwest::ClientBuilder {
    static PROVIDER: Once = Once::new();
    PROVIDER.call_once(|| {
        // Fails only when one is already installed, which is just as good.
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
    let builder = reqwest::Client::builder();
    #[cfg(target_os = "android")]
    let builder = builder.tls_certs_only(android_roots().clone());
    builder
}

/// Android has no certificate store a Rust library can ask for, so the system's own list of trusted authorities is read from
/// where the phone keeps it (the newer place first), once.
#[cfg(target_os = "android")]
fn android_roots() -> &'static Vec<reqwest::Certificate> {
    static ROOTS: OnceLock<Vec<reqwest::Certificate>> = OnceLock::new();
    ROOTS.get_or_init(|| {
        let folder = ["/apex/com.android.conscrypt/cacerts", "/system/etc/security/cacerts"]
            .into_iter()
            .find(|path| std::path::Path::new(path).is_dir());
        let files = folder.and_then(|path| std::fs::read_dir(path).ok()).into_iter().flatten().flatten();
        files
            .filter_map(|file| std::fs::read(file.path()).ok())
            .filter_map(|pem| reqwest::Certificate::from_pem(&pem).ok())
            .collect()
    })
}

pub fn client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        client_builder()
            .user_agent("AkiManga/1.0")
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .build()
            .expect("http client builds")
    })
}

/// Reads a response body, failing once it grows past `max` bytes.
pub async fn read_capped(mut response: reqwest::Response, max: usize) -> anyhow::Result<Vec<u8>> {
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        anyhow::ensure!(body.len() + chunk.len() <= max, "the response is too large");
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}
