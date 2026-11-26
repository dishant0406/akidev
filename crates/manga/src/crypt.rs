//! Decrypting what a site hands over already encrypted: the AES blocks a chapter protector wraps its page list in,
//! and the fixed-key blocks some sites put their image addresses in.
//!
//! Only decryption: nothing here makes a key, signs anything or talks to a site. `aes-cbc` is the one cipher the
//! sources in the catalog use, with the key and IV either given (hex) or derived from a passphrase the way a site's
//! own JavaScript derives them (`MD5(passphrase ++ salt)` repeated, which is OpenSSL's `EVP_BytesToKey`).

use aes::Aes256;
use aes::cipher::{BlockDecryptMut, KeyIvInit, block_padding::Pkcs7};
use anyhow::{Context, bail, ensure};
use md5::{Digest, Md5};

type Aes256Cbc = cbc::Decryptor<Aes256>;

/// How big the input is allowed to be. A chapter list or a page's image addresses are small; something far
/// larger is a misread selector, not a payload.
const MAX_INPUT: usize = 64 * 1024 * 1024;

/// A key of 32 bytes (AES-256) or 16 (AES-128) and an IV of 16.
pub struct Key {
    pub bytes: Vec<u8>,
    pub iv: Vec<u8>,
}

/// The bytes a text stands for: hex when `hex`, base64 otherwise. Whitespace is allowed (base64 is often wrapped).
pub fn bytes(text: &str, hex: bool) -> anyhow::Result<Vec<u8>> {
    let text = text.trim();
    ensure!(!text.is_empty(), "there is nothing to decode");
    ensure!(text.len() <= MAX_INPUT, "the value is too large to decrypt");
    if hex {
        ensure!(
            text.len().is_multiple_of(2) && text.bytes().all(|b| b.is_ascii_hexdigit()),
            "the value is not hexadecimal"
        );
        return (0..text.len())
            .step_by(2)
            .map(|at| {
                u8::from_str_radix(&text[at..at + 2], 16)
                    .with_context(|| format!("not hexadecimal at byte {at}"))
            })
            .collect();
    }
    // base64 as a `<script>` holds it: possibly wrapped across lines, possibly without its padding.
    let squashed: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    let unpadded = squashed.trim_end_matches('=');
    use base64::Engine;
    base64::engine::general_purpose::STANDARD
        .decode(&squashed)
        .or_else(|_| base64::engine::general_purpose::STANDARD_NO_PAD.decode(unpadded))
        .or_else(|_| base64::engine::general_purpose::URL_SAFE.decode(&squashed))
        .or_else(|_| base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(unpadded))
        .context("the value is not base64")
}

/// The key and IV a site derives from a passphrase and a salt: `MD5(previous ++ passphrase ++ salt)` repeated until
/// there are 48 bytes, the first 32 the key and the last 16 the IV (OpenSSL's `EVP_BytesToKey`, which is what the
/// chapter-protector scripts on Madara sites do).
pub fn derive(passphrase: &str, salt: &[u8]) -> Key {
    let password = passphrase.as_bytes();
    let mut material: Vec<u8> = Vec::with_capacity(48);
    let mut previous: Vec<u8> = Vec::new();
    while material.len() < 48 {
        let mut hasher = Md5::new();
        hasher.update(&previous);
        hasher.update(password);
        hasher.update(salt);
        previous = hasher.finalize().to_vec();
        material.extend_from_slice(&previous);
    }
    Key {
        bytes: material[..32].to_vec(),
        iv: material[32..48].to_vec(),
    }
}

/// The key a site states plainly: a hex key (32 or 16 bytes) and a hex IV.
pub fn fixed(key_hex: &str, iv_hex: &str) -> anyhow::Result<Key> {
    Ok(Key {
        bytes: bytes(key_hex, true).context("the key is not hexadecimal")?,
        iv: bytes(iv_hex, true).context("the IV is not hexadecimal")?,
    })
}

/// Removes the AES-CBC padding. What comes back is often text, but not always, so it is left as bytes.
pub fn decrypt(key: &Key, data: &[u8]) -> anyhow::Result<Vec<u8>> {
    ensure!(
        key.iv.len() == 16,
        "an AES IV is 16 bytes, not {}",
        key.iv.len()
    );
    ensure!(
        data.len().is_multiple_of(16) && !data.is_empty(),
        "an AES-CBC ciphertext is a whole number of 16-byte blocks"
    );
    let mut buffer = data.to_vec();
    let plain: &[u8] = match key.bytes.len() {
        32 => Aes256Cbc::new_from_slices(&key.bytes, &key.iv)
            .map_err(|_| anyhow::anyhow!("the AES key is not 32 bytes"))?
            .decrypt_padded_mut::<Pkcs7>(&mut buffer),
        16 => cbc::Decryptor::<aes::Aes128>::new_from_slices(&key.bytes, &key.iv)
            .map_err(|_| anyhow::anyhow!("the AES key is not 16 bytes"))?
            .decrypt_padded_mut::<Pkcs7>(&mut buffer),
        other => bail!("an AES key is 32 bytes (256 bit) or 16 (128 bit), not {other}"),
    }
    .map_err(|_| {
        anyhow::anyhow!(
            "the padding is wrong: the key, the IV or the ciphertext is not the one expected"
        )
    })?;
    Ok(plain.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;
    use aes::cipher::BlockEncryptMut;

    fn hex(text: &str) -> Vec<u8> {
        bytes(text, true).expect("a test's own hex is valid")
    }

    fn encrypt(key: &[u8], iv: &[u8], plain: &[u8]) -> Vec<u8> {
        let mut buffer = plain.to_vec();
        buffer.resize(plain.len() + 16 - plain.len() % 16, 0);
        let length = buffer.len();
        cbc::Encryptor::<Aes256>::new_from_slices(key, iv)
            .unwrap()
            .encrypt_padded_mut::<Pkcs7>(&mut buffer[..length], plain.len())
            .unwrap()
            .to_vec()
    }

    #[test]
    fn md5_is_the_standard_one() {
        use md5::Digest;
        assert_eq!(
            format!("{:x}", Md5::digest(b"abc")),
            "900150983cd24fb0d6963f7d28e17f72"
        );
    }

    /// NIST SP 800-38A, F.2.1/F.2.6: the first block of AES-256-CBC.
    #[test]
    fn aes_256_cbc_matches_the_nist_vector() {
        let key = hex("603deb1015ca71be2b73aef0857d77811f352c073b6108d72d9810a30914dff4");
        let iv = hex("000102030405060708090a0b0c0d0e0f");
        let mut plain = hex("6bc1bee22e409f96e93d7e117393172a");
        plain.extend_from_slice(&[16; 16]);
        let data = encrypt(&key, &iv, &plain);
        let out = decrypt(&Key { bytes: key, iv }, &data).unwrap();
        assert_eq!(&out[..16], &plain[..16]);
    }

    #[test]
    fn a_passphrase_and_salt_derive_forty_eight_bytes_and_round_trip() {
        let key = derive("secret", &[0, 1, 2, 3, 4, 5, 6, 7]);
        assert_eq!((key.bytes.len(), key.iv.len()), (32, 16));
        // No salt and no passphrase gives the same thing twice: it is a function, not a random draw.
        assert_eq!(derive("p", b"1234").bytes, derive("p", b"1234").bytes);

        let plain = br#"["https://img.test/1.jpg","https://img.test/2.jpg"]"#;
        let data = encrypt(&key.bytes, &key.iv, plain);
        assert_eq!(decrypt(&key, &data).unwrap(), plain);
    }

    #[test]
    fn a_wrong_key_or_a_bad_block_is_an_error_not_panic() {
        let good = derive("right", b"saltsalt");
        let bad = derive("wrong", b"saltsalt");
        let data = encrypt(&good.bytes, &good.iv, b"some plaintext that is long enough");
        assert!(decrypt(&bad, &data).is_err());
        assert!(decrypt(&good, &data[..data.len() - 1]).is_err());
        assert!(decrypt(&good, &[]).is_err());
    }

    #[test]
    fn bytes_reads_hex_and_base64_and_refuses_junk() {
        assert_eq!(bytes("00ff10", true).unwrap(), [0x00, 0xff, 0x10]);
        assert!(bytes("00f", true).is_err());
        assert!(bytes("zz", true).is_err());
        use base64::Engine;
        let encoded = base64::engine::general_purpose::STANDARD.encode(b"hello world");
        assert_eq!(bytes(&encoded, false).unwrap(), b"hello world");
        // Wrapped and unpadded, the way a `<script>` holds it.
        let wrapped = "aGVs\nbG8g\nd29ybGQ";
        assert_eq!(bytes(wrapped, false).unwrap(), b"hello world");
        assert!(bytes("not base64!!", false).is_err());
        assert!(bytes("", false).is_err());
    }

    #[test]
    fn only_a_32_or_16_byte_key_is_accepted() {
        assert!(
            fixed(
                "00112233445566778899aabbccddeeff",
                "000102030405060708090a0b0c0d0e0f"
            )
            .is_ok()
        );
        let odd = Key {
            bytes: vec![0; 24],
            iv: vec![0; 16],
        };
        assert!(decrypt(&odd, &[0; 16]).is_err());
        let short_iv = Key {
            bytes: vec![0; 32],
            iv: vec![0; 8],
        };
        assert!(decrypt(&short_iv, &[0; 16]).is_err());
    }
}
