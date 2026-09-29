// 同步模块：对接服务端 API（登录/注册/校验/上传/下载/合并）。
use reqwest::blocking::Client;
use serde::Deserialize;
use std::time::Duration;

use crate::store::Entry;

/// 同步服务默认端口（与飞牛端 TCP 监听端口一致）。
const DEFAULT_PORT: u16 = 5201;

/// 归一化服务器地址，支持多种输入形态：
/// - 完整 URL（含 scheme）→ 原样（去尾部 /）
/// - IP[:端口] / localhost[:端口] → http://（缺端口时补 5201）
/// - 域名[:端口] → https://
pub fn normalize_server_url(input: &str) -> String {
    let s = input.trim();
    if s.is_empty() {
        return s.to_string();
    }
    if s.contains("://") {
        return s.trim_end_matches('/').to_string();
    }

    // 含 : → host:port
    if s.contains(':') {
        let host = s.split(':').next().unwrap_or("");
        if is_ipv4(host) || host == "localhost" {
            return format!("http://{}", s);
        }
        return format!("https://{}", s);
    }

    // 无端口：IP 或 localhost → http + 默认端口；域名 → https
    if is_ipv4(s) || s == "localhost" {
        return format!("http://{}:{}", s, DEFAULT_PORT);
    }
    format!("https://{}", s)
}

/// 判断字符串是否为点分十进制 IPv4（不含端口）。
fn is_ipv4(s: &str) -> bool {
    let parts: Vec<&str> = s.split('.').collect();
    if parts.len() != 4 {
        return false;
    }
    parts.iter().all(|p| {
        !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()) && p.parse::<u32>().map_or(false, |n| n <= 255)
    })
}

pub struct SyncClient {
    base_url: String,
    token: Option<String>,
    client: Client,
}

pub struct AuthResult {
    pub token: String,
    pub avatar: String,
}

impl SyncClient {
    pub fn new(base_url: &str) -> Self {
        Self {
            base_url: normalize_server_url(base_url),
            token: None,
            client: Client::builder()
                .timeout(Duration::from_secs(15))
                .build()
                .unwrap_or_default(),
        }
    }

    pub fn with_token(mut self, token: &str) -> Self {
        self.token = Some(token.to_string());
        self
    }

    fn auth_header(
        &self,
        req: reqwest::blocking::RequestBuilder,
    ) -> reqwest::blocking::RequestBuilder {
        match &self.token {
            Some(t) => req.bearer_auth(t),
            None => req,
        }
    }

    pub fn register(
        &self,
        username: &str,
        password: &str,
        email: &str,
        code: &str,
    ) -> Result<AuthResult, String> {
        let body = serde_json::json!({
            "username": username,
            "password": password,
            "email": email,
            "code": code,
        });
        self.auth_request("/api/register", body)
    }

    pub fn login(&self, username: &str, password: &str) -> Result<AuthResult, String> {
        let body = serde_json::json!({
            "username": username,
            "password": password,
        });
        self.auth_request("/api/login", body)
    }

    fn auth_request(&self, path: &str, body: serde_json::Value) -> Result<AuthResult, String> {
        #[derive(Deserialize)]
        struct AuthResponse {
            #[serde(default)]
            token: String,
            #[serde(default)]
            avatar: String,
            #[serde(default)]
            error: String,
        }
        let resp = self
            .client
            .post(format!("{}{}", self.base_url, path))
            .json(&body)
            .send()
            .map_err(|e| e.to_string())?;
        let status = resp.status();
        let text = resp.text().map_err(|e| e.to_string())?;
        let out: AuthResponse = serde_json::from_str(&text).unwrap_or(AuthResponse {
            token: String::new(),
            avatar: String::new(),
            error: String::new(),
        });
        if !status.is_success() {
            if !out.error.is_empty() {
                return Err(out.error);
            }
            return Err(format!("请求失败: {}", status));
        }
        Ok(AuthResult {
            token: out.token,
            avatar: out.avatar,
        })
    }

    pub fn send_register_code(&self, email: &str) -> Result<(), String> {
        #[derive(Deserialize)]
        struct ErrResp {
            error: String,
        }
        let body = serde_json::json!({ "email": email });
        let resp = self
            .client
            .post(format!("{}/api/register/send-code", self.base_url))
            .json(&body)
            .send()
            .map_err(|e| e.to_string())?;
        if resp.status().is_success() {
            Ok(())
        } else {
            let text = resp.text().unwrap_or_default();
            let e: ErrResp = serde_json::from_str(&text).unwrap_or(ErrResp {
                error: String::new(),
            });
            if e.error.is_empty() {
                Err("请求失败".into())
            } else {
                Err(e.error)
            }
        }
    }

    pub fn check(&self) -> Result<(), String> {
        let resp = self
            .auth_header(self.client.get(format!("{}/api/me", self.base_url)))
            .send()
            .map_err(|e| e.to_string())?;
        if resp.status().is_success() {
            Ok(())
        } else {
            Err(format!("token 无效: {}", resp.status()))
        }
    }

    pub fn push(&self, entries: &[Entry]) -> Result<(), String> {
        let body = serde_json::json!({ "entries": entries });
        let resp = self
            .auth_header(self.client.put(format!("{}/api/vault", self.base_url)))
            .json(&body)
            .send()
            .map_err(|e| e.to_string())?;
        if resp.status().is_success() {
            Ok(())
        } else {
            Err(format!("推送失败: {}", resp.status()))
        }
    }

    pub fn pull(&self) -> Result<Vec<Entry>, String> {
        #[derive(Deserialize)]
        struct VaultResp {
            entries: Vec<Entry>,
        }
        let resp = self
            .auth_header(self.client.get(format!("{}/api/vault", self.base_url)))
            .send()
            .map_err(|e| e.to_string())?;
        if !resp.status().is_success() {
            return Err(format!("下载失败: {}", resp.status()));
        }
        let out: VaultResp = resp.json().map_err(|e| e.to_string())?;
        Ok(out.entries)
    }
}

/// 合并两份条目：按 id 归并，同 id 比较 updated_at，较新者覆盖较旧者。
pub fn merge_entries(local: &[Entry], remote: &[Entry]) -> Vec<Entry> {
    use std::collections::HashMap;
    let mut map: HashMap<i64, Entry> = HashMap::new();
    let mut order: Vec<i64> = Vec::new();
    for e in local {
        if !map.contains_key(&e.id) {
            order.push(e.id);
        }
        map.insert(e.id, e.clone());
    }
    for e in remote {
        if let Some(cur) = map.get(&e.id) {
            if newer(&e.updated_at, &cur.updated_at) {
                map.insert(e.id, e.clone());
            }
        } else {
            order.push(e.id);
            map.insert(e.id, e.clone());
        }
    }
    order.into_iter().filter_map(|id| map.remove(&id)).collect()
}

fn newer(a: &str, b: &str) -> bool {
    match (
        chrono::DateTime::parse_from_rfc3339(a),
        chrono::DateTime::parse_from_rfc3339(b),
    ) {
        (Ok(ta), Ok(tb)) => ta > tb,
        _ => a > b,
    }
}
