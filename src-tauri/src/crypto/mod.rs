// 加密模块：AES-256-GCM 加解密 + scrypt 密钥派生。
// 参数与 Go 端保持一致，确保旧数据库可解密。
use aes_gcm::{aead::Aead, Aes256Gcm, KeyInit, Nonce};
use base64::{engine::general_purpose, Engine};
use rand::RngCore;
use scrypt::{scrypt, Params};

/// scrypt 密钥派生：N=32768, r=8, p=1，输出 32 字节（与 Go 端一致）。
pub fn derive_key(password: &str, salt: &[u8]) -> Result<[u8; 32], String> {
    // log2(N)=15 → N=32768
    let params = Params::new(15, 8, 1, 32).map_err(|e| e.to_string())?;
    let mut key = [0u8; 32];
    scrypt(password.as_bytes(), salt, &params, &mut key).map_err(|e| e.to_string())?;
    Ok(key)
}

pub fn new_salt() -> Vec<u8> {
    let mut salt = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut salt);
    salt.to_vec()
}

/// AES-256-GCM 加密：返回 base64(nonce + ciphertext)，nonce 12 字节（与 Go 端一致）。
pub fn encrypt_bytes(key: &[u8; 32], plaintext: &[u8]) -> Result<String, String> {
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|e| e.to_string())?;
    let mut nonce_bytes = [0u8; 12];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ct = cipher.encrypt(nonce, plaintext).map_err(|e| e.to_string())?;
    let mut combined = nonce_bytes.to_vec();
    combined.extend_from_slice(&ct);
    Ok(general_purpose::STANDARD.encode(&combined))
}

pub fn decrypt_bytes(key: &[u8; 32], encoded: &str) -> Result<Vec<u8>, String> {
    let combined = general_purpose::STANDARD
        .decode(encoded)
        .map_err(|e| e.to_string())?;
    if combined.len() < 12 {
        return Err("ciphertext too short".into());
    }
    let (nonce_bytes, ct) = combined.split_at(12);
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|e| e.to_string())?;
    let nonce = Nonce::from_slice(nonce_bytes);
    cipher.decrypt(nonce, ct).map_err(|e| e.to_string())
}

pub fn encrypt_string(key: &[u8; 32], s: &str) -> Result<String, String> {
    encrypt_bytes(key, s.as_bytes())
}

pub fn decrypt_string(key: &[u8; 32], encoded: &str) -> Result<String, String> {
    let b = decrypt_bytes(key, encoded)?;
    String::from_utf8(b).map_err(|e| e.to_string())
}
