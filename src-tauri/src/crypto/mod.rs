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

/// 生成 16 字节随机盐并以 base64 编码返回，用作主密钥派生盐（kdf_salt）。
/// 相比以用户名作盐，随机盐不会因用户改名而变化，避免既有数据无法解密。
pub fn new_salt_b64() -> String {
    general_purpose::STANDARD.encode(new_salt())
}

/// 从账号密码 + 派生盐得出 master key（用于加密/解密 vault key）。
/// 调用方优先传入服务端下发的随机盐 kdf_salt；
/// 历史账号无盐时回退传用户名，保证旧数据仍可解密。
pub fn derive_master_key(password: &str, salt: &str) -> Result<[u8; 32], String> {
    derive_key(password, salt.as_bytes())
}

/// 生成随机 32 字节 vault key（端到端加密的对称密钥）。
pub fn new_vault_key() -> Vec<u8> {
    let mut key = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut key);
    key.to_vec()
}

/// 生成条目的全局唯一同步标识（UUID v4，PT-04）。
///
/// 新条目在**创建时就地生成随机标识**：这样同一账号的两台设备各自新建条目
/// 也不会"撞号"互相覆盖（此前按本地自增 id 合并会静默丢数据）。
pub fn new_uuid_v4() -> String {
    let mut b = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut b);
    b[6] = (b[6] & 0x0f) | 0x40; // version 4
    b[8] = (b[8] & 0x3f) | 0x80; // variant RFC 4122
    let hex: String = b.iter().map(|x| format!("{:02x}", x)).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
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
