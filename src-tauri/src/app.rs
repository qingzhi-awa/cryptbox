// 应用核心：Tauri 命令注册与运行时状态管理。
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use rusqlite::Connection;
use tauri::Manager;
use tauri::State;
use tauri_plugin_dialog::DialogExt;

use crate::crypto;
use crate::import_export;
use crate::network;
use crate::secret::SecretStore;
use crate::settings;
use crate::store::{self, Entry};
use crate::sync;

/// 校验口令（verifier）派生时混入的常量串。
///
/// **禁止修改此值**：它参与 `verifier = KDF(password, VERIFIER_PLAIN)` 的计算，
/// 而 verifier 已按此值写入所有存量用户的数据库（app.db）。
/// 字符串里的 "passbook" 是项目旧名遗留（数据库文件已改名为 app.db），
/// 保留旧值是为了与已有数据兼容——一旦改动，所有老用户将无法用原密码解锁。
const VERIFIER_PLAIN: &str = "passbook-verifier-v1";

pub struct AppState {
    pub db: Mutex<Option<Connection>>,
    pub key: Mutex<Option<[u8; 32]>>,
    /// 系统凭据库封装（vault_key 不再明文落 SQLite，见 PT-03）。
    pub secret: SecretStore,
    /// 同步会话令牌（JWT）。仅存于 Rust 侧内存 + 系统凭据库，
    /// 前端不再持有或持久化原始令牌（PT-06）。
    pub token: Mutex<Option<String>>,
}

impl AppState {
    /// 获取数据库互斥锁，**容忍锁被毒化**（R11-07）。
    ///
    /// 默认的 `.lock().unwrap()` 在任一持锁线程 panic 后会**永久**返回 `Err`，使之后
    /// 所有数据库操作级联失败、应用实质不可用（R11-02 的越界 panic 正是被此放大成
    /// "持久砖化"）。这里用 `PoisonError::into_inner()` 取回内部数据继续服务：
    ///
    /// * 毒化只说明"曾有线程持锁时 panic"，**不代表数据已损坏**；
    /// * `Connection` 的内部状态由 rusqlite 维护，未提交事务在 unwind 时由
    ///   `Transaction` 的 Drop 回滚，因此恢复使用是安全的；
    /// * 宁可让某次操作报错重试，也不要把整个应用变成不可用。
    ///
    /// R12-04：毒化恢复是"低概率高后果"路径，故做两件事——打印显著日志（原先静默），
    /// 并主动 `ROLLBACK` 一次，自愈"panic 发生在已 BEGIN 但未走到 Drop"时残留的写锁；
    /// 随后 `clear_poison()` 复位毒化标志，避免之后每次取锁都重复回滚与打日志。
    pub fn db_lock(&self) -> std::sync::MutexGuard<'_, Option<Connection>> {
        match self.db.lock() {
            Ok(g) => g,
            Err(e) => {
                let g = e.into_inner();
                if let Some(conn) = g.as_ref() {
                    // 无活动事务时 ROLLBACK 会返回错误，属预期，忽略即可。
                    let _ = conn.execute_batch("ROLLBACK");
                }
                self.db.clear_poison();
                eprintln!(
                    "[app] 数据库互斥锁曾被毒化（有线程持锁时 panic），已回滚残留事务并恢复使用（R12-04）"
                );
                g
            }
        }
    }

    /// 获取解锁密钥互斥锁，容忍毒化（R11-07）。语义同 [`AppState::db_lock`]。
    pub fn key_lock(&self) -> std::sync::MutexGuard<'_, Option<[u8; 32]>> {
        match self.key.lock() {
            Ok(g) => g,
            Err(e) => {
                let g = e.into_inner();
                self.key.clear_poison();
                eprintln!("[app] 解锁密钥互斥锁曾被毒化，已恢复继续使用（R12-04）");
                g
            }
        }
    }

    /// 获取会话令牌互斥锁，容忍毒化（R11-07）。语义同 [`AppState::db_lock`]。
    pub fn token_lock(&self) -> std::sync::MutexGuard<'_, Option<String>> {
        match self.token.lock() {
            Ok(g) => g,
            Err(e) => {
                let g = e.into_inner();
                self.token.clear_poison();
                eprintln!("[app] 会话令牌互斥锁曾被毒化，已恢复继续使用（R12-04）");
                g
            }
        }
    }
}

pub fn init(app: &tauri::AppHandle) -> Result<(), String> {
    let db_path = store::db_file_path();
    let conn = store::open_store(&db_path)?;
    let secret = SecretStore::new(&store::data_dir());
    // 恢复上次会话：令牌曾交由凭据库保管，这里读回内存。
    let token = match secret.get_session() {
        Ok(t) if t.as_deref().map(|s| !s.is_empty()).unwrap_or(false) => t,
        Ok(_) => None,
        Err(e) => {
            eprintln!("[app] 读取会话令牌失败：{e}");
            None
        }
    };
    app.manage(AppState {
        db: Mutex::new(Some(conn)),
        key: Mutex::new(None),
        secret,
        token: Mutex::new(token),
    });
    Ok(())
}

/// 取出当前会话令牌；未登录时返回结构化错误 NOT_LOGGED_IN。
fn current_token(state: &AppState) -> Result<String, String> {
    state
        .token_lock()
        .clone()
        .filter(|t| !t.is_empty())
        .ok_or_else(|| "NOT_LOGGED_IN".to_string())
}

/// 记录会话令牌：内存 + 系统凭据库（下次启动自动恢复）。
fn store_session(state: &AppState, token: &str) {
    if token.is_empty() {
        return;
    }
    *state.token_lock() = Some(token.to_string());
    if let Err(e) = state.secret.set_session(token) {
        eprintln!("[app] 保存会话令牌失败：{e}");
    }
}

/// 清除会话令牌（内存 + 凭据库）。
fn clear_session_token(state: &AppState) {
    *state.token_lock() = None;
    if let Err(e) = state.secret.delete_session() {
        eprintln!("[app] 清除会话令牌失败：{e}");
    }
}

/// 主密钥副本守卫：Drop 时以零覆盖密钥字节，避免拷贝残留在栈或寄存器中。
/// require_unlock 返回的是密钥的按值拷贝（这样不必在命令执行期间一直持锁），
/// 因此必须由该守卫负责在使用完毕后擦除。
pub struct KeyGuard([u8; 32]);

impl KeyGuard {
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl Drop for KeyGuard {
    fn drop(&mut self) {
        for b in self.0.iter_mut() {
            *b = 0;
        }
        std::hint::black_box(&self.0);
    }
}

fn require_unlock(state: &AppState) -> Result<KeyGuard, String> {
    let guard = state.key_lock();
    (*guard).map(KeyGuard).ok_or_else(|| "未解锁".to_string())
}

/// 本地是否已设置主密码（salt 存在）。
fn local_master_initialized(db: &Connection) -> bool {
    store::get_meta(db, "salt").is_some()
}

/// 破坏性同步命令的前置校验。
///
/// PT-03 落地后，`sync_login` / `sync_register` 只写系统凭据库与服务器地址，
/// **不再触碰本地密钥材料**，因此已放开解锁要求。但：
///   · `recover_vault` 会改写当前账号的同步密钥（影响端到端解密）；
///   · `reset_vault_remote` 会清空服务端密码库。
/// 这两个属破坏性操作，永久保留「本地已设主密码则需先解锁」的约束。
///
/// 返回 LOCAL_LOCKED 时由前端提示用户先解锁。
fn guard_destructive_sync(state: &AppState) -> Result<(), String> {
    let need_unlock = {
        let guard = state.db_lock();
        guard.as_ref().map(local_master_initialized).unwrap_or(false)
    };
    if need_unlock && state.key_lock().is_none() {
        return Err("LOCAL_LOCKED".to_string());
    }
    Ok(())
}

// ---- 服务端身份校验（PT-02）----

/// 信任记录在 meta 中的键：`server_fp:<host[:port]>`。
fn trust_key(authority: &str) -> String {
    format!("server_fp:{}", authority)
}

/// 连接一个服务器时需要采取的确认动作（纯函数，便于单测覆盖策略全分支）。
#[derive(Debug, PartialEq, Eq)]
enum TrustAction {
    /// 已授权**且已记录指纹**且指纹一致 → 直接放行。
    ///
    /// 注意：只有"白名单 + 指纹"两者都在时才算可信。仅有白名单而没有指纹记录，
    /// 说明上一次写入是部分成功的（白名单已落盘、指纹未落盘），此时任何证书都
    /// 无法被比对，等价于首次使用 —— 这正是 TOFU 最危险的状态（PT-11 补充）。
    Allow,
    /// 已授权但指纹变了 → 必须二次确认（疑似中间人换证）。
    ConfirmChanged,
    /// 首次使用该地址（或白名单/指纹不完整）→ 必须确认（HTTPS 时同时展示证书指纹）。
    ConfirmFirstUse,
}

fn trust_action(authorized: bool, known: Option<&str>, fingerprint: &str) -> TrustAction {
    if !authorized {
        return TrustAction::ConfirmFirstUse;
    }
    match known {
        // 白名单在、指纹不在：无法比对，按首次使用处理（不静默放行）。
        None => TrustAction::ConfirmFirstUse,
        Some(k) if k != fingerprint => TrustAction::ConfirmChanged,
        Some(_) => TrustAction::Allow,
    }
}

/// 地址白名单在 meta 中的键：`server_allow:<host[:port]>`（PT-11）。
fn allow_key(authority: &str) -> String {
    format!("server_allow:{}", authority)
}

/// 弹出一个**原生**确认框，返回用户是否确认。
///
/// 与渲染层弹窗的关键区别：原生对话框由操作系统绘制，被控的渲染层既无法伪造、
/// 也无法自动点掉，因此它是"是否允许把密码库发往该服务器"的最后一道人工关卡。
fn confirm_native(
    app: &tauri::AppHandle,
    message: &str,
    ok_label: &str,
    cancel_label: &str,
) -> Result<(), String> {
    let yes = app
        .dialog()
        .message(message)
        .title("密匣 · 服务器信任确认")
        .kind(tauri_plugin_dialog::MessageDialogKind::Warning)
        .buttons(tauri_plugin_dialog::MessageDialogButtons::OkCancelCustom(
            ok_label.to_string(),
            cancel_label.to_string(),
        ))
        .blocking_show();
    if yes {
        Ok(())
    } else {
        Err("TRUST_DENIED".to_string())
    }
}

/// 服务器访问闸门：**地址白名单（PT-11）+ 证书指纹固定（PT-02）**。
///
/// · 首次连接某地址 → 原生确认框（HTTPS 时同时展示证书指纹）→ 确认后写入
///   地址白名单与指纹记录；
/// · 已允许过 → 仅校验指纹是否变化，变化时用原生告警框二次确认；
/// · 拒绝 → 返回 TRUST_DENIED，任何凭据/数据都不会发往该服务器。
///
/// 返回应固定的证书指纹（明文 HTTP 返回 None）。
fn ensure_server_allowed(
    app: &tauri::AppHandle,
    db: &Connection,
    server: &str,
) -> Result<Option<String>, String> {
    let url = sync::normalize_server_url(server);
    if url.is_empty() {
        return Err("服务器地址为空".to_string());
    }
    let authority = sync::pinning::authority(&url);
    let akey = allow_key(&authority);
    let fkey = trust_key(&authority);
    let allowed = store::get_meta(db, &akey).is_some();

    // 明文 HTTP：无证书可校验，仅做地址白名单确认（界面另有常驻风险提示）。
    if !sync::pinning::is_https(&url) {
        if allowed {
            return Ok(None);
        }
        confirm_native(
            app,
            &format!(
                "是否允许连接到同步服务器 {authority}？\n\n\
                 该地址为明文 HTTP：传输不加密，也无法验证服务器身份。\
                 若服务器支持 HTTPS，建议改用加密地址。"
            ),
            "允许连接",
            "取消",
        )?;
        store::set_meta(db, &akey, "1")?;
        return Ok(None);
    }

    let p = sync::pinning::probe(&url)?;
    let known = store::get_meta(db, &fkey);
    // 以握手实测到的主机为准（与上面按 URL 解析的结果一致，这里直接复用探测结果）。
    let _ = &p.authority;
    match trust_action(allowed, known.as_deref(), &p.fingerprint) {
        TrustAction::Allow => {}
        TrustAction::ConfirmChanged => {
            confirm_native(
                app,
                &format!(
                    "服务器 {authority} 的证书已变更！\n\n\
                     原指纹：{}\n新指纹：{}\n\n\
                     若你确认是服务器重装或更换了证书，可选择「仍要信任」；\
                     否则请点「取消」（存在中间人攻击的可能）。",
                    known.unwrap_or_default(),
                    p.fingerprint
                ),
                "仍要信任",
                "取消",
            )?;
        }
        TrustAction::ConfirmFirstUse => {
            let note = if p.verified {
                "该证书由受信任的 CA 签发。".to_string()
            } else {
                format!(
                    "该服务器使用自签证书。请与服务器端展示的「证书指纹」核对一致。\n\n\
                     证书指纹（SHA-256）：\n{}",
                    p.fingerprint
                )
            };
            confirm_native(
                app,
                &format!(
                    "首次连接同步服务器 {authority}。\n\n{note}\n\n\
                     是否允许连接并把密码库同步到该服务器？"
                ),
                "信任并连接",
                "取消",
            )?;
        }
    }
    // 确认后同时落地「地址白名单」与「指纹记录」——两条记录必须原子写入，
    // 否则会留下"地址已放行、但指纹缺失"的中间状态（此状态下任何证书都无法比对）。
    store::set_meta_batch(
        db,
        &[
            (akey.clone(), "1".to_string()),
            (fkey.clone(), p.fingerprint.clone()),
        ],
    )?;
    Ok(Some(p.fingerprint))
}

/// 按服务器地址构造同步客户端（已通过信任校验、并固定指纹）。
fn client_for(server: &str, pinned: &Option<String>) -> sync::SyncClient {
    let c = sync::SyncClient::new(server);
    match pinned {
        Some(fp) => c.with_pinned_fp(fp),
        None => c,
    }
}

// ---- 本地 vault key 保管（PT-03）----

fn key_from_bytes(bytes: &[u8]) -> Result<[u8; 32], String> {
    if bytes.len() != 32 {
        return Err("vault key 长度错误".into());
    }
    let mut k = [0u8; 32];
    k.copy_from_slice(bytes);
    Ok(k)
}

/// 读取本地缓存的 vault key：优先系统凭据库；发现旧版明文（meta）时一次性迁移。
fn read_vault_key(state: &AppState, db: &Connection) -> Result<[u8; 32], String> {
    if let Some(bytes) = state.secret.get()? {
        return key_from_bytes(&bytes);
    }
    // 旧版把 base64(vault_key) 明文写在 meta.vault_key：迁移到凭据库后清除。
    if let Some(b64) = store::get_meta(db, "vault_key") {
        use base64::{engine::general_purpose, Engine};
        match general_purpose::STANDARD.decode(&b64).map_err(|e| e.to_string()) {
            Ok(bytes) if bytes.len() == 32 => {
                let _ = state.secret.set(&bytes);
                let _ = store::delete_meta(db, "vault_key");
                let mut k = [0u8; 32];
                k.copy_from_slice(&bytes);
                return Ok(k);
            }
            _ => {
                // 无法解析的旧值直接清除，避免每次同步都失败。
                let _ = store::delete_meta(db, "vault_key");
            }
        }
    }
    Err("vault key 未初始化，请重新登录".into())
}

/// 写入 vault key 到系统凭据库（并清除可能的旧版明文残留）。
fn write_vault_key(state: &AppState, db: &Connection, key: &[u8; 32]) -> Result<(), String> {
    state.secret.set(key)?;
    let _ = store::delete_meta(db, "vault_key");
    Ok(())
}

/// 清除本地缓存的 vault key（凭据库条目 + 旧版明文）。
fn clear_vault_key(state: &AppState, db: &Connection) -> Result<(), String> {
    state.secret.delete()?;
    let _ = store::delete_meta(db, "vault_key");
    Ok(())
}

// ---- 主密码 / 解锁 ----

#[tauri::command]
pub fn init_db(state: State<'_, AppState>) -> Result<bool, String> {
    let guard = state.db_lock();
    let db = guard.as_ref().ok_or("数据库未初始化")?;
    Ok(store::get_meta(db, "salt").is_some())
}

#[tauri::command]
pub fn setup_master(password: String, state: State<'_, AppState>) -> Result<bool, String> {
    if password.chars().count() < 6 {
        return Err("主密码至少 6 位".into());
    }
    let salt = crypto::new_salt();
    let key = crypto::derive_key(&password, &salt)?;
    let verifier = crypto::encrypt_string(&key, VERIFIER_PLAIN)?;
    // R12-03：db guard 收窄到本作用域，写完即释放，避免与其他命令构成 AB-BA 死锁。
    // 其余命令统一按「key_lock → db_lock」取锁（require_unlock 先取 key 并即刻释放），
    // 原实现在此**同时**持有 db 与 key，顺序与全局相反，是唯一的锁序反转点。
    {
        let guard = state.db_lock();
        let db = guard.as_ref().ok_or("数据库未初始化")?;
        use base64::{engine::general_purpose, Engine};
        store::set_meta(db, "salt", &general_purpose::STANDARD.encode(&salt))?;
        store::set_meta(db, "verifier", &verifier)?;
    }
    *state.key_lock() = Some(key);
    Ok(true)
}

#[tauri::command]
pub fn unlock(password: String, state: State<'_, AppState>) -> Result<bool, String> {
    let salt_b64 = {
        let guard = state.db_lock();
        let db = guard.as_ref().ok_or("数据库未初始化")?;
        store::get_meta(db, "salt").ok_or("尚未初始化")?
    };
    use base64::{engine::general_purpose, Engine};
    let salt = general_purpose::STANDARD
        .decode(&salt_b64)
        .map_err(|e| e.to_string())?;
    let key = crypto::derive_key(&password, &salt)?;
    let verifier = {
        let guard = state.db_lock();
        let db = guard.as_ref().ok_or("数据库未初始化")?;
        store::get_meta(db, "verifier").unwrap_or_default()
    };
    let plain = crypto::decrypt_string(&key, &verifier).map_err(|e| e.to_string())?;
    if plain != VERIFIER_PLAIN {
        return Ok(false);
    }
    *state.key_lock() = Some(key);
    Ok(true)
}

/// 用零覆盖并清除内存中的主密钥，降低内存转储提取密钥的风险。
pub fn clear_key(state: &AppState) {
    let mut guard = state.key_lock();
    if let Some(mut key) = guard.take() {
        for b in key.iter_mut() {
            *b = 0;
        }
        std::hint::black_box(&key);
    }
}
#[tauri::command]
pub fn lock(state: State<'_, AppState>) {
    clear_key(&state);
}

#[tauri::command]
pub fn is_unlocked(state: State<'_, AppState>) -> bool {
    state.key_lock().is_some()
}

// ---- 密码条目 ----

#[tauri::command]
pub fn list_entries(state: State<'_, AppState>) -> Result<Vec<Entry>, String> {
    let key = require_unlock(&state)?;
    let guard = state.db_lock();
    let db = guard.as_ref().ok_or("数据库未初始化")?;
    store::list_entries(db, key.as_bytes())
}

#[tauri::command]
pub fn save_entry(entry: Entry, state: State<'_, AppState>) -> Result<Entry, String> {
    let key = require_unlock(&state)?;
    let guard = state.db_lock();
    let db = guard.as_ref().ok_or("数据库未初始化")?;
    store::save_entry(db, key.as_bytes(), entry)
}

#[tauri::command]
pub fn delete_entry(id: i64, soft: bool, state: State<'_, AppState>) -> Result<(), String> {
    let _key = require_unlock(&state)?;
    let guard = state.db_lock();
    let db = guard.as_ref().ok_or("数据库未初始化")?;
    if soft {
        store::soft_delete_entry(db, id)
    } else {
        store::hard_delete_entry(db, id).map(|_| ())
    }
}

/// 置顶 / 取消置顶（仅影响本设备显示顺序，不参与跨端同步）。
#[tauri::command]
pub fn pin_entry(id: i64, pinned: bool, state: State<'_, AppState>) -> Result<(), String> {
    let _key = require_unlock(&state)?;
    let guard = state.db_lock();
    let db = guard.as_ref().ok_or("数据库未初始化")?;
    store::set_pinned(db, id, pinned)
}

/// 按给定顺序重排条目（列表拖拽排序）。置顶条目与普通条目分属两组，组间不互换。
#[tauri::command]
pub fn reorder_entries(ids: Vec<i64>, state: State<'_, AppState>) -> Result<(), String> {
    let _key = require_unlock(&state)?;
    let guard = state.db_lock();
    let db = guard.as_ref().ok_or("数据库未初始化")?;
    store::reorder_entries(db, &ids)
}

#[tauri::command]
pub fn list_trash(state: State<'_, AppState>) -> Result<Vec<Entry>, String> {
    let key = require_unlock(&state)?;
    let guard = state.db_lock();
    let db = guard.as_ref().ok_or("数据库未初始化")?;
    // 读取回收站前先按保留天数清理过期条目。
    let days = crate::store::get_meta(db, "set_recycle_days")
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(30);
    let _ = store::purge_expired(db, days);
    store::list_trash(db, key.as_bytes())
}

#[tauri::command]
pub fn restore_entry(id: i64, state: State<'_, AppState>) -> Result<(), String> {
    let _key = require_unlock(&state)?;
    let guard = state.db_lock();
    let db = guard.as_ref().ok_or("数据库未初始化")?;
    store::restore_entry(db, id)
}

#[tauri::command]
pub fn purge_entry(id: i64, state: State<'_, AppState>) -> Result<(), String> {
    let _key = require_unlock(&state)?;
    let guard = state.db_lock();
    let db = guard.as_ref().ok_or("数据库未初始化")?;
    store::hard_delete_entry(db, id).map(|_| ())
}

#[tauri::command]
pub fn empty_trash(state: State<'_, AppState>) -> Result<usize, String> {
    let _key = require_unlock(&state)?;
    let guard = state.db_lock();
    let db = guard.as_ref().ok_or("数据库未初始化")?;
    store::empty_trash(db)
}

// ---- 导入 / 导出（PT-08）----
//
// 路径**不再由渲染层提供**：每个命令自己弹出系统文件对话框，并在同一函数内
// 完成读写。这样即使渲染层被完全控制，也无法让后端读写任意路径——它只能选择
// 用户在对话框里明确点选的那个文件。

/// 弹出"保存"对话框，返回用户选择的路径（取消则 None）。
fn pick_save_path(
    app: &tauri::AppHandle,
    file_name: &str,
    ext: &str,
    desc: &str,
) -> Result<Option<PathBuf>, String> {
    let picked = app
        .dialog()
        .file()
        .set_title("保存文件")
        .set_file_name(file_name)
        .add_filter(desc, &[ext])
        .blocking_save_file();
    match picked {
        None => Ok(None),
        Some(p) => p.into_path().map(Some).map_err(|e| e.to_string()),
    }
}

/// 弹出"打开"对话框，返回用户选择的路径（取消则 None）。
fn pick_open_path(
    app: &tauri::AppHandle,
    ext: &str,
    desc: &str,
    title: &str,
) -> Result<Option<PathBuf>, String> {
    let picked = app
        .dialog()
        .file()
        .set_title(title)
        .add_filter(desc, &[ext])
        .blocking_pick_file();
    match picked {
        None => Ok(None),
        Some(p) => p.into_path().map(Some).map_err(|e| e.to_string()),
    }
}

#[tauri::command]
pub fn export_txt(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<String, String> {
    let key = require_unlock(&state)?;
    let Some(path) = pick_save_path(&app, "CryPtBox.txt", "txt", "TXT")? else {
        return Ok(String::new());
    };
    let guard = state.db_lock();
    let db = guard.as_ref().ok_or("数据库未初始化")?;
    let list = store::list_entries(db, key.as_bytes())?;
    import_export::export_txt(&path.to_string_lossy(), &list)?;
    Ok(path.to_string_lossy().to_string())
}

#[tauri::command]
pub fn export_csv(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<String, String> {
    let key = require_unlock(&state)?;
    let Some(path) = pick_save_path(&app, "CryPtBox.csv", "csv", "CSV")? else {
        return Ok(String::new());
    };
    let guard = state.db_lock();
    let db = guard.as_ref().ok_or("数据库未初始化")?;
    let list = store::list_entries(db, key.as_bytes())?;
    import_export::export_csv(&path.to_string_lossy(), &list)?;
    Ok(path.to_string_lossy().to_string())
}

#[tauri::command]
pub fn import_csv(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<i64, String> {
    let key = require_unlock(&state)?;
    let Some(path) = pick_open_path(&app, "csv", "CSV", "导入密码（支持 Chrome / Edge 导出的 CSV）")? else {
        return Ok(0);
    };
    let entries = import_export::parse_csv_file(&path.to_string_lossy())?;
    let guard = state.db_lock();
    let db = guard.as_ref().ok_or("数据库未初始化")?;
    let mut count = 0i64;
    for e in entries {
        store::save_entry(db, key.as_bytes(), e)?;
        count += 1;
    }
    Ok(count)
}

#[tauri::command]
pub fn import_txt(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<i64, String> {
    let key = require_unlock(&state)?;
    let Some(path) = pick_open_path(&app, "txt", "TXT", "导入密码 TXT")? else {
        return Ok(0);
    };
    let entries = import_export::parse_txt_file(&path.to_string_lossy())?;
    let guard = state.db_lock();
    let db = guard.as_ref().ok_or("数据库未初始化")?;
    let mut count = 0i64;
    for e in entries {
        store::save_entry(db, key.as_bytes(), e)?;
        count += 1;
    }
    Ok(count)
}

/// 保存文本到用户选择的路径（模板下载 / 导出文本共用）。
/// 同样不接受调用方传入路径——只能由用户在对话框中选择。
#[tauri::command]
pub fn save_text_file(
    content: String,
    filename: String,
    app: tauri::AppHandle,
) -> Result<String, String> {
    // 只取文件名部分，防止调用方用 "../.." 影响默认目录展示。
    let name = filename.rsplit(['/', '\\']).next().unwrap_or("export.txt");
    let ext = name.rsplit('.').next().unwrap_or("txt");
    let Some(path) = pick_save_path(&app, name, ext, "文本文件")? else {
        return Ok(String::new());
    };
    std::fs::write(&path, content).map_err(|e| e.to_string())?;
    Ok(path.to_string_lossy().to_string())
}

// ---- 同步 ----

#[tauri::command]
pub fn get_server_config(state: State<'_, AppState>) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let guard = state.db_lock();
    if let Some(db) = guard.as_ref() {
        map.insert(
            "server".into(),
            store::get_meta(db, "server_url").unwrap_or_default(),
        );
        map.insert(
            "username".into(),
            store::get_meta(db, "server_username").unwrap_or_default(),
        );
    }
    map
}

#[tauri::command]
pub fn sync_register(
    server: String,
    username: String,
    password: String,
    email: String,
    code: String,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<HashMap<String, String>, String> {
    // 服务端身份校验（HTTPS 指纹固定）；明文模式返回 None。
    let pinned = {
        let guard = state.db_lock();
        let db = guard.as_ref().ok_or("数据库未初始化")?;
        ensure_server_allowed(&app, db, &server)?
    };
    // 生成随机派生盐：主密钥不再以用户名作盐，用户改名后仍能解密既有数据。
    let kdf_salt = crypto::new_salt_b64();
    let master_key = crypto::derive_master_key(&password, &kdf_salt)?;
    let vault_key = crypto::new_vault_key();
    let vault_key_enc = crypto::encrypt_bytes(&master_key, &vault_key)?;

    let c = client_for(&server, &pinned);
    let r = c.register(&username, &password, &email, &code, &vault_key_enc, &kdf_salt)?;
    let guard = state.db_lock();
    {
        let db = guard.as_ref().ok_or("数据库未初始化")?;
        // 服务器地址写入失败必须报错：否则下次启动读到空地址，
        // 用户会以为"配置丢了"，且无法找回该服务器。
        // 地址+账号两条同生共死，避免只写一半。
        store::set_meta_batch(
            db,
            &[
                ("server_url".to_string(), server.clone()),
                ("server_username".to_string(), username.clone()),
            ],
        )?;
        // vault key 存入系统凭据库（PT-03），不再明文落 SQLite。
        if let Err(e) = write_vault_key(&state, db, &key_from_bytes(&vault_key)?) {
            eprintln!("[app] 写入 vault key 失败：{e}");
        }
    }
    // 会话令牌仅保存在 Rust 侧 + 凭据库，不返回给前端（PT-06）。
    store_session(&state, &r.token);
    let mut map = HashMap::new();
    map.insert("username".into(), username);
    map.insert("avatar".into(), r.avatar);
    Ok(map)
}

#[tauri::command]
pub fn sync_login(
    server: String,
    username: String,
    password: String,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<HashMap<String, String>, String> {
    let pinned = {
        let guard = state.db_lock();
        let db = guard.as_ref().ok_or("数据库未初始化")?;
        ensure_server_allowed(&app, db, &server)?
    };
    let c = client_for(&server, &pinned);
    let r = c.login(&username, &password)?;
    // 派生盐优先取服务端下发的随机盐；历史账号无盐时回退用用户名，保持向后兼容。
    let salt = if r.kdf_salt.is_empty() {
        username.clone()
    } else {
        r.kdf_salt.clone()
    };
    let master_key = crypto::derive_master_key(&password, &salt)?;
    // 恢复 vault key：首次登录（vault_key_enc 为空）生成并上传，否则用 master key 解密。
    let vault_key = if r.vault_key_enc.is_empty() {
        let vk = crypto::new_vault_key();
        let enc = crypto::encrypt_bytes(&master_key, &vk)?;
        // 首次启用：服务端 vault_key_enc 为空，无需口令即可写入（R13-02 约定）；
        // 仍带上刚刚用于登录的口令，便于服务端未来收紧时不致失配。
        c.put_vault_key(&enc, &password)?;
        vk
    } else {
        // 解密失败通常意味着账号密码曾被重置：返回结构化标识，
        // 前端据此展示恢复入口（旧密码恢复 / 清空重建）。
        crypto::decrypt_bytes(&master_key, &r.vault_key_enc)
            .map_err(|_| "VAULT_KEY_MISMATCH".to_string())?
    };
    let guard = state.db_lock();
    {
        let db = guard.as_ref().ok_or("数据库未初始化")?;
        // 同 register：地址写入失败必须报错，否则下次启动服务器地址为空。
        store::set_meta_batch(
            db,
            &[
                ("server_url".to_string(), server.clone()),
                ("server_username".to_string(), username.clone()),
            ],
        )?;
        // vault key 存入系统凭据库（PT-03）。
        if let Err(e) = write_vault_key(&state, db, &key_from_bytes(&vault_key)?) {
            eprintln!("[app] 写入 vault key 失败：{e}");
        }
    }
    // 会话令牌仅保存在 Rust 侧 + 凭据库，不返回给前端（PT-06）。
    store_session(&state, &r.token);
    let mut map = HashMap::new();
    map.insert("username".into(), username);
    map.insert("avatar".into(), r.avatar);
    Ok(map)
}

/// 查询当前会话状态：是否已登录、以及记住的服务器与账号。
/// 前端据此渲染登录态，而无需接触令牌本身。
#[tauri::command]
pub fn session_info(state: State<'_, AppState>) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let logged_in = state
        .token_lock()
        .as_deref()
        .map(|t| !t.is_empty())
        .unwrap_or(false);
    map.insert("loggedIn".into(), if logged_in { "1" } else { "0" }.into());
    let guard = state.db_lock();
    if let Some(db) = guard.as_ref() {
        map.insert("server".into(), store::get_meta(db, "server_url").unwrap_or_default());
        map.insert(
            "username".into(),
            store::get_meta(db, "server_username").unwrap_or_default(),
        );
    }
    map
}

/// 退出登录：清除本地会话令牌（内存 + 凭据库）。
/// 保留记住的服务器与账号，方便下次登录。
#[tauri::command]
pub fn clear_session(state: State<'_, AppState>) -> HashMap<String, String> {
    clear_session_token(&state);
    session_info(state)
}

#[tauri::command]
pub fn sync_check(
    server: String,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let token = current_token(&state)?;
    let pinned = {
        let guard = state.db_lock();
        let db = guard.as_ref().ok_or("数据库未初始化")?;
        ensure_server_allowed(&app, db, &server)?
    };
    let c = client_for(&server, &pinned).with_token(&token);
    c.check()
}

/// 拉取当前登录用户的头像并转为 data URL 供 <img> 使用。
/// 头像端点要求登录（防未认证枚举全部用户头像），WebView 的 <img> 请求
/// 无法携带 JWT，因此由 Rust 侧经固定指纹客户端代理拉取。
#[tauri::command]
pub fn fetch_avatar(
    server: String,
    id: i64,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let token = current_token(&state)?;
    let pinned = {
        let guard = state.db_lock();
        let db = guard.as_ref().ok_or("数据库未初始化")?;
        ensure_server_allowed(&app, db, &server)?
    };
    let c = client_for(&server, &pinned).with_token(&token);
    let bytes = c.get_avatar(id)?;
    // 按魔数判定 MIME，不信任扩展名。
    let mime = if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        "image/png"
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        "image/jpeg"
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        "image/gif"
    } else if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        "image/webp"
    } else {
        return Err("不支持的头像格式".into());
    };
    let b64 = {
        use base64::Engine;
        base64::engine::general_purpose::STANDARD.encode(&bytes)
    };
    Ok(format!("data:{};base64,{}", mime, b64))
}

#[tauri::command]
pub fn sync_send_code(
    server: String,
    email: String,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let pinned = {
        let guard = state.db_lock();
        let db = guard.as_ref().ok_or("数据库未初始化")?;
        ensure_server_allowed(&app, db, &server)?
    };
    let c = client_for(&server, &pinned);
    c.send_register_code(&email)
}

// encrypt_entries 用 vault key 加密条目的 password/notes（端到端）。
fn encrypt_entries(list: &[Entry], vault_key: &[u8; 32]) -> Result<Vec<Entry>, String> {
    list.iter()
        .map(|e| {
            let mut e2 = e.clone();
            e2.password = crypto::encrypt_string(vault_key, &e.password)?;
            e2.notes = crypto::encrypt_string(vault_key, &e.notes)?;
            Ok(e2)
        })
        .collect()
}

// decrypt_entries 用 vault key 解密条目的 password/notes（端到端）。
fn decrypt_entries(list: &[Entry], vault_key: &[u8; 32]) -> Result<Vec<Entry>, String> {
    list.iter()
        .map(|e| {
            let mut e2 = e.clone();
            e2.password = crypto::decrypt_string(vault_key, &e.password)?;
            e2.notes = crypto::decrypt_string(vault_key, &e.notes)?;
            Ok(e2)
        })
        .collect()
}

#[tauri::command]
pub fn push_vault(
    server: String,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let token = current_token(&state)?;
    let key = require_unlock(&state)?;
    let (encrypted, pinned) = {
        let guard = state.db_lock();
        let db = guard.as_ref().ok_or("数据库未初始化")?;
        let pinned = ensure_server_allowed(&app, db, &server)?;
        let list = store::list_entries_all(db, key.as_bytes())?;
        let vault_key = read_vault_key(&state, db)?;
        (encrypt_entries(&list, &vault_key)?, pinned)
    };
    let c = client_for(&server, &pinned).with_token(&token);
    c.push(&encrypted)
}

#[tauri::command]
pub fn pull_vault(
    server: String,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<i64, String> {
    let token = current_token(&state)?;
    let key = require_unlock(&state)?;
    let pinned = {
        let guard = state.db_lock();
        let db = guard.as_ref().ok_or("数据库未初始化")?;
        ensure_server_allowed(&app, db, &server)?
    };
    let c = client_for(&server, &pinned).with_token(&token);
    let (list, pin_sync) = c.pull()?;
    let guard = state.db_lock();
    let db = guard.as_ref().ok_or("数据库未初始化")?;
    let vault_key = read_vault_key(&state, db)?;
    let decrypted = decrypt_entries(&list, &vault_key)?;
    // 置顶同步开启时采纳服务端置顶；关闭时保留本机置顶（pull 后快照恢复）。
    store::replace_entries(db, key.as_bytes(), &decrypted, pin_sync)?;
    Ok(decrypted.len() as i64)
}

#[tauri::command]
pub fn merge_vault(
    server: String,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<i64, String> {
    let token = current_token(&state)?;
    let key = require_unlock(&state)?;
    let pinned = {
        let guard = state.db_lock();
        let db = guard.as_ref().ok_or("数据库未初始化")?;
        ensure_server_allowed(&app, db, &server)?
    };
    let c = client_for(&server, &pinned).with_token(&token);
    let (server_list, pin_sync) = c.pull()?;
    let (merged, encrypted) = {
        let guard = state.db_lock();
        let db = guard.as_ref().ok_or("数据库未初始化")?;
        let vault_key = read_vault_key(&state, db)?;
        let server_decrypted = decrypt_entries(&server_list, &vault_key)?;
        let local_list = store::list_entries_all(db, key.as_bytes())?;
        let mut merged = sync::merge_entries(&local_list, &server_decrypted);
        // 置顶同步开启时以服务端置顶为准，避免 updated_at 较新的本机数据
        // 把其他设备刚做的置顶变更覆盖掉。
        if pin_sync {
            sync::apply_remote_pins(&mut merged, &server_decrypted);
        }
        store::replace_entries(db, key.as_bytes(), &merged, pin_sync)?;
        let encrypted = encrypt_entries(&merged, &vault_key)?;
        (merged.len() as i64, encrypted)
    };
    c.push(&encrypted)?;
    Ok(merged)
}

/// 读取账号级「置顶参与同步」开关（服务端按账号保存，两端共用）。
#[tauri::command]
pub fn get_pin_sync(
    server: String,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    let token = current_token(&state)?;
    let pinned = {
        let guard = state.db_lock();
        let db = guard.as_ref().ok_or("数据库未初始化")?;
        ensure_server_allowed(&app, db, &server)?
    };
    let c = client_for(&server, &pinned).with_token(&token);
    c.pin_sync()
}

/// 修改账号级「置顶参与同步」开关。
/// 开启：置顶随整库同步，在任一设备/网页端的变更会传播到所有端；
/// 关闭：桌面端置顶按设备保存在本地，网页端按账号保存在服务端。
#[tauri::command]
pub fn set_pin_sync(
    server: String,
    enabled: bool,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let token = current_token(&state)?;
    let pinned = {
        let guard = state.db_lock();
        let db = guard.as_ref().ok_or("数据库未初始化")?;
        ensure_server_allowed(&app, db, &server)?
    };
    let c = client_for(&server, &pinned).with_token(&token);
    c.set_pin_sync(enabled)
}

#[tauri::command]
pub fn scan_lan() -> Result<Vec<network::LanServer>, String> {
    network::scan_lan()
}

/// 密码重置后用旧密码恢复密码库（数据无损）：
/// 用当前（新）密码登录 → 旧密码解开旧 vault key → 新密码重新包裹上传。
/// 旧密码错误时返回 OLD_PASSWORD_WRONG，由前端提示。
///
/// **隐式契约**：本流程的可行性依赖"重置密码不改变 `kdf_salt`"这一前提——
/// 服务端 `reset_password` 只改口令哈希，`users.kdf_salt` 保持不变；因此旧密码
/// 能派生出与当初包裹 `vault_key_enc` 时**完全相同**的 master key，从而解开旧密钥。
/// 若将来有人顺手在改密流程里重新生成 `kdf_salt`，此恢复路径会静默失效（一律
/// 报 OLD_PASSWORD_WRONG），存量用户的旧密码库将永久无法找回。
/// 该契约由 `recover_vault_relies_on_stable_kdf_salt` 单测锁定，改服务端前请先看它。
#[tauri::command]
pub fn recover_vault(
    server: String,
    username: String,
    current_password: String,
    old_password: String,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    guard_destructive_sync(&state)?;
    let pinned = {
        let guard = state.db_lock();
        let db = guard.as_ref().ok_or("数据库未初始化")?;
        ensure_server_allowed(&app, db, &server)?
    };
    let c = client_for(&server, &pinned);
    let r = c.login(&username, &current_password)?;
    // 恢复流程内部完成了一次登录：沿用该会话，避免前端再登一次（PT-06）。
    store_session(&state, &r.token);
    let salt = if r.kdf_salt.is_empty() {
        username.clone()
    } else {
        r.kdf_salt.clone()
    };
    let old_master = crypto::derive_master_key(&old_password, &salt)?;
    let vault_key = crypto::decrypt_bytes(&old_master, &r.vault_key_enc)
        .map_err(|_| "OLD_PASSWORD_WRONG".to_string())?;
    let new_master = crypto::derive_master_key(&current_password, &salt)?;
    let enc = crypto::encrypt_bytes(&new_master, &vault_key)?;
    // 账号已有 vault_key_enc：按 R13-02 约定必须带当前口令才能改写解锁材料。
    c.put_vault_key(&enc, &current_password)?;

    // 持久化重新包裹后的 vault key（凭据库），恢复本地同步能力。
    let guard = state.db_lock();
    {
        let db = guard.as_ref().ok_or("数据库未初始化")?;
        // 地址写入失败必须报错（否则下次启动读不到服务器地址）。
        store::set_meta_batch(
            db,
            &[
                ("server_url".to_string(), server.clone()),
                ("server_username".to_string(), username.clone()),
            ],
        )?;
        if let Err(e) = write_vault_key(&state, db, &key_from_bytes(&vault_key)?) {
            eprintln!("[app] 写入 vault key 失败：{e}");
        }
    }
    Ok(())
}

/// 放弃旧密码库：清空服务端旧密文并重置 vault key，同时移除本地缓存的
/// vault key。之后重新登录会自动生成新 vault key（旧数据不可恢复）。
#[tauri::command]
pub fn reset_vault_remote(
    server: String,
    username: String,
    password: String,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    // 破坏性操作：清空服务端密码库并移除本地同步密钥，要求本地已解锁。
    guard_destructive_sync(&state)?;
    let pinned = {
        let guard = state.db_lock();
        let db = guard.as_ref().ok_or("数据库未初始化")?;
        ensure_server_allowed(&app, db, &server)?
    };
    let c = client_for(&server, &pinned);
    let r = c.login(&username, &password)?;
    store_session(&state, &r.token);
    c.delete_vault()?;

    let guard = state.db_lock();
    {
        let db = guard.as_ref().ok_or("数据库未初始化")?;
        // 地址写入失败必须报错；clear_vault_key 的失败同样不能吞——
        // 密钥没清掉会让"已重置"的库仍能用旧密钥解开，破坏重置语义。
        store::set_meta_batch(
            db,
            &[
                ("server_url".to_string(), server.clone()),
                ("server_username".to_string(), username.clone()),
            ],
        )?;
        clear_vault_key(&state, db)?;
    }
    Ok(())
}

// ---- 服务器信任管理（PT-02 / PT-11）----
//
// 注意：信任记录**只能**由 `ensure_server_allowed` 中的原生确认框写入。
// 渲染层没有"直接信任某服务器"的命令——这样即便渲染层被完全控制，也无法把
// 密码库发往一个用户从未在原生对话框中同意过的地址。

/// 列出已确认的服务器地址。用于界面展示与撤销。
#[tauri::command]
pub fn list_allowed_servers(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let guard = state.db_lock();
    let db = guard.as_ref().ok_or("数据库未初始化")?;
    Ok(store::list_meta_keys_with_prefix(db, "server_allow:")
        .into_iter()
        .map(|k| k.trim_start_matches("server_allow:").to_string())
        .collect())
}

/// 撤销某服务器的确认（同时清除指纹记录）：下次连接会重新弹出原生确认框。
#[tauri::command]
pub fn remove_allowed_server(server: String, state: State<'_, AppState>) -> Result<(), String> {
    let authority = sync::pinning::authority(&sync::normalize_server_url(&server));
    let guard = state.db_lock();
    let db = guard.as_ref().ok_or("数据库未初始化")?;
    store::delete_meta(db, &allow_key(&authority))?;
    store::delete_meta(db, &trust_key(&authority))
}

/// 清除某服务器的信任记录（服务器正常更换证书后，用户核对无误再重新信任）。
#[tauri::command]
pub fn forget_server_trust(server: String, state: State<'_, AppState>) -> Result<(), String> {
    let authority = sync::pinning::authority(&sync::normalize_server_url(&server));
    let guard = state.db_lock();
    let db = guard.as_ref().ok_or("数据库未初始化")?;
    store::delete_meta(db, &trust_key(&authority))
}

/// 查询某服务器当前已信任的证书指纹（无记录返回空串）。
#[tauri::command]
pub fn get_trusted_fingerprint(server: String, state: State<'_, AppState>) -> Result<String, String> {
    let authority = sync::pinning::authority(&sync::normalize_server_url(&server));
    let guard = state.db_lock();
    let db = guard.as_ref().ok_or("数据库未初始化")?;
    Ok(store::get_meta(db, &trust_key(&authority)).unwrap_or_default())
}

/// 返回当前环境实际可用的凭据库后端（"keyring" 或 "file"），供界面提示降级。
#[tauri::command]
pub fn get_secret_backend(state: State<'_, AppState>) -> String {
    state.secret.probe().as_str().to_string()
}

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Result<HashMap<String, String>, String> {
    let guard = state.db_lock();
    let db = guard.as_ref().ok_or("数据库未初始化")?;
    Ok(settings::get_settings(db))
}

#[tauri::command]
pub fn save_settings(
    autostart: bool,
    autosync: bool,
    priority: String,
    recycle: bool,
    recycle_days: i64,
    state: State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let guard = state.db_lock();
    let db = guard.as_ref().ok_or("数据库未初始化")?;
    settings::save_settings(db, autostart, autosync, &priority, recycle, recycle_days, &app)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FP_A: &str = "AA:BB";
    const FP_B: &str = "CC:DD";

    #[test]
    fn trust_action_unconfirmed_server_needs_first_use_confirmation() {
        // 未确认过的地址：无论有无指纹记录，都必须先弹原生确认框。
        assert_eq!(trust_action(false, None, FP_A), TrustAction::ConfirmFirstUse);
        assert_eq!(trust_action(false, Some(FP_A), FP_A), TrustAction::ConfirmFirstUse);
    }

    #[test]
    fn trust_action_matching_fingerprint_allows() {
        // 白名单 + 指纹都在、且一致 → 直接放行。
        assert_eq!(trust_action(true, Some(FP_A), FP_A), TrustAction::Allow);
    }

    #[test]
    fn trust_action_allowlisted_without_fingerprint_needs_confirmation() {
        // PT-11 补充：只有白名单、没有指纹记录（上一次两条 meta 写了一半），
        // 无法比对任何证书，必须按首次使用重新确认，绝不能静默放行。
        assert_eq!(trust_action(true, None, FP_A), TrustAction::ConfirmFirstUse);
    }

    #[test]
    fn trust_action_changed_fingerprint_requires_second_confirmation() {
        // 已确认的服务器换了证书：必须二次确认，不能静默继续。
        assert_eq!(trust_action(true, Some(FP_A), FP_B), TrustAction::ConfirmChanged);
    }

    #[test]
    fn gate_keys_are_scoped_by_authority() {
        // 白名单与指纹记录必须按主机（含端口）区分，避免不同服务器互相覆盖。
        assert_eq!(allow_key("192.168.1.5:5201"), "server_allow:192.168.1.5:5201");
        assert_eq!(trust_key("192.168.1.5:5201"), "server_fp:192.168.1.5:5201");
        assert_ne!(allow_key("a:1"), allow_key("a:2"));
    }

    #[test]
    fn normalize_defaults_to_https_for_lan() {
        // PT-02：应用层归一化后，内网地址一律 https（由 ensure_trusted 接管校验）。
        let url = sync::normalize_server_url("192.168.1.5");
        assert!(sync::pinning::is_https(&url), "内网地址应默认 https：{url}");
        assert_eq!(sync::pinning::authority(&url), "192.168.1.5:5201");
        // 显式 http:// 仍保留明文（知情降级）。
        let plain = sync::normalize_server_url("http://192.168.1.5:5201");
        assert!(!sync::pinning::is_https(&plain));
    }
}
