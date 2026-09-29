// 存储模块：SQLite 连接、迁移与密码条目 CRUD。
// 表结构与 Go 端完全一致，保证旧数据可直接读取。
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::crypto;

/// 一条密码记录。Password/Notes 在内存中为明文，落盘时被主密钥加密。
/// Deleted 为墓碑标记：软删除后保留 id + 时间戳用于跨端同步传播删除。
#[derive(Clone, Serialize, Deserialize)]
pub struct Entry {
    pub id: i64,
    pub sort_order: i64,
    pub title: String,
    pub username: String,
    pub password: String,
    pub url: String,
    pub category: String,
    pub notes: String,
    pub created_at: String,
    pub updated_at: String,
    pub deleted: bool,
}

/// 返回数据库文件路径：
/// 便携模式（exe 旁有 portable.flag）→ exe 同目录；安装模式 → 配置目录。
pub fn db_file_path() -> PathBuf {
    let path = if let Ok(exe) = std::env::current_exe() {
        let flag = exe.with_file_name("portable.flag");
        if flag.exists() {
            exe.with_file_name("app.db")
        } else if let Some(config_dir) = dirs::config_dir() {
            let p = config_dir.join("CryPtBox");
            let _ = std::fs::create_dir_all(&p);
            p.join("app.db")
        } else {
            PathBuf::from("app.db")
        }
    } else {
        PathBuf::from("app.db")
    };
    // 旧版本数据库名为 passbook.db：若新库不存在且旧库存在，则重命名迁移，避免升级丢数据。
    if !path.exists() {
        let old = path.with_file_name("passbook.db");
        if old.exists() {
            let _ = std::fs::rename(&old, &path);
        }
    }
    path
}

pub fn open_store(path: &PathBuf) -> Result<Connection, String> {
    let conn = Connection::open(path).map_err(|e| e.to_string())?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS entries (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            sort_order INTEGER NOT NULL DEFAULT 0,
            title TEXT NOT NULL,
            username TEXT NOT NULL DEFAULT '',
            password_enc TEXT NOT NULL DEFAULT '',
            url TEXT NOT NULL DEFAULT '',
            category TEXT NOT NULL DEFAULT '',
            notes_enc TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            deleted INTEGER NOT NULL DEFAULT 0
        );
        CREATE TABLE IF NOT EXISTS meta (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );",
    )
    .map_err(|e| e.to_string())?;

    // 兼容旧库：为 entries 表补充 deleted 墓碑列（若缺失）。
    let has_deleted = {
        let mut stmt = conn
            .prepare("PRAGMA table_info(entries)")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(1))
            .map_err(|e| e.to_string())?;
        let mut found = false;
        for name in rows.flatten() {
            if name == "deleted" {
                found = true;
                break;
            }
        }
        found
    };
    if !has_deleted {
        conn.execute(
            "ALTER TABLE entries ADD COLUMN deleted INTEGER NOT NULL DEFAULT 0",
            [],
        )
        .map_err(|e| e.to_string())?;
    }

    // 兼容旧库：为 entries 表补充 sort_order 序号列（若缺失），并按 id 顺序初始化旧数据。
    let has_sort_order = {
        let mut stmt = conn
            .prepare("PRAGMA table_info(entries)")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(1))
            .map_err(|e| e.to_string())?;
        let mut found = false;
        for name in rows.flatten() {
            if name == "sort_order" {
                found = true;
                break;
            }
        }
        found
    };
    if !has_sort_order {
        conn.execute(
            "ALTER TABLE entries ADD COLUMN sort_order INTEGER NOT NULL DEFAULT 0",
            [],
        )
        .map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare("SELECT id FROM entries WHERE deleted = 0 ORDER BY id")
            .map_err(|e| e.to_string())?;
        let ids: Vec<i64> = stmt
            .query_map([], |row| row.get::<_, i64>(0))
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        for (i, id) in ids.iter().enumerate() {
            conn.execute(
                "UPDATE entries SET sort_order = ? WHERE id = ?",
                params![(i as i64) + 1, id],
            )
            .map_err(|e| e.to_string())?;
        }
    }
    Ok(conn)
}

pub fn get_meta(db: &Connection, key: &str) -> Option<String> {
    db.query_row("SELECT value FROM meta WHERE key = ?", [key], |row| row.get(0))
        .ok()
}

pub fn set_meta(db: &Connection, key: &str, value: &str) -> Result<(), String> {
    db.execute(
        "INSERT OR REPLACE INTO meta (key, value) VALUES (?, ?)",
        params![key, value],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn list_entries(db: &Connection, key: &[u8; 32]) -> Result<Vec<Entry>, String> {
    let mut stmt = db
        .prepare(
            "SELECT id, sort_order, title, username, password_enc, url, category, notes_enc, created_at, updated_at \
             FROM entries WHERE deleted = 0 ORDER BY sort_order ASC, id ASC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, String>(8)?,
                row.get::<_, String>(9)?,
            ))
        })
        .map_err(|e| e.to_string())?;
    let mut list = Vec::new();
    for r in rows {
        let (id, sort_order, title, username, pw_enc, url, category, notes_enc, created_at, updated_at) =
            r.map_err(|e| e.to_string())?;
        let password = crypto::decrypt_string(key, &pw_enc).unwrap_or_default();
        let notes = crypto::decrypt_string(key, &notes_enc).unwrap_or_default();
        list.push(Entry {
            id,
            sort_order,
            title,
            username,
            password,
            url,
            category,
            notes,
            created_at,
            updated_at,
            deleted: false,
        });
    }
    Ok(list)
}

pub fn list_entries_all(db: &Connection, key: &[u8; 32]) -> Result<Vec<Entry>, String> {
    let mut stmt = db
        .prepare(
            "SELECT id, sort_order, title, username, password_enc, url, category, notes_enc, created_at, updated_at, deleted \
             FROM entries ORDER BY sort_order ASC, id ASC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, String>(8)?,
                row.get::<_, String>(9)?,
                row.get::<_, i64>(10)?,
            ))
        })
        .map_err(|e| e.to_string())?;
    let mut list = Vec::new();
    for r in rows {
        let (id, sort_order, title, username, pw_enc, url, category, notes_enc, created_at, updated_at, deleted) =
            r.map_err(|e| e.to_string())?;
        let password = crypto::decrypt_string(key, &pw_enc).unwrap_or_default();
        let notes = crypto::decrypt_string(key, &notes_enc).unwrap_or_default();
        list.push(Entry {
            id,
            sort_order,
            title,
            username,
            password,
            url,
            category,
            notes,
            created_at,
            updated_at,
            deleted: deleted != 0,
        });
    }
    Ok(list)
}

pub fn save_entry(db: &Connection, key: &[u8; 32], mut entry: Entry) -> Result<Entry, String> {
    let now = chrono::Utc::now().to_rfc3339();
    let pw_enc = crypto::encrypt_string(key, &entry.password)?;
    let notes_enc = crypto::encrypt_string(key, &entry.notes)?;
    if entry.id == 0 {
        // 新条目追加到末尾（序号为当前最大序号 + 1）
        let max_sort: i64 = db
            .query_row(
                "SELECT COALESCE(MAX(sort_order), 0) FROM entries WHERE deleted = 0",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);
        entry.sort_order = max_sort + 1;
        entry.created_at = now.clone();
        entry.updated_at = now;
        db.execute(
            "INSERT INTO entries (sort_order, title, username, password_enc, url, category, notes_enc, created_at, updated_at) \
             VALUES (?,?,?,?,?,?,?,?,?)",
            params![
                entry.sort_order,
                entry.title,
                entry.username,
                pw_enc,
                entry.url,
                entry.category,
                notes_enc,
                entry.created_at,
                entry.updated_at
            ],
        )
        .map_err(|e| e.to_string())?;
        entry.id = db.last_insert_rowid();
    } else {
        entry.updated_at = now;
        db.execute(
            "UPDATE entries SET title=?, username=?, password_enc=?, url=?, category=?, notes_enc=?, updated_at=? WHERE id=?",
            params![
                entry.title,
                entry.username,
                pw_enc,
                entry.url,
                entry.category,
                notes_enc,
                entry.updated_at,
                entry.id
            ],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(entry)
}

/// 删除后重排：把序号大于被删序号、且未删除的条目前移一位，保持序号连续。
fn renumber_after_delete(db: &Connection, sort_order: i64) -> Result<(), String> {
    if sort_order <= 0 {
        return Ok(());
    }
    db.execute(
        "UPDATE entries SET sort_order = sort_order - 1 WHERE sort_order > ? AND deleted = 0",
        params![sort_order],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// 软删除（走回收站）：打墓碑标记并记录删除时间，保留密文以便后续恢复。
pub fn soft_delete_entry(db: &Connection, id: i64) -> Result<(), String> {
    let now = chrono::Utc::now().to_rfc3339();
    let sort_order: i64 = db
        .query_row("SELECT sort_order FROM entries WHERE id = ?", [id], |r| r.get(0))
        .unwrap_or(0);
    db.execute(
        "UPDATE entries SET deleted = 1, sort_order = 0, updated_at = ? WHERE id = ?",
        params![now, id],
    )
    .map_err(|e| e.to_string())?;
    renumber_after_delete(db, sort_order)
}

/// 彻底删除：从数据库物理删除（不走回收站，或从回收站永久删除），返回删除条数。
pub fn hard_delete_entry(db: &Connection, id: i64) -> Result<usize, String> {
    let sort_order: i64 = db
        .query_row("SELECT sort_order FROM entries WHERE id = ?", [id], |r| r.get(0))
        .unwrap_or(0);
    let n = db
        .execute("DELETE FROM entries WHERE id = ?", params![id])
        .map_err(|e| e.to_string())?;
    renumber_after_delete(db, sort_order)?;
    Ok(n)
}

/// 恢复回收站条目：清除墓碑标记并追加到末尾（重新分配序号）。
pub fn restore_entry(db: &Connection, id: i64) -> Result<(), String> {
    let now = chrono::Utc::now().to_rfc3339();
    let max_sort: i64 = db
        .query_row(
            "SELECT COALESCE(MAX(sort_order), 0) FROM entries WHERE deleted = 0",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    db.execute(
        "UPDATE entries SET deleted = 0, sort_order = ?, updated_at = ? WHERE id = ?",
        params![max_sort + 1, now, id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// 清空回收站：物理删除所有墓碑条目，返回删除条数。
pub fn empty_trash(db: &Connection) -> Result<usize, String> {
    db.execute("DELETE FROM entries WHERE deleted = 1", []).map_err(|e| e.to_string())
}

/// 自动清理：物理删除超过保留天数的墓碑条目，返回删除条数。
pub fn purge_expired(db: &Connection, days: i64) -> Result<usize, String> {
    if days <= 0 {
        return Ok(0);
    }
    let threshold = (chrono::Utc::now() - chrono::Duration::days(days)).to_rfc3339();
    db.execute(
        "DELETE FROM entries WHERE deleted = 1 AND updated_at < ?",
        params![threshold],
    )
    .map_err(|e| e.to_string())
}

/// 列出回收站中的墓碑条目（含已解密内容，供回收站界面展示与恢复）。
pub fn list_trash(db: &Connection, key: &[u8; 32]) -> Result<Vec<Entry>, String> {
    let mut stmt = db
        .prepare(
            "SELECT id, sort_order, title, username, password_enc, url, category, notes_enc, created_at, updated_at \
             FROM entries WHERE deleted = 1 ORDER BY updated_at DESC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, String>(8)?,
                row.get::<_, String>(9)?,
            ))
        })
        .map_err(|e| e.to_string())?;
    let mut list = Vec::new();
    for r in rows {
        let (id, sort_order, title, username, pw_enc, url, category, notes_enc, created_at, updated_at) =
            r.map_err(|e| e.to_string())?;
        let password = crypto::decrypt_string(key, &pw_enc).unwrap_or_default();
        let notes = crypto::decrypt_string(key, &notes_enc).unwrap_or_default();
        list.push(Entry {
            id,
            sort_order,
            title,
            username,
            password,
            url,
            category,
            notes,
            created_at,
            updated_at,
            deleted: true,
        });
    }
    Ok(list)
}

/// 全量替换（同步合并用）：清空后按列表重建，保留客户端上传的 id。
pub fn replace_entries(db: &Connection, key: &[u8; 32], list: &[Entry]) -> Result<(), String> {
    let tx = db.unchecked_transaction().map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM entries", []).map_err(|e| e.to_string())?;
    for e in list {
        let pw_enc = crypto::encrypt_string(key, &e.password)?;
        let notes_enc = crypto::encrypt_string(key, &e.notes)?;
        let deleted = if e.deleted { 1 } else { 0 };
        tx.execute(
            "INSERT INTO entries (id, sort_order, title, username, password_enc, url, category, notes_enc, created_at, updated_at, deleted) \
             VALUES (?,?,?,?,?,?,?,?,?,?,?)",
            params![
                e.id, e.sort_order, e.title, e.username, pw_enc, e.url, e.category, notes_enc, e.created_at,
                e.updated_at, deleted
            ],
        )
        .map_err(|e| e.to_string())?;
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}
