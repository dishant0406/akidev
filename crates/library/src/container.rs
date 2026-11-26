//! The file a saved item is kept in.
//!
//! The media is cut into chunks, and each chunk is encrypted on its own, so a player can read
//! from anywhere in the file without decrypting what is before it. Each file has its own key,
//! which is stored in the file's header, encrypted with the library's master key (see `keys`).
//! A copy of the file is therefore unreadable without the master key of the machine it was made
//! on. This keeps casual copying out. It is not DRM: the app has to be able to read the file, so
//! someone with the app and the key can too.
//!
//! Layout: a 128 byte header, then the chunks, each `CHUNK` bytes of media plus a 16 byte tag
//! (the last one is shorter). A chunk's nonce is the file's salt and its number, and its
//! authenticated data is its number and whether it is the last, so chunks cannot be reordered
//! or the file cut short without the tags failing.

use anyhow::{Context, bail, ensure};
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

const MAGIC: &[u8; 4] = b"AKC1";
const VERSION: u16 = 1;
const HEADER: usize = 128;
const TAG: usize = 16;
/// Media bytes in a full chunk.
pub const CHUNK: usize = 64 * 1024;
/// How many decrypted chunks a reader keeps, which is what a player re-reads when it seeks a little.
const KEPT: usize = 32;

/// The library's master key. It wraps each file's own key.
#[derive(Clone)]
pub struct MasterKey(pub [u8; 32]);

/// The part of the header the wrapped key is bound to.
struct Header {
    chunk_size: u32,
    plain_len: u64,
    salt: [u8; 16],
    wrap_nonce: [u8; 24],
    wrapped_key: [u8; 48],
}

impl Header {
    /// The fixed fields before the wrapped key: what the key's tag covers.
    fn bound(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(36);
        bytes.extend_from_slice(MAGIC);
        bytes.extend_from_slice(&VERSION.to_le_bytes());
        bytes.extend_from_slice(&0u16.to_le_bytes());
        bytes.extend_from_slice(&self.chunk_size.to_le_bytes());
        bytes.extend_from_slice(&self.plain_len.to_le_bytes());
        bytes.extend_from_slice(&self.salt);
        bytes
    }

    fn to_bytes(&self) -> [u8; HEADER] {
        let mut bytes = [0u8; HEADER];
        let bound = self.bound();
        bytes[..bound.len()].copy_from_slice(&bound);
        let mut at = bound.len();
        bytes[at..at + 24].copy_from_slice(&self.wrap_nonce);
        at += 24;
        bytes[at..at + 48].copy_from_slice(&self.wrapped_key);
        bytes
    }

    fn from_bytes(bytes: &[u8; HEADER]) -> anyhow::Result<Header> {
        ensure!(&bytes[..4] == MAGIC, "this is not a saved Akidev file");
        let version = u16::from_le_bytes([bytes[4], bytes[5]]);
        ensure!(version == VERSION, "this saved file is from a newer Akidev");
        let chunk_size = u32::from_le_bytes(bytes[8..12].try_into()?);
        ensure!(
            chunk_size as usize == CHUNK,
            "this saved file has an unknown layout"
        );
        let mut header = Header {
            chunk_size,
            plain_len: u64::from_le_bytes(bytes[12..20].try_into()?),
            salt: [0; 16],
            wrap_nonce: [0; 24],
            wrapped_key: [0; 48],
        };
        header.salt.copy_from_slice(&bytes[20..36]);
        header.wrap_nonce.copy_from_slice(&bytes[36..60]);
        header.wrapped_key.copy_from_slice(&bytes[60..108]);
        Ok(header)
    }
}

/// Fills `buf` from `offset` without moving the file's position, so readers need no lock.
fn read_exact_at(file: &File, buf: &mut [u8], offset: u64) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        std::os::unix::fs::FileExt::read_exact_at(file, buf, offset)
    }
    #[cfg(windows)]
    {
        let mut done = 0;
        while done < buf.len() {
            let read = std::os::windows::fs::FileExt::seek_read(
                file,
                &mut buf[done..],
                offset + done as u64,
            )?;
            if read == 0 {
                return Err(std::io::ErrorKind::UnexpectedEof.into());
            }
            done += read;
        }
        Ok(())
    }
}

fn random<const N: usize>() -> anyhow::Result<[u8; N]> {
    let mut bytes = [0u8; N];
    getrandom::fill(&mut bytes).map_err(|error| anyhow::anyhow!("no random numbers: {error}"))?;
    Ok(bytes)
}

fn chunk_nonce(salt: &[u8; 16], index: u64) -> XNonce {
    let mut nonce = [0u8; 24];
    nonce[..16].copy_from_slice(salt);
    nonce[16..].copy_from_slice(&index.to_le_bytes());
    XNonce::from(nonce)
}

fn chunk_aad(index: u64, last: bool) -> [u8; 9] {
    let mut aad = [0u8; 9];
    aad[..8].copy_from_slice(&index.to_le_bytes());
    aad[8] = u8::from(last);
    aad
}

fn chunk_count(plain_len: u64) -> u64 {
    plain_len.div_ceil(CHUNK as u64).max(1)
}

/// Reads as much as fits in `buf`, so a short read means the end of the file.
fn read_full(file: &mut File, buf: &mut [u8]) -> std::io::Result<usize> {
    let mut filled = 0;
    while filled < buf.len() {
        let read = file.read(&mut buf[filled..])?;
        if read == 0 {
            break;
        }
        filled += read;
    }
    Ok(filled)
}

/// Encrypts the file `input` into `output`, and returns the media's length. The file is written
/// under a temporary name and renamed when complete, so `output` is never half written.
pub fn seal(input: &Path, output: &Path, master: &MasterKey) -> anyhow::Result<u64> {
    let result = write_sealed(input, output, master);
    if result.is_err() {
        // A half written file is never left behind.
        let _ = fs::remove_file(partial_path(output));
    }
    result
}

fn write_sealed(input: &Path, output: &Path, master: &MasterKey) -> anyhow::Result<u64> {
    let mut source = File::open(input).with_context(|| format!("opening {}", input.display()))?;
    let plain_len = source.metadata()?.len();
    let file_key: [u8; 32] = random()?;
    let mut header = Header {
        chunk_size: CHUNK as u32,
        plain_len,
        salt: random()?,
        wrap_nonce: random()?,
        wrapped_key: [0; 48],
    };
    let wrap = XChaCha20Poly1305::new((&master.0).into());
    let wrapped = wrap
        .encrypt(
            &XNonce::from(header.wrap_nonce),
            Payload {
                msg: &file_key,
                aad: &header.bound(),
            },
        )
        .map_err(|_| anyhow::anyhow!("could not wrap the file key"))?;
    header.wrapped_key.copy_from_slice(&wrapped);

    let partial = partial_path(output);
    let mut target =
        File::create(&partial).with_context(|| format!("creating {}", partial.display()))?;
    target.write_all(&header.to_bytes())?;
    let cipher = XChaCha20Poly1305::new((&file_key).into());
    let count = chunk_count(plain_len);
    let mut buffer = vec![0u8; CHUNK];
    for index in 0..count {
        let read = read_full(&mut source, &mut buffer)?;
        let last = index + 1 == count;
        ensure!(
            read == CHUNK || last,
            "{} changed while it was being saved",
            input.display()
        );
        let sealed = cipher
            .encrypt(
                &chunk_nonce(&header.salt, index),
                Payload {
                    msg: &buffer[..read],
                    aad: &chunk_aad(index, last),
                },
            )
            .map_err(|_| anyhow::anyhow!("could not encrypt a chunk"))?;
        target.write_all(&sealed)?;
    }
    target.sync_all()?;
    drop(target);
    fs::rename(&partial, output).with_context(|| format!("finishing {}", output.display()))?;
    Ok(plain_len)
}

fn partial_path(output: &Path) -> PathBuf {
    let mut name = output.as_os_str().to_owned();
    name.push(".part");
    PathBuf::from(name)
}

/// A sealed file opened for reading.
pub struct Sealed {
    file: File,
    cipher: XChaCha20Poly1305,
    salt: [u8; 16],
    plain_len: u64,
    /// Recently decrypted chunks, newest last.
    kept: Mutex<Vec<(u64, Vec<u8>)>>,
}

impl Sealed {
    /// Opens `path`. Fails when it is not a saved file, is from a newer version, was made with
    /// another master key, or has been cut short.
    pub fn open(path: &Path, master: &MasterKey) -> anyhow::Result<Sealed> {
        let file = File::open(path).with_context(|| format!("opening {}", path.display()))?;
        let mut bytes = [0u8; HEADER];
        read_exact_at(&file, &mut bytes, 0).context("this saved file is damaged")?;
        let header = Header::from_bytes(&bytes)?;
        let file_key = XChaCha20Poly1305::new((&master.0).into())
            .decrypt(
                &XNonce::from(header.wrap_nonce),
                Payload {
                    msg: &header.wrapped_key,
                    aad: &header.bound(),
                },
            )
            .map_err(|_| anyhow::anyhow!("this saved file cannot be opened on this device"))?;
        let expected =
            HEADER as u64 + header.plain_len + chunk_count(header.plain_len) * TAG as u64;
        if file.metadata()?.len() != expected {
            bail!("this saved file is incomplete");
        }
        Ok(Sealed {
            file,
            cipher: XChaCha20Poly1305::new(file_key.as_slice().into()),
            salt: header.salt,
            plain_len: header.plain_len,
            kept: Mutex::new(Vec::new()),
        })
    }

    /// The media's length in bytes.
    pub fn len(&self) -> u64 {
        self.plain_len
    }

    pub fn is_empty(&self) -> bool {
        self.plain_len == 0
    }

    /// Reads up to `buf.len()` bytes of media starting at `offset`. Returns 0 at the end.
    pub fn read_at(&self, offset: u64, buf: &mut [u8]) -> anyhow::Result<usize> {
        let mut done = 0;
        while done < buf.len() && offset + (done as u64) < self.plain_len {
            let at = offset + done as u64;
            let index = at / CHUNK as u64;
            let chunk = self.chunk(index)?;
            let within = (at % CHUNK as u64) as usize;
            let take = (chunk.len() - within).min(buf.len() - done);
            buf[done..done + take].copy_from_slice(&chunk[within..within + take]);
            done += take;
        }
        Ok(done)
    }

    fn chunk(&self, index: u64) -> anyhow::Result<Vec<u8>> {
        if let Some((_, data)) = self
            .kept
            .lock()
            .unwrap()
            .iter()
            .find(|(kept, _)| *kept == index)
        {
            return Ok(data.clone());
        }
        let count = chunk_count(self.plain_len);
        let last = index + 1 == count;
        let plain = if last {
            self.plain_len - (count - 1) * CHUNK as u64
        } else {
            CHUNK as u64
        } as usize;
        let mut sealed = vec![0u8; plain + TAG];
        read_exact_at(
            &self.file,
            &mut sealed,
            HEADER as u64 + index * (CHUNK + TAG) as u64,
        )
        .context("this saved file is damaged")?;
        let data = self
            .cipher
            .decrypt(
                &chunk_nonce(&self.salt, index),
                Payload {
                    msg: &sealed,
                    aad: &chunk_aad(index, last),
                },
            )
            .map_err(|_| anyhow::anyhow!("this saved file is damaged"))?;
        let mut kept = self.kept.lock().unwrap();
        if kept.len() >= KEPT {
            kept.remove(0);
        }
        kept.push((index, data.clone()));
        Ok(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn master(byte: u8) -> MasterKey {
        MasterKey([byte; 32])
    }

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("akidev-container-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn media(len: usize) -> Vec<u8> {
        (0..len)
            .map(|i| (i.wrapping_mul(31) ^ (i >> 8)) as u8)
            .collect()
    }

    fn sealed_file(dir: &Path, bytes: &[u8]) -> PathBuf {
        let input = dir.join("in.mkv");
        fs::write(&input, bytes).unwrap();
        let output = dir.join("out.akc");
        assert_eq!(
            seal(&input, &output, &master(1)).unwrap(),
            bytes.len() as u64
        );
        output
    }

    #[test]
    fn what_is_sealed_reads_back_from_anywhere() {
        let dir = scratch("roundtrip");
        let bytes = media(CHUNK * 3 + 1234);
        let sealed = Sealed::open(&sealed_file(&dir, &bytes), &master(1)).unwrap();
        assert_eq!(sealed.len(), bytes.len() as u64);
        // Across a chunk boundary, from the very end, and past it.
        for (at, len) in [
            (0, 10),
            (CHUNK as u64 - 5, 20),
            (bytes.len() as u64 - 7, 100),
            (bytes.len() as u64 + 5, 4),
        ] {
            let mut buf = vec![0u8; len];
            let read = sealed.read_at(at, &mut buf).unwrap();
            let end = (at as usize + len).min(bytes.len());
            let want = if (at as usize) < bytes.len() {
                &bytes[at as usize..end]
            } else {
                &[][..]
            };
            assert_eq!(&buf[..read], want, "at {at}");
        }
        let mut all = vec![0u8; bytes.len()];
        assert_eq!(sealed.read_at(0, &mut all).unwrap(), bytes.len());
        assert_eq!(all, bytes);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn an_empty_file_and_an_exact_multiple_of_the_chunk_both_work() {
        let dir = scratch("edges");
        for len in [0, CHUNK, CHUNK * 2] {
            let bytes = media(len);
            let sealed = Sealed::open(&sealed_file(&dir, &bytes), &master(1)).unwrap();
            let mut all = vec![0u8; len];
            assert_eq!(sealed.read_at(0, &mut all).unwrap(), len);
            assert_eq!(all, bytes);
        }
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn the_file_is_not_the_media_and_needs_the_master_key() {
        let dir = scratch("key");
        let bytes = media(CHUNK + 10);
        let path = sealed_file(&dir, &bytes);
        let raw = fs::read(&path).unwrap();
        assert!(
            !raw.windows(64).any(|w| w == &bytes[1000..1064]),
            "the media shows through"
        );
        let error = Sealed::open(&path, &master(2)).err().unwrap();
        assert!(
            error
                .to_string()
                .contains("cannot be opened on this device")
        );
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn damage_cutting_and_reordering_are_caught() {
        let dir = scratch("damage");
        let bytes = media(CHUNK * 2 + 50);
        let path = sealed_file(&dir, &bytes);
        let good = fs::read(&path).unwrap();

        // A flipped byte in a chunk is found when that chunk is read, and only then.
        let mut flipped = good.clone();
        flipped[HEADER + 100] ^= 1;
        fs::write(&path, &flipped).unwrap();
        let sealed = Sealed::open(&path, &master(1)).unwrap();
        let mut buf = [0u8; 8];
        assert!(sealed.read_at(0, &mut buf).is_err());
        assert!(sealed.read_at(CHUNK as u64 * 2, &mut buf).is_ok());

        // A file cut short is refused on open.
        fs::write(&path, &good[..good.len() - 10]).unwrap();
        assert!(
            Sealed::open(&path, &master(1))
                .err()
                .unwrap()
                .to_string()
                .contains("incomplete")
        );

        // Two chunks swapped decrypt to nothing: the number is part of what the tag covers.
        let size = CHUNK + TAG;
        let mut swapped = good.clone();
        swapped[HEADER..HEADER + size].copy_from_slice(&good[HEADER + size..HEADER + 2 * size]);
        swapped[HEADER + size..HEADER + 2 * size].copy_from_slice(&good[HEADER..HEADER + size]);
        fs::write(&path, &swapped).unwrap();
        let sealed = Sealed::open(&path, &master(1)).unwrap();
        assert!(sealed.read_at(0, &mut buf).is_err());

        // A header that is not ours.
        fs::write(&path, b"not a saved file at all, just some bytes that are long enough to have a header ................................................................................").unwrap();
        assert!(Sealed::open(&path, &master(1)).is_err());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn a_file_is_only_there_once_it_is_complete() {
        let dir = scratch("partial");
        let path = sealed_file(&dir, &media(10));
        assert!(path.exists());
        assert!(!partial_path(&path).exists());
        // Sealing something that is not there leaves nothing behind.
        let gone = dir.join("gone.akc");
        assert!(seal(&dir.join("missing"), &gone, &master(1)).is_err());
        assert!(!gone.exists() && !partial_path(&gone).exists());
        let _ = fs::remove_dir_all(dir);
    }
}
