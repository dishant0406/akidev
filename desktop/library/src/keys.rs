//! Where the master key is kept.
//!
//! The key that unlocks the saved files is made once, the first time something is saved, and kept
//! in the system's keychain (macOS Keychain, Windows Credential Manager), so a copy of the library
//! folder is useless on another machine or account. Where there is no keychain, or it refuses, the
//! key is kept in a file in the library folder that only the user can read. That is weaker, since
//! copying the whole folder copies the key, and it is the fallback, not the plan.

use crate::container::MasterKey;
use anyhow::{Context, bail};
use std::fs;
use std::path::Path;
use std::time::Duration;

const FILE: &str = "master.key";
#[cfg(any(target_os = "macos", windows))]
const SERVICE: &str = "live.akidev.Akidev";
#[cfg(any(target_os = "macos", windows))]
const ACCOUNT: &str = "library-master-key";

/// Written once the keychain has held the key, so a later failure of the keychain is never
/// answered with a new key that nothing saved so far can be opened with.
const IN_KEYCHAIN: &str = "keychain-in-use";

/// The master key, made if there is none. A key kept in the file stays the one in use. Once the
/// keychain has held the key it is the only place to look: when it does not answer (locked, a
/// permission dialog nobody answered) the error is returned and the next try asks again. Only a
/// library that never had its key in the keychain falls back to the file.
pub fn load(dir: &Path) -> anyhow::Result<MasterKey> {
    load_with(dir, from_keychain)
}

fn load_with(
    dir: &Path,
    keychain: impl FnOnce() -> anyhow::Result<MasterKey> + Send + 'static,
) -> anyhow::Result<MasterKey> {
    if dir.join(FILE).exists() {
        return from_file(dir);
    }
    match within(KEYCHAIN_WAIT, keychain) {
        Ok(key) => {
            fs::create_dir_all(dir)?;
            fs::write(dir.join(IN_KEYCHAIN), b"")?;
            Ok(key)
        }
        Err(error) if dir.join(IN_KEYCHAIN).exists() => {
            Err(error.context("the keychain holds this library's key and did not give it"))
        }
        Err(error) => {
            tracing::warn!("[library] no keychain, keeping the key in a file: {error:#}");
            from_file(dir)
        }
    }
}

/// How long the keychain gets to answer. It may be asking the user for permission, and a saved film
/// must not wait for ever on a dialog nobody sees.
const KEYCHAIN_WAIT: Duration = Duration::from_secs(20);

/// Runs `work` on its own thread and gives its answer, or an error when it takes longer than `time`.
fn within<T: Send + 'static>(
    time: Duration,
    work: impl FnOnce() -> anyhow::Result<T> + Send + 'static,
) -> anyhow::Result<T> {
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = sender.send(work());
    });
    receiver
        .recv_timeout(time)
        .unwrap_or_else(|_| bail!("the keychain did not answer in time"))
}

fn parse(bytes: &[u8]) -> anyhow::Result<MasterKey> {
    match <[u8; 32]>::try_from(bytes) {
        Ok(key) => Ok(MasterKey(key)),
        Err(_) => bail!("the stored key is not 32 bytes"),
    }
}

fn new_key() -> anyhow::Result<[u8; 32]> {
    let mut key = [0u8; 32];
    getrandom::fill(&mut key).map_err(|error| anyhow::anyhow!("no random numbers: {error}"))?;
    Ok(key)
}

fn from_file(dir: &Path) -> anyhow::Result<MasterKey> {
    let path = dir.join(FILE);
    match fs::read(&path) {
        Ok(bytes) => parse(&bytes),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir_all(dir)?;
            let key = new_key()?;
            write_private(&path, &key)?;
            Ok(MasterKey(key))
        }
        Err(error) => Err(error).with_context(|| format!("reading {}", path.display())),
    }
}

/// Writes a file only its owner can read.
fn write_private(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    use std::io::Write;
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let mut file = options
        .open(path)
        .with_context(|| format!("creating {}", path.display()))?;
    file.write_all(bytes)?;
    Ok(())
}

#[cfg(any(target_os = "macos", windows))]
fn from_keychain() -> anyhow::Result<MasterKey> {
    let entry = keyring::Entry::new(SERVICE, ACCOUNT)?;
    match entry.get_secret() {
        Ok(bytes) => parse(&bytes),
        Err(keyring::Error::NoEntry) => {
            let key = new_key()?;
            entry.set_secret(&key)?;
            Ok(MasterKey(key))
        }
        Err(error) => Err(error.into()),
    }
}

#[cfg(not(any(target_os = "macos", windows)))]
fn from_keychain() -> anyhow::Result<MasterKey> {
    bail!("this system has no keychain")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("akidev-keys-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn a_key_made_in_a_file_is_the_same_the_next_time() {
        let dir = scratch("file");
        let first = from_file(&dir).unwrap();
        let second = from_file(&dir).unwrap();
        assert_eq!(first.0, second.0);
        assert_ne!(first.0, [0u8; 32]);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(dir.join(FILE)).unwrap().permissions().mode();
            assert_eq!(mode & 0o077, 0, "others can read the key");
        }
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn a_keychain_that_held_the_key_is_never_replaced_by_a_file_key() {
        let dir = scratch("marker");
        let key = MasterKey([3; 32]);
        let given = load_with(&dir, move || Ok(key)).unwrap();
        assert_eq!(given.0, [3; 32]);
        // Later the keychain is locked: an error, and no new key is made.
        assert!(load_with(&dir, || bail!("locked")).is_err());
        assert!(!dir.join(FILE).exists());
        // A library that never had its key there falls back to the file.
        let fresh = scratch("never");
        let fallen = load_with(&fresh, || bail!("no keychain")).unwrap();
        assert_eq!(fallen.0, from_file(&fresh).unwrap().0);
        let _ = fs::remove_dir_all(dir);
        let _ = fs::remove_dir_all(fresh);
    }

    #[test]
    fn a_keychain_that_never_answers_is_given_up_on() {
        let slow = within(Duration::from_millis(100), || {
            std::thread::sleep(Duration::from_secs(5));
            Ok(1)
        });
        assert!(slow.err().unwrap().to_string().contains("did not answer"));
        assert_eq!(within(Duration::from_secs(5), || Ok(7)).unwrap(), 7);
    }

    #[test]
    fn a_key_that_is_the_wrong_size_is_an_error_and_not_a_new_key() {
        let dir = scratch("short");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(FILE), b"short").unwrap();
        assert!(load(&dir).is_err());
        assert_eq!(fs::read(dir.join(FILE)).unwrap(), b"short");
        let _ = fs::remove_dir_all(dir);
    }
}
