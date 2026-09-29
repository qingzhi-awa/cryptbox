// 应用核心：Tauri 命令注册与运行时状态管理。
use std::collections::HashMap;
use std::sync::Mutex;

use rusqlite::Connection;
use tauri::Manager;
use tauri::State;

use crate::crypto;
use crate::import_export;
use crate::network;
use crate::settings;
use crate::store::{self, Entry};
use crate::sync;

const VERIFIER_PLAIN: &str = "passbook-verifier-v1";

pub struct AppState {
    pub db: Mutex<Option<Connection>>,
    pub key: Mutex<Option<[u8; 32]>>,
}

pub fn init(app: &tauri::AppHandle) -> Result<(), String> {
    let db_path = store::db_file_path();
    let conn = store::open_store(&db_path)?;
    app.manage(AppState {
        db: Mutex::new(Some(conn)),
        key: Mutex::new(None),
    });
    Ok(())
}

fn require_unlock(state: &AppState) -> Result<[u8; 32], String> {
    let guard = state.key.lock().unwrap();
    (*guard).ok_or_else(|| "未解锁".to_string())
}

// ---- 主密码 / 解锁 ----

#[tauri::command]
pub fn init_db(state: State<'_, AppState>) -> Result<bool, String> {
    let guard = state.db.lock().unwrap();
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
    let guard = state.db.lock().unwrap();
    let db = guard.as_ref().ok_or("数据库未初始化")?;
    use base64::{engine::general_purpose, Engine};
    store::set_meta(db, "salt", &general_purpose::STANDARD.encode(&salt))?;
    store::set_meta(db, "verifier", &verifier)?;
    *state.key.lock().unwrap() = Some(key);
    Ok(true)
}

#[tauri::command]
pub fn unlock(password: String, state: State<'_, AppState>) -> Result<bool, String> {
    let salt_b64 = {
        let guard = state.db.lock().unwrap();
        let db = guard.as_ref().ok_or("数据库未初始化")?;
        store::get_meta(db, "salt").ok_or("尚未初始化")?
    };
    use base64::{engine::general_purpose, Engine};
    let salt = general_purpose::STANDARD
        .decode(&salt_b64)
        .map_err(|e| e.to_string())?;
    let key = crypto::derive_key(&password, &salt)?;
    let verifier = {
        let guard = state.db.lock().unwrap();
        let db = guard.as_ref().ok_or("数据库未初始化")?;
        store::get_meta(db, "verifier").unwrap_or_default()
    };
    let plain = crypto::decrypt_string(&key, &verifier).map_err(|e| e.to_string())?;
    if plain != VERIFIER_PLAIN {
        return Ok(false);
    }
    *state.key.lock().unwrap() = Some(key);
    Ok(true)
}

#[tauri::command]
pub fn lock(state: State<'_, AppState>) {
    *state.key.lock().unwrap() = None;
}

#[tauri::command]
pub fn is_unlocked(state: State<'_, AppState>) -> bool {
    state.key.lock().unwrap().is_some()
}

// ---- 密码条目 ----

#[tauri::command]
pub fn list_entries(state: State<'_, AppState>) -> Result<Vec<Entry>, String> {
    let key = require_unlock(&state)?;
    let guard = state.db.lock().unwrap();
    let db = guard.as_ref().ok_or("数据库未初始化")?;
    store::list_entries(db, &key)
}

#[tauri::command]
pub fn save_entry(entry: Entry, state: State<'_, AppState>) -> Result<Entry, String> {
    let key = require_unlock(&state)?;
    let guard = state.db.lock().unwrap();
    let db = guard.as_ref().ok_or("数据库未初始化")?;
    store::save_entry(db, &key, entry)
}

#[tauri::command]
pub fn delete_entry(id: i64, soft: bool, state: State<'_, AppState>) -> Result<(), String> {
    let _key = require_unlock(&state)?;
    let guard = state.db.lock().unwrap();
    let db = guard.as_ref().ok_or("数据库未初始化")?;
    if soft {
        store::soft_delete_entry(db, id)
    } else {
        store::hard_delete_entry(db, id).map(|_| ())
    }
}

#[tauri::command]
pub fn list_trash(state: State<'_, AppState>) -> Result<Vec<Entry>, String> {
    let key = require_unlock(&state)?;
    let guard = state.db.lock().unwrap();
    let db = guard.as_ref().ok_or("数据库未初始化")?;
    // 读取回收站前先按保留天数清理过期条目。
    let days = crate::store::get_meta(db, "set_recycle_days")
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(30);
    let _ = store::purge_expired(db, days);
    store::list_trash(db, &key)
}

#[tauri::command]
pub fn restore_entry(id: i64, state: State<'_, AppState>) -> Result<(), String> {
    let _key = require_unlock(&state)?;
    let guard = state.db.lock().unwrap();
    let db = guard.as_ref().ok_or("数据库未初始化")?;
    store::restore_entry(db, id)
}

#[tauri::command]
pub fn purge_entry(id: i64, state: State<'_, AppState>) -> Result<(), String> {
    let _key = require_unlock(&state)?;
    let guard = state.db.lock().unwrap();
    let db = guard.as_ref().ok_or("数据库未初始化")?;
    store::hard_delete_entry(db, id).map(|_| ())
}

#[tauri::command]
pub fn empty_trash(state: State<'_, AppState>) -> Result<usize, String> {
    let _key = require_unlock(&state)?;
    let guard = state.db.lock().unwrap();
    let db = guard.as_ref().ok_or("数据库未初始化")?;
    store::empty_trash(db)
}

#[tauri::command]
pub fn export_txt(path: String, state: State<'_, AppState>) -> Result<i64, String> {
    let key = require_unlock(&state)?;
    let guard = state.db.lock().unwrap();
    let db = guard.as_ref().ok_or("数据库未初始化")?;
    let list = store::list_entries(db, &key)?;
    import_export::export_txt(&path, &list)
}

#[tauri::command]
pub fn export_csv(path: String, state: State<'_, AppState>) -> Result<i64, String> {
    let key = require_unlock(&state)?;
    let guard = state.db.lock().unwrap();
    let db = guard.as_ref().ok_or("数据库未初始化")?;
    let list = store::list_entries(db, &key)?;
    import_export::export_csv(&path, &list)
}

#[tauri::command]
pub fn import_csv(path: String, state: State<'_, AppState>) -> Result<i64, String> {
    let key = require_unlock(&state)?;
    let entries = import_export::parse_csv_file(&path)?;
    let guard = state.db.lock().unwrap();
    let db = guard.as_ref().ok_or("数据库未初始化")?;
    let mut count = 0i64;
    for e in entries {
        store::save_entry(db, &key, e)?;
        count += 1;
    }
    Ok(count)
}

#[tauri::command]
pub fn import_txt(path: String, state: State<'_, AppState>) -> Result<i64, String> {
    let key = require_unlock(&state)?;
    let entries = import_export::parse_txt_file(&path)?;
    let guard = state.db.lock().unwrap();
    let db = guard.as_ref().ok_or("数据库未初始化")?;
    let mut count = 0i64;
    for e in entries {
        store::save_entry(db, &key, e)?;
        count += 1;
    }
    Ok(count)
}

#[tauri::command]
pub fn save_text_file(path: String, content: String) -> Result<(), String> {
    std::fs::write(&path, content).map_err(|e| e.to_string())
}

// ---- 同步 ----

#[tauri::command]
pub fn get_server_config(state: State<'_, AppState>) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let guard = state.db.lock().unwrap();
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
    state: State<'_, AppState>,
) -> Result<HashMap<String, String>, String> {
    let c = sync::SyncClient::new(&server);
    let r = c.register(&username, &password, &email, &code)?;
    let guard = state.db.lock().unwrap();
    if let Some(db) = guard.as_ref() {
        let _ = store::set_meta(db, "server_url", &server);
        let _ = store::set_meta(db, "server_username", &username);
    }
    let mut map = HashMap::new();
    map.insert("token".into(), r.token);
    map.insert("avatar".into(), r.avatar);
    Ok(map)
}

#[tauri::command]
pub fn sync_login(
    server: String,
    username: String,
    password: String,
    state: State<'_, AppState>,
) -> Result<HashMap<String, String>, String> {
    let c = sync::SyncClient::new(&server);
    let r = c.login(&username, &password)?;
    let guard = state.db.lock().unwrap();
    if let Some(db) = guard.as_ref() {
        let _ = store::set_meta(db, "server_url", &server);
        let _ = store::set_meta(db, "server_username", &username);
    }
    let mut map = HashMap::new();
    map.insert("token".into(), r.token);
    map.insert("avatar".into(), r.avatar);
    Ok(map)
}

#[tauri::command]
pub fn sync_check(server: String, token: String) -> Result<(), String> {
    let c = sync::SyncClient::new(&server).with_token(&token);
    c.check()
}

#[tauri::command]
pub fn sync_send_code(server: String, email: String) -> Result<(), String> {
    let c = sync::SyncClient::new(&server);
    c.send_register_code(&email)
}

#[tauri::command]
pub fn push_vault(server: String, token: String, state: State<'_, AppState>) -> Result<(), String> {
    let key = require_unlock(&state)?;
    let guard = state.db.lock().unwrap();
    let db = guard.as_ref().ok_or("数据库未初始化")?;
    let list = store::list_entries_all(db, &key)?;
    let c = sync::SyncClient::new(&server).with_token(&token);
    c.push(&list)
}

#[tauri::command]
pub fn pull_vault(
    server: String,
    token: String,
    state: State<'_, AppState>,
) -> Result<i64, String> {
    let key = require_unlock(&state)?;
    let c = sync::SyncClient::new(&server).with_token(&token);
    let list = c.pull()?;
    let guard = state.db.lock().unwrap();
    let db = guard.as_ref().ok_or("数据库未初始化")?;
    store::replace_entries(db, &key, &list)?;
    Ok(list.len() as i64)
}

#[tauri::command]
pub fn merge_vault(
    server: String,
    token: String,
    state: State<'_, AppState>,
) -> Result<i64, String> {
    let key = require_unlock(&state)?;
    let c = sync::SyncClient::new(&server).with_token(&token);
    let server_list = c.pull()?;
    let guard = state.db.lock().unwrap();
    let db = guard.as_ref().ok_or("数据库未初始化")?;
    let local_list = store::list_entries_all(db, &key)?;
    let merged = sync::merge_entries(&local_list, &server_list);
    store::replace_entries(db, &key, &merged)?;
    c.push(&merged)?;
    Ok(merged.len() as i64)
}

#[tauri::command]
pub fn scan_lan() -> Result<Vec<String>, String> {
    network::scan_lan()
}

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Result<HashMap<String, String>, String> {
    let guard = state.db.lock().unwrap();
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
    let guard = state.db.lock().unwrap();
    let db = guard.as_ref().ok_or("数据库未初始化")?;
    settings::save_settings(db, autostart, autosync, &priority, recycle, recycle_days, &app)
}
