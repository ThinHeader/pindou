//! 把微信 `session_key` 用 AES-256-GCM 封存进数据库。
//!
//! 密钥从 `JWT_SECRET` 派生，不另存一份。密文可以落库；明文、AppKey、session_key 都不能打进日志或响应。

use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use sha2::{Digest, Sha256};
use uuid::Uuid;

fn derived_key(jwt_secret: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"pindou-wx-session-key-v1:");
    hasher.update(jwt_secret.as_bytes());
    hasher.finalize().into()
}

fn random_nonce() -> [u8; 12] {
    let mut nonce = [0u8; 12];
    nonce.copy_from_slice(&Uuid::new_v4().into_bytes()[..12]);
    for (slot, byte) in nonce.iter_mut().zip(Uuid::new_v4().into_bytes()) {
        *slot ^= byte;
    }
    nonce
}

pub fn seal(jwt_secret: &str, session_key: &str) -> Result<String, String> {
    if jwt_secret.is_empty() || session_key.is_empty() {
        return Err("empty".into());
    }
    let key = derived_key(jwt_secret);
    let nonce = random_nonce();
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key));
    let ciphertext = cipher
        .encrypt(
            Nonce::from_slice(&nonce),
            Payload {
                msg: session_key.as_bytes(),
                aad: b"wx-session-key",
            },
        )
        .map_err(|_| "seal".to_string())?;
    let mut blob = Vec::with_capacity(12 + ciphertext.len());
    blob.extend_from_slice(&nonce);
    blob.extend_from_slice(&ciphertext);
    Ok(STANDARD.encode(blob))
}

pub fn open(jwt_secret: &str, stored: &str) -> Result<String, String> {
    if jwt_secret.is_empty() {
        return Err("empty".into());
    }
    let blob = STANDARD
        .decode(stored.trim())
        .map_err(|_| "decode".to_string())?;
    if blob.len() < 12 + 16 {
        return Err("short".into());
    }
    let (nonce, ciphertext) = blob.split_at(12);
    let key = derived_key(jwt_secret);
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key));
    let plain = cipher
        .decrypt(
            Nonce::from_slice(nonce),
            Payload {
                msg: ciphertext,
                aad: b"wx-session-key",
            },
        )
        .map_err(|_| "open".to_string())?;
    let text = String::from_utf8(plain).map_err(|_| "utf8".to_string())?;
    if text.is_empty() {
        return Err("empty".into());
    }
    Ok(text)
}

#[doc(hidden)]
pub fn seal_session_key_for_tests(jwt_secret: &str, session_key: &str) -> Result<String, String> {
    seal(jwt_secret, session_key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seal_roundtrip_hides_plaintext_and_rejects_wrong_secret() {
        let secret = "jwt-secret-for-session-seal";
        let session = "session-key-plain-value";
        let stored = seal(secret, session).unwrap();
        assert!(!stored.contains(session));
        assert_eq!(open(secret, &stored).unwrap(), session);
        assert!(open("other-secret", &stored).is_err());
        assert!(seal(secret, "").is_err());
        assert!(open(secret, "aaaa").is_err());
    }
}
