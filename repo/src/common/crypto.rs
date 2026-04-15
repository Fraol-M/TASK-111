use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm, Key, Nonce,
};
use base64::{engine::general_purpose::STANDARD as B64, Engine};

use crate::common::errors::AppError;

/// Holds the 32-byte AES-256-GCM key for at-rest field encryption.
#[derive(Clone)]
pub struct EncryptionKey([u8; 32]);

impl EncryptionKey {
    /// Parse a 64-character hex string into an EncryptionKey.
    pub fn from_hex(hex_str: &str) -> Result<Self, AppError> {
        let bytes = hex::decode(hex_str).map_err(|e| {
            AppError::Internal(format!("Invalid ENCRYPTION_KEY_HEX: {}", e))
        })?;
        if bytes.len() != 32 {
            return Err(AppError::Internal(
                "ENCRYPTION_KEY_HEX must decode to exactly 32 bytes".into(),
            ));
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        Ok(EncryptionKey(arr))
    }

    /// Encrypt plaintext using AES-256-GCM.
    /// Wire format: base64(random_nonce_12_bytes || ciphertext_with_tag)
    pub fn encrypt(&self, plaintext: &str) -> Result<String, AppError> {
        let key = Key::<Aes256Gcm>::from_slice(&self.0);
        let cipher = Aes256Gcm::new(key);
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        let ciphertext = cipher
            .encrypt(&nonce, plaintext.as_bytes())
            .map_err(|e| AppError::Internal(format!("Encryption failed: {}", e)))?;

        let mut combined = nonce.to_vec();
        combined.extend_from_slice(&ciphertext);
        Ok(B64.encode(&combined))
    }

    /// Decrypt a value produced by `encrypt`.
    pub fn decrypt(&self, encoded: &str) -> Result<String, AppError> {
        let combined = B64
            .decode(encoded)
            .map_err(|e| AppError::Internal(format!("Decryption base64 error: {}", e)))?;

        if combined.len() < 12 {
            return Err(AppError::Internal("Ciphertext too short".into()));
        }

        let (nonce_bytes, ciphertext) = combined.split_at(12);
        let key = Key::<Aes256Gcm>::from_slice(&self.0);
        let cipher = Aes256Gcm::new(key);
        let nonce = Nonce::from_slice(nonce_bytes);

        let plaintext = cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| AppError::Internal(format!("Decryption failed: {}", e)))?;

        String::from_utf8(plaintext)
            .map_err(|e| AppError::Internal(format!("Decrypted data not UTF-8: {}", e)))
    }

    /// Mask a string, showing only the last `visible_chars` characters.
    /// e.g. mask("12345678", 4) → "****5678"
    pub fn mask(value: &str, visible_chars: usize) -> String {
        let len = value.len();
        if len <= visible_chars {
            return "*".repeat(len);
        }
        let hidden = len - visible_chars;
        format!("{}{}", "*".repeat(hidden), &value[hidden..])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_key() -> EncryptionKey {
        EncryptionKey::from_hex(
            "0000000000000000000000000000000000000000000000000000000000000001",
        )
        .unwrap()
    }

    #[test]
    fn encrypt_decrypt_round_trip() {
        let key = test_key();
        let plaintext = "sensitive_value_123";
        let ciphertext = key.encrypt(plaintext).unwrap();
        let decrypted = key.decrypt(&ciphertext).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn encrypt_produces_different_ciphertext_each_time() {
        let key = test_key();
        let c1 = key.encrypt("same_value").unwrap();
        let c2 = key.encrypt("same_value").unwrap();
        // Due to random nonce, outputs should differ
        assert_ne!(c1, c2);
    }

    #[test]
    fn decrypt_fails_on_tampered_data() {
        let key = test_key();
        let mut ct = key.encrypt("secret").unwrap();
        // Tamper with last byte
        let mut bytes = ct.into_bytes();
        *bytes.last_mut().unwrap() ^= 0xFF;
        ct = String::from_utf8(bytes).unwrap_or_default();
        assert!(key.decrypt(&ct).is_err());
    }

    #[test]
    fn mask_hides_all_but_last_n() {
        assert_eq!(EncryptionKey::mask("12345678", 4), "****5678");
        assert_eq!(EncryptionKey::mask("abc", 4), "***");
        assert_eq!(EncryptionKey::mask("", 4), "");
    }
}
