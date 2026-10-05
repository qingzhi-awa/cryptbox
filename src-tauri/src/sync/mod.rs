// 同步模块：对接服务端 API（登录/注册/校验/上传/下载/合并）。
use reqwest::blocking::{Client, Response};
use serde::Deserialize;
use std::time::Duration;

use crate::store::Entry;

pub mod pinning;

/// 同步服务默认端口（与飞牛端 TCP 监听端口一致）。
const DEFAULT_PORT: u16 = 5201;

/// 归一化服务器地址，支持多种输入形态：
/// - 完整 URL（含 scheme）→ 原样（去尾部 /）
/// - IP[:端口] / localhost[:端口] → **https://**（缺端口时补 5201）
/// - 域名[:端口] → https://
///
/// 注意：自 PT-02 起，未显式填写 scheme 的地址一律默认走 **加密的 HTTPS**；
/// 只有用户**显式输入 `http://…`** 才会走明文（服务端同端口双协议仍兼容）。
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
        return format!("https://{}", s);
    }

    // 无端口：IP / localhost → https + 默认端口；域名 → https（缺省 443，反向代理场景）
    if is_ipv4(s) || s == "localhost" {
        return format!("https://{}:{}", s, DEFAULT_PORT);
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

/// 把 HTTP 状态码翻译成用户可读的提示，避免把实现细节（状态码/英文）直接抛给用户。
/// 401/403 → 会话失效；429 → 频率限制；5xx → 服务端暂时异常；其余为通用失败。
fn friendly_http_error(prefix: &str, status: reqwest::StatusCode) -> String {
    let code = status.as_u16();
    let detail = match code {
        401 | 403 => "登录状态已失效，请重新登录",
        429 => "操作过于频繁，请稍后再试",
        400 => "请求被服务器拒绝，请检查输入内容",
        413 => "数据过大，服务器拒绝接收",
        c if c >= 500 => "服务器暂时无法处理，请稍后重试",
        _ => "请求失败，请检查网络与服务器地址",
    };
    format!("{}：{}", prefix, detail)
}

/// 把网络层错误翻译成用户可读的提示（区分为超时 / 连不上 / 其他）。
fn friendly_net_error(prefix: &str, e: reqwest::Error) -> String {
    if e.is_timeout() {
        format!("{}：连接超时，请检查服务器地址与网络", prefix)
    } else if e.is_connect() {
        format!("{}：无法连接到服务器，请确认地址、端口与证书信任状态", prefix)
    } else {
        format!("{}：{}", prefix, e)
    }
}

pub struct SyncClient {
    base_url: String,
    token: Option<String>,
    /// 已信任的对端证书指纹（HTTPS 时用于逐次响应校验，封堵探测与请求之间的时间窗）。
    pinned_fp: Option<String>,
    /// HTTPS 时持有「可观测对端证书」的客户端，用于指纹固定。
    observed: Option<pinning::ObservedClient>,
    client: Client,
}

pub struct AuthResult {
    pub token: String,
    pub avatar: String,
    pub vault_key_enc: String,
    pub kdf_salt: String,
}

impl SyncClient {
    pub fn new(base_url: &str) -> Self {
        let base_url = normalize_server_url(base_url);
        // HTTPS → 自定义校验器客户端（自签可通过握手，但会记录对端证书以便固定指纹）；
        // 明文 HTTP → 普通客户端（无身份校验，由界面提示风险）。
        let observed = if pinning::is_https(&base_url) {
            pinning::observed_client(None, Duration::from_secs(15)).ok()
        } else {
            None
        };
        let client = observed
            .as_ref()
            .map(|o| o.client.clone())
            .unwrap_or_else(pinning::plain_client);
        Self {
            base_url,
            token: None,
            pinned_fp: None,
            observed,
            client,
        }
    }

    pub fn with_token(mut self, token: &str) -> Self {
        self.token = Some(token.to_string());
        self
    }

    /// 设置已信任的证书指纹。
    ///
    /// 重建客户端：把指纹交给 rustls 校验器，**握手阶段**即拒绝不一致的证书——
    /// 保证任何凭据都不会发往指纹不符的服务器；响应阶段再校验一次作为兜底。
    pub fn with_pinned_fp(mut self, fp: &str) -> Self {
        self.pinned_fp = Some(fp.to_string());
        if pinning::is_https(&self.base_url) {
            if let Ok(oc) = pinning::observed_client(Some(fp.to_string()), Duration::from_secs(15)) {
                self.client = oc.client.clone();
                self.observed = Some(oc);
            }
        }
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

    /// 发送请求并校验对端证书指纹（HTTPS 且已固定时）。
    fn send(&self, req: reqwest::blocking::RequestBuilder) -> Result<Response, String> {
        let resp = req.send().map_err(|e| friendly_net_error("网络请求失败", e))?;
        if let Some(expected) = &self.pinned_fp {
            let actual = self
                .observed
                .as_ref()
                .and_then(|o| o.observed_fingerprint())
                .unwrap_or_default();
            if actual.is_empty() || &actual != expected {
                return Err(format!(
                    "{}:{}|{}|{}",
                    pinning::ERR_FP_CHANGED,
                    pinning::authority(&self.base_url),
                    actual,
                    expected
                ));
            }
        }
        Ok(resp)
    }

    pub fn register(
        &self,
        username: &str,
        password: &str,
        email: &str,
        code: &str,
        vault_key_enc: &str,
        kdf_salt: &str,
    ) -> Result<AuthResult, String> {
        let body = serde_json::json!({
            "username": username,
            "password": password,
            "email": email,
            "code": code,
            "vault_key_enc": vault_key_enc,
            "kdf_salt": kdf_salt,
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
            vault_key_enc: String,
            #[serde(default)]
            kdf_salt: String,
            #[serde(default)]
            error: String,
        }
        let resp = self.send(self.client.post(format!("{}{}", self.base_url, path)).json(&body))?;
        let status = resp.status();
        let text = resp.text().map_err(|e| e.to_string())?;
        let out: AuthResponse = serde_json::from_str(&text).unwrap_or(AuthResponse {
            token: String::new(),
            avatar: String::new(),
            vault_key_enc: String::new(),
            kdf_salt: String::new(),
            error: String::new(),
        });
        if !status.is_success() {
            if !out.error.is_empty() {
                return Err(out.error);
            }
            return Err(friendly_http_error("请求失败", status));
        }
        Ok(AuthResult {
            token: out.token,
            avatar: out.avatar,
            vault_key_enc: out.vault_key_enc,
            kdf_salt: out.kdf_salt,
        })
    }

    pub fn put_vault_key(&self, vault_key_enc: &str) -> Result<(), String> {
        let body = serde_json::json!({ "vault_key_enc": vault_key_enc });
        let resp = self.send(
            self.auth_header(self.client.put(format!("{}/api/vault-key", self.base_url)))
                .json(&body),
        )?;
        if resp.status().is_success() {
            Ok(())
        } else {
            Err(friendly_http_error("上传密钥失败", resp.status()))
        }
    }

    pub fn send_register_code(&self, email: &str) -> Result<(), String> {
        #[derive(Deserialize)]
        struct ErrResp {
            error: String,
        }
        let body = serde_json::json!({ "email": email });
        let resp = self.send(
            self.client
                .post(format!("{}/api/register/send-code", self.base_url))
                .json(&body),
        )?;
        if resp.status().is_success() {
            Ok(())
        } else {
            let text = resp.text().unwrap_or_default();
            let e: ErrResp = serde_json::from_str(&text).unwrap_or(ErrResp {
                error: String::new(),
            });
            if e.error.is_empty() {
                Err("请求失败，请稍后重试".into())
            } else {
                Err(e.error)
            }
        }
    }

    pub fn check(&self) -> Result<(), String> {
        let resp = self.send(self.auth_header(self.client.get(format!("{}/api/me", self.base_url))))?;
        if resp.status().is_success() {
            Ok(())
        } else {
            Err(friendly_http_error("登录状态校验失败", resp.status()))
        }
    }

    pub fn push(&self, entries: &[Entry]) -> Result<(), String> {
        let body = serde_json::json!({ "entries": entries });
        let resp = self.send(
            self.auth_header(self.client.put(format!("{}/api/vault", self.base_url)))
                .json(&body),
        )?;
        if resp.status().is_success() {
            Ok(())
        } else {
            Err(friendly_http_error("推送失败", resp.status()))
        }
    }

    /// 拉取头像图片（带认证与指纹固定），返回原始字节。
    /// 头像端点要求登录，WebView 的 <img> 无法携带 JWT，改由 Rust 侧代理拉取。
    pub fn get_avatar(&self, id: i64) -> Result<Vec<u8>, String> {
        let resp = self.send(
            self.auth_header(
                self.client
                    .get(format!("{}/api/avatar/{}", self.base_url, id)),
            ),
        )?;
        if !resp.status().is_success() {
            return Err(friendly_http_error("拉取头像失败", resp.status()));
        }
        resp.bytes().map(|b| b.to_vec()).map_err(|e| e.to_string())
    }

    pub fn pull(&self) -> Result<(Vec<Entry>, bool), String> {
        #[derive(Deserialize)]
        struct VaultResp {
            entries: Vec<Entry>,
            /// 账号级「置顶参与同步」开关：开启时客户端应采纳服务端置顶状态。
            #[serde(default)]
            pin_sync: bool,
        }
        let resp = self.send(self.auth_header(self.client.get(format!("{}/api/vault", self.base_url))))?;
        if !resp.status().is_success() {
            return Err(friendly_http_error("下载失败", resp.status()));
        }
        let out: VaultResp = resp.json().map_err(|e| e.to_string())?;
        // 安全要点（服务端下发数据不可信）：条目 id/uuid 完全由服务端控制，必须先
        // 净化再交给存储层。详见 sanitize_server_entries 的说明。
        Ok((sanitize_server_entries(out.entries), out.pin_sync))
    }

    /// 读取账号级「置顶参与同步」开关。
    pub fn pin_sync(&self) -> Result<bool, String> {
        #[derive(Deserialize)]
        struct Resp {
            #[serde(default)]
            pin_sync: bool,
        }
        let resp =
            self.send(self.auth_header(self.client.get(format!("{}/api/vault/pin-sync", self.base_url))))?;
        if !resp.status().is_success() {
            return Err(friendly_http_error("读取置顶同步设置失败", resp.status()));
        }
        let out: Resp = resp.json().map_err(|e| e.to_string())?;
        Ok(out.pin_sync)
    }

    /// 修改账号级「置顶参与同步」开关（服务端按账号保存，两端共用）。
    pub fn set_pin_sync(&self, enabled: bool) -> Result<(), String> {
        let resp = self.send(
            self.auth_header(self.client.post(format!("{}/api/vault/pin-sync", self.base_url)))
                .json(&serde_json::json!({ "enabled": enabled })),
        )?;
        if resp.status().is_success() {
            Ok(())
        } else {
            Err(friendly_http_error("保存置顶同步设置失败", resp.status()))
        }
    }

    /// 清空服务端密码库并重置 vault key（密码重置后放弃旧数据，不可恢复）。
    pub fn delete_vault(&self) -> Result<(), String> {
        let resp = self.send(self.auth_header(self.client.delete(format!("{}/api/vault", self.base_url))))?;
        if resp.status().is_success() {
            Ok(())
        } else {
            Err(friendly_http_error("清空密码库失败", resp.status()))
        }
    }
}

/// 合并两份条目（PT-04）：优先按全局唯一标识 `uuid` 归并，同一条目比较
/// `updated_at`、较新者覆盖较旧者；两侧 uuid 均为空时回退按本地 `id`（兼容旧协议）。
///
/// 为衔接"刚升级完成"的混合状态，先做一次**归一化**：若某条 uuid 为空、而同 id 的
/// 另一条已有 uuid，则认定二者是同一条目并采用该 uuid——避免升级后首次合并把
/// 同一条目当成两条（重复条目）。
pub fn merge_entries(local: &[Entry], remote: &[Entry]) -> Vec<Entry> {
    use std::collections::HashMap;

    // 1) id → uuid（优先取非空者），用于把存量条目的空 uuid 归一化。
    let mut id_to_uuid: HashMap<i64, String> = HashMap::new();
    for e in local.iter().chain(remote.iter()) {
        if !e.uuid.is_empty() {
            id_to_uuid.entry(e.id).or_insert_with(|| e.uuid.clone());
        }
    }
    let normalize = |e: &Entry| -> Entry {
        let mut c = e.clone();
        if c.uuid.is_empty() {
            if let Some(u) = id_to_uuid.get(&c.id) {
                c.uuid = u.clone();
            }
        }
        c
    };
    // 2) 归并键：有 uuid 用 uuid（全局唯一），否则退回本地 id。
    let key_of = |e: &Entry| -> String {
        if e.uuid.is_empty() {
            format!("i:{}", e.id)
        } else {
            format!("u:{}", e.uuid)
        }
    };

    let mut map: HashMap<String, Entry> = HashMap::new();
    let mut order: Vec<String> = Vec::new();
    for e in local.iter().map(normalize) {
        let k = key_of(&e);
        if !map.contains_key(&k) {
            order.push(k.clone());
        }
        map.insert(k, e);
    }
    for e in remote.iter().map(normalize) {
        let k = key_of(&e);
        match map.get(&k) {
            Some(cur) if !should_replace(&e, cur) => {}
            Some(_) => {
                map.insert(k, e);
            }
            None => {
                order.push(k.clone());
                map.insert(k, e);
            }
        }
    }
    ensure_unique_ids(order.into_iter().filter_map(|k| map.remove(&k)).collect())
}

/// 保证合并结果内本地 id 不重复（PT-04 的必要配套）。
///
/// uuid 才是条目身份，但服务端仍以 `(user_id, id)` 为复合主键：若合并后出现两条
/// 相同 id 的条目（典型场景：两台设备各自新建了 id=1 的不同条目），整批上传会因
/// 主键冲突失败、或被去重逻辑丢弃。因此对重复/非正的 id 重新分配未占用的编号。
/// 分配顺序确定（按合并结果顺序），因此各端得到一致结果、不会来回抖动。
fn ensure_unique_ids(list: Vec<Entry>) -> Vec<Entry> {
    use std::collections::HashSet;
    let mut next = list.iter().map(|e| e.id).max().unwrap_or(0) + 1;
    let mut used: HashSet<i64> = HashSet::new();
    list.into_iter()
        .map(|mut e| {
            if e.id <= 0 || used.contains(&e.id) {
                e.id = next;
                next += 1;
            }
            used.insert(e.id);
            e
        })
        .collect()
}

/// 净化**服务端下发**的条目列表，使其可以安全地写入本地库。
///
/// 为什么必须做：`GET /api/vault` 返回的 `entries[].id` / `.uuid` 完全由对端控制。
/// 客户端把这些字段直接写进 SQLite（`id` 甚至就是本地主键），因此一个被攻陷的
/// 服务端（或明文 HTTP 下的中间人）可以借此破坏本地数据：
///   · 下发**负数 / 极大** id → 打乱本地主键空间，后续 `save_entry` 的
///     `WHERE id = ?`（前端只传它自己看到的 id）会命中错误行甚至写失败；
///   · 下发**重复 id** → 整批 INSERT 触发主键冲突，`replace_entries` 事务回滚，
///     用户表现为"同步成功但数据没更新"；
///   · 下发**重复 / 空 uuid** → 破坏"uuid 是条目身份"这一不变量，合并时把不同
///     条目错误折叠成一条（静默丢数据）；
///   · 下发**超长 uuid / 非法字符** → 污染 meta 键与合并映射。
///
/// 处理策略（本地编号一律由客户端重新分配，不再信任服务端的 id）：
///   1. 非法（负 / 0 以外的越界）id 统一置 0，交给下方按序重编；
///   2. uuid 去重：重复或空白的丢弃并重新生成 v4（uuid 是身份，不能重复）；
///   3. 最后统一重排 id：`ensure_unique_ids` 保证互不重复且为正。
///
/// 注意：**不做**任何解密/内容校验——密码与备注的密文由 `decrypt_entries`
/// 负责，这里只做"结构安全"层面的净化。
fn sanitize_server_entries(list: Vec<Entry>) -> Vec<Entry> {
    use std::collections::HashSet;
    // id 上限：给本地主键留出充裕空间，同时挡住溢出/恶意极大值。
    // i64::MAX 附近的 id 会让 `next = max + 1` 溢出，因此先收敛到合理区间。
    const MAX_ID: i64 = 1 << 40;
    let mut seen_uuid: HashSet<String> = HashSet::new();
    let mut out: Vec<Entry> = Vec::with_capacity(list.len());
    for mut e in list {
        // 1) id 收敛：非正或超过上限的一律置 0，稍后统一重编。
        if e.id <= 0 || e.id > MAX_ID {
            e.id = 0;
        }
        // 2) uuid 去重：空串或与前面重复的，重新生成随机 v4。
        if e.uuid.is_empty() || !seen_uuid.insert(e.uuid.clone()) {
            e.uuid = crate::crypto::new_uuid_v4();
            seen_uuid.insert(e.uuid.clone());
        }
        out.push(e);
    }
    // 3) 重排 id：保证互不重复且为正数。
    ensure_unique_ids(out)
}

/// 判断候选条目 `cand` 是否应覆盖现存条目 `cur`。
///
/// **裁决只按 `updated_at`，不按 `revision`。** 原因：
/// 服务端采用「整库上传」协议，每次 PUT 都把全部条目删掉重建，因此 `entries.revision`
/// 是**整库写入序号**而非条目版本——同一账号里任何人推一次，所有条目的 revision 都会 +1，
/// 哪怕内容一个字都没变。若把它当作条目版本参与跨端比较，会出现两个致命后果：
///   1. 未修改的旧副本因为 revision 被抬过，会被误判为"更新"从而覆盖其他设备的真实改动；
///   2. 客户端若能影响该值，只需填入极大值即可永久压制其他设备的修改。
/// （服务端已改为完全忽略客户端提交的 revision，见 `replaceEntries`；客户端也不再比较它。）
///
/// `updated_at` 由发生修改的那台设备生成、只有真正的编辑才会变更，因此它才是
/// "这份内容何时被改的"可靠依据。两端均已统一为带时区偏移的 RFC3339（UTC）。
///
/// 裁决顺序：
///   1. 解析不出时间的（历史脏数据）→ 保持现存条目，不做覆盖，避免抖动；
///   2. 都解析得出 → 时间较晚者胜（严格大于；相等时保持现存）。
fn should_replace(cand: &Entry, cur: &Entry) -> bool {
    match (
        chrono::DateTime::parse_from_rfc3339(&cand.updated_at),
        chrono::DateTime::parse_from_rfc3339(&cur.updated_at),
    ) {
        (Ok(tc), Ok(tt)) => tc > tt,
        _ => false,
    }
}

/// 置顶同步开启时，合并后以**服务端**的置顶状态为准：
/// 按 uuid 归并（空 uuid 的旧条目回退按 id）。仅在服务端存在的条目不受影响，
/// 本地新增条目保留本机置顶（随后随整库上传到服务端）。
pub fn apply_remote_pins(entries: &mut [Entry], remote: &[Entry]) {
    use std::collections::HashMap;
    let mut by_uuid: HashMap<String, bool> = HashMap::new();
    let mut by_id: HashMap<i64, bool> = HashMap::new();
    for e in remote {
        if !e.uuid.is_empty() {
            by_uuid.insert(e.uuid.clone(), e.pinned);
        }
        by_id.insert(e.id, e.pinned);
    }
    for e in entries.iter_mut() {
        let pinned = if !e.uuid.is_empty() {
            by_uuid
                .get(&e.uuid)
                .or_else(|| by_id.get(&e.id))
                .copied()
        } else {
            by_id.get(&e.id).copied()
        };
        if let Some(p) = pinned {
            e.pinned = p;
        }
    }
}

// 渗透测试集成用例（默认 #[ignore]，需显式指定环境变量与 --ignored 才运行）。
#[cfg(test)]
mod pentest_tests;
