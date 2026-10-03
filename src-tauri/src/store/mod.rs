// 存储模块：SQLite 连接、迁移与密码条目 CRUD。
// 表结构与 Go 端完全一致，保证旧数据可直接读取。
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::crypto;

/// 一条密码记录。Password/Notes 在内存中为明文，落盘时被主密钥加密。
/// Deleted 为墓碑标记：软删除后保留 id + 时间戳用于跨端同步传播删除。
///
/// `uuid` 为全局唯一同步标识（PT-04）：新建条目时生成随机 v4；旧数据为空，
/// 同步时由服务端按 (user_id, id) 分配确定性 v5 后回传。合并优先按 uuid。
#[derive(Clone, Serialize, Deserialize)]
pub struct Entry {
    pub id: i64,
    #[serde(default)]
    pub uuid: String,
    /// 置顶标记：仅影响本设备的列表显示顺序，不参与跨端同步（见 set_pinned）。
    #[serde(default)]
    pub pinned: bool,
    #[serde(default)]
    pub sort_order: i64,
    pub title: String,
    pub username: String,
    pub password: String,
    pub url: String,
    pub category: String,
    pub notes: String,
    // 以下字段由前端「新增」表单提交时可缺省：时间戳与序号由存储层生成，
    // 墓碑标记默认为未删除。缺省时按默认值填充，避免整条命令反序列化失败。
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
    #[serde(default)]
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

/// 返回数据目录（数据库所在目录），用于存放降级密钥文件等伴随数据。
pub fn data_dir() -> PathBuf {
    db_file_path()
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn open_store(path: &PathBuf) -> Result<Connection, String> {
    let conn = Connection::open(path).map_err(|e| e.to_string())?;
    // PT-14：类 Unix 下把数据库文件收紧为 0600（Windows 由安装目录 ACL 保护）。
    harden_db_perm(path);
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS entries (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            uuid TEXT NOT NULL DEFAULT '',
            pinned INTEGER NOT NULL DEFAULT 0,
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
    if !column_exists(&conn, "deleted")? {
        conn.execute(
            "ALTER TABLE entries ADD COLUMN deleted INTEGER NOT NULL DEFAULT 0",
            [],
        )
        .map_err(|e| e.to_string())?;
    }

    // 兼容旧库：为 entries 表补充 uuid 同步标识列（若缺失，PT-04）。
    if !column_exists(&conn, "uuid")? {
        conn.execute(
            "ALTER TABLE entries ADD COLUMN uuid TEXT NOT NULL DEFAULT ''",
            [],
        )
        .map_err(|e| e.to_string())?;
    }

    // 兼容旧库：为 entries 表补充 pinned 置顶列（若缺失）。
    if !column_exists(&conn, "pinned")? {
        conn.execute(
            "ALTER TABLE entries ADD COLUMN pinned INTEGER NOT NULL DEFAULT 0",
            [],
        )
        .map_err(|e| e.to_string())?;
    }

    // 兼容旧库：为 entries 表补充 sort_order 序号列（若缺失），并按 id 顺序初始化旧数据。
    let has_sort_order = column_exists(&conn, "sort_order")?;
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

/// 把数据库文件（含 WAL/SHM）权限收紧为 0600（仅类 Unix 生效）。
fn harden_db_perm(path: &std::path::Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for suffix in ["", "-wal", "-shm"] {
            let p = if suffix.is_empty() {
                path.to_path_buf()
            } else {
                std::path::PathBuf::from(format!("{}{}", path.to_string_lossy(), suffix))
            };
            let Ok(meta) = std::fs::metadata(&p) else { continue };
            if meta.permissions().mode() & 0o077 != 0 {
                let _ = std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o600));
            }
        }
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
}

/// 判断 entries 表是否已存在某列（用于旧库补列）。
fn column_exists(conn: &Connection, name: &str) -> Result<bool, String> {
    let mut stmt = conn
        .prepare("PRAGMA table_info(entries)")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|e| e.to_string())?;
    for n in rows.flatten() {
        if n == name {
            return Ok(true);
        }
    }
    Ok(false)
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

/// 列出所有以指定前缀开头的 meta 键（用于枚举已确认的服务器地址等）。
pub fn list_meta_keys_with_prefix(db: &Connection, prefix: &str) -> Vec<String> {
    let mut out = Vec::new();
    // 用 Rust 侧前缀匹配而非 SQL LIKE：避免键名中的 `_` 被当作通配符。
    let Ok(mut stmt) = db.prepare("SELECT key FROM meta ORDER BY key") else {
        return out;
    };
    if let Ok(rows) = stmt.query_map([], |r| r.get::<_, String>(0)) {
        for k in rows.flatten() {
            if k.starts_with(prefix) {
                out.push(k);
            }
        }
    }
    out
}

/// 删除一条 meta（如密码重置后清掉本地缓存的 vault_key）。
pub fn delete_meta(db: &Connection, key: &str) -> Result<(), String> {
    db.execute("DELETE FROM meta WHERE key = ?", params![key])
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn list_entries(db: &Connection, key: &[u8; 32]) -> Result<Vec<Entry>, String> {
    let mut stmt = db
        .prepare(
            "SELECT id, uuid, sort_order, title, username, password_enc, url, category, notes_enc, created_at, updated_at, pinned \
             FROM entries WHERE deleted = 0 ORDER BY pinned DESC, sort_order ASC, id ASC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, String>(8)?,
                row.get::<_, String>(9)?,
                row.get::<_, String>(10)?,
                row.get::<_, i64>(11)?,
            ))
        })
        .map_err(|e| e.to_string())?;
    let mut list = Vec::new();
    for r in rows {
        let (id, uuid, sort_order, title, username, pw_enc, url, category, notes_enc, created_at, updated_at, pinned) =
            r.map_err(|e| e.to_string())?;
        let password = crypto::decrypt_string(key, &pw_enc).unwrap_or_default();
        let notes = crypto::decrypt_string(key, &notes_enc).unwrap_or_default();
        list.push(Entry {
            id,
            uuid,
            pinned: pinned != 0,
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
            "SELECT id, uuid, sort_order, title, username, password_enc, url, category, notes_enc, created_at, updated_at, deleted, pinned \
             FROM entries ORDER BY pinned DESC, sort_order ASC, id ASC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, String>(8)?,
                row.get::<_, String>(9)?,
                row.get::<_, String>(10)?,
                row.get::<_, i64>(11)?,
                row.get::<_, i64>(12)?,
            ))
        })
        .map_err(|e| e.to_string())?;
    let mut list = Vec::new();
    for r in rows {
        let (id, uuid, sort_order, title, username, pw_enc, url, category, notes_enc, created_at, updated_at, deleted, pinned) =
            r.map_err(|e| e.to_string())?;
        let password = crypto::decrypt_string(key, &pw_enc).unwrap_or_default();
        let notes = crypto::decrypt_string(key, &notes_enc).unwrap_or_default();
        list.push(Entry {
            id,
            uuid,
            pinned: pinned != 0,
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
        // 新条目就地生成全局唯一同步标识（PT-04）。
        if entry.uuid.is_empty() {
            entry.uuid = crypto::new_uuid_v4();
        }
        db.execute(
            "INSERT INTO entries (uuid, pinned, sort_order, title, username, password_enc, url, category, notes_enc, created_at, updated_at) \
             VALUES (?,?,?,?,?,?,?,?,?,?,?)",
            params![
                entry.uuid,
                if entry.pinned { 1 } else { 0 },
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
        // uuid 一旦分配即为条目的稳定身份：入参缺失时保留库中已有值，
        // 避免前端只提交表单字段时把同步标识抹掉。
        if entry.uuid.is_empty() {
            entry.uuid = db
                .query_row("SELECT uuid FROM entries WHERE id = ?", [entry.id], |r| {
                    r.get::<_, String>(0)
                })
                .unwrap_or_default();
        }
        db.execute(
            "UPDATE entries SET uuid=?, title=?, username=?, password_enc=?, url=?, category=?, notes_enc=?, updated_at=? WHERE id=?",
            params![
                entry.uuid,
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
            "SELECT id, uuid, sort_order, title, username, password_enc, url, category, notes_enc, created_at, updated_at, pinned \
             FROM entries WHERE deleted = 1 ORDER BY updated_at DESC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, String>(8)?,
                row.get::<_, String>(9)?,
                row.get::<_, String>(10)?,
                row.get::<_, i64>(11)?,
            ))
        })
        .map_err(|e| e.to_string())?;
    let mut list = Vec::new();
    for r in rows {
        let (id, uuid, sort_order, title, username, pw_enc, url, category, notes_enc, created_at, updated_at, pinned) =
            r.map_err(|e| e.to_string())?;
        let password = crypto::decrypt_string(key, &pw_enc).unwrap_or_default();
        let notes = crypto::decrypt_string(key, &notes_enc).unwrap_or_default();
        list.push(Entry {
            id,
            uuid,
            pinned: pinned != 0,
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
///
/// uuid 按入参原样保留（不在本地另生成）：服务端下发的标识即该条目的身份，
/// 空值表示"尚未分配"，会在下一次推送时由服务端按 (user_id, id) 确定性补齐。
///
/// 置顶是**本设备**的视图偏好、不参与跨端同步，故入库前先按 uuid（回退 id）
/// 记录现有置顶状态，重建后原样恢复——否则每次拉取/合并都会把用户的置顶清空。
pub fn replace_entries(
    db: &Connection,
    key: &[u8; 32],
    list: &[Entry],
    adopt_server_pins: bool,
) -> Result<(), String> {
    use std::collections::HashSet;
    // 置顶语义由账号级开关决定：
    //   adopt_server_pins = true（同步开启）：以服务端下发的 pinned 为准；
    //   adopt_server_pins = false（同步关闭）：服务端载荷可能携带网页端按账号的
    //   置顶状态（或旧协议根本没有该字段），此时用本地快照恢复本机置顶。
    let mut pinned_uuids: HashSet<String> = HashSet::new();
    let mut pinned_ids: HashSet<i64> = HashSet::new();
    if !adopt_server_pins {
        if let Ok(mut stmt) = db.prepare("SELECT id, uuid FROM entries WHERE pinned = 1") {
            if let Ok(rows) = stmt.query_map([], |r| {
                Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
            }) {
                for (id, uuid) in rows.flatten() {
                    if !uuid.is_empty() {
                        pinned_uuids.insert(uuid);
                    }
                    pinned_ids.insert(id);
                }
            }
        }
    }

    let tx = db.unchecked_transaction().map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM entries", []).map_err(|e| e.to_string())?;
    for e in list {
        let pw_enc = crypto::encrypt_string(key, &e.password)?;
        let notes_enc = crypto::encrypt_string(key, &e.notes)?;
        let deleted = if e.deleted { 1 } else { 0 };
        let mut pinned = if e.pinned { 1 } else { 0 };
        if !adopt_server_pins
            && ((e.uuid.is_empty() && pinned_ids.contains(&e.id))
                || (!e.uuid.is_empty() && pinned_uuids.contains(&e.uuid)))
        {
            pinned = 1;
        }
        tx.execute(
            "INSERT INTO entries (id, uuid, pinned, sort_order, title, username, password_enc, url, category, notes_enc, created_at, updated_at, deleted) \
             VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?)",
            params![
                e.id, e.uuid, pinned, e.sort_order, e.title, e.username, pw_enc, e.url, e.category, notes_enc,
                e.created_at, e.updated_at, deleted
            ],
        )
        .map_err(|e| e.to_string())?;
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

/// 设置/取消置顶。仅影响本设备显示顺序，不更新 updated_at（避免无谓的同步写入）。
pub fn set_pinned(db: &Connection, id: i64, pinned: bool) -> Result<(), String> {
    db.execute(
        "UPDATE entries SET pinned = ? WHERE id = ?",
        params![if pinned { 1 } else { 0 }, id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// 按给定的条目顺序重排。
///
/// 置顶与普通条目分属两个序号序列（置顶组始终排在前面），因此同一份「显示顺序」
/// 在两端都稳定；序号变化会更新 updated_at，使新顺序能随同步传播到其他设备。
pub fn reorder_entries(db: &Connection, ordered_ids: &[i64]) -> Result<(), String> {
    let now = chrono::Utc::now().to_rfc3339();
    let tx = db.unchecked_transaction().map_err(|e| e.to_string())?;
    let mut pinned_seq: i64 = 0;
    let mut normal_seq: i64 = 0;
    for id in ordered_ids {
        let pinned: i64 = tx
            .query_row(
                "SELECT pinned FROM entries WHERE id = ? AND deleted = 0",
                [id],
                |r| r.get(0),
            )
            .unwrap_or(0);
        let next = if pinned != 0 {
            pinned_seq += 1;
            pinned_seq
        } else {
            normal_seq += 1;
            normal_seq
        };
        tx.execute(
            "UPDATE entries SET sort_order = ?, updated_at = ? WHERE id = ? AND deleted = 0",
            params![next, now, id],
        )
        .map_err(|e| e.to_string())?;
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_db(tag: &str) -> PathBuf {
        let mut d = std::env::temp_dir();
        d.push(format!("cryptbox-store-test-{}-{}", tag, std::process::id()));
        let _ = std::fs::remove_file(&d);
        d
    }

    fn new_entry(title: &str) -> Entry {
        Entry {
            id: 0,
            uuid: String::new(),
            pinned: false,
            sort_order: 0,
            title: title.to_string(),
            username: "u".into(),
            password: "p".into(),
            url: String::new(),
            category: String::new(),
            notes: String::new(),
            created_at: String::new(),
            updated_at: String::new(),
            deleted: false,
        }
    }

    fn titles(list: &[Entry]) -> Vec<String> {
        list.iter().map(|e| e.title.clone()).collect()
    }

    #[test]
    fn partial_form_payload_deserializes() {
        // 前端「新增」表单只提交 标题/用户名/口令/网址/分类/备注：
        // 序号、时间戳、墓碑标记由存储层补齐，不得因缺字段导致整条命令失败。
        let json = r#"{"id":0,"title":"t","username":"u","password":"p","url":"","category":"","notes":""}"#;
        let e: Entry = serde_json::from_str(json).expect("部分字段应可反序列化");
        assert_eq!(e.id, 0);
        assert_eq!(e.sort_order, 0);
        assert!(!e.pinned);
        assert!(!e.deleted);
        assert!(e.uuid.is_empty());
    }

    #[test]
    fn legacy_shape_without_defaults_rejected_partial_payload() {
        // 留档：修复前 Entry 的 sort_order/created_at/updated_at/deleted 没有
        // #[serde(default)]，而前端「新增」表单恰好不提交这四个字段——serde 会
        // 直接报 missing field，导致整条 save_entry 命令失败。此处复刻旧结构以固化结论。
        #[derive(serde::Deserialize)]
        #[allow(dead_code)]
        struct LegacyEntry {
            id: i64,
            uuid: String,
            sort_order: i64,
            title: String,
            username: String,
            password: String,
            url: String,
            category: String,
            notes: String,
            created_at: String,
            updated_at: String,
            deleted: bool,
        }
        let json = r#"{"id":0,"title":"t","username":"u","password":"p","url":"","category":"","notes":""}"#;
        assert!(
            serde_json::from_str::<LegacyEntry>(json).is_err(),
            "旧结构（无默认值）确实会因缺字段拒绝前端新增表单的载荷"
        );
    }

    #[test]
    fn pin_keeps_entry_first_and_survives_pull() {
        let path = temp_db("pin");
        let conn = open_store(&path).unwrap();
        let key = [7u8; 32];
        let _a = save_entry(&conn, &key, new_entry("a")).unwrap();
        let b = save_entry(&conn, &key, new_entry("b")).unwrap();
        let _c = save_entry(&conn, &key, new_entry("c")).unwrap();

        assert_eq!(titles(&list_entries(&conn, &key).unwrap()), ["a", "b", "c"]);

        set_pinned(&conn, b.id, true).unwrap();
        assert_eq!(titles(&list_entries(&conn, &key).unwrap()), ["b", "a", "c"]);

        // 模拟同步拉取（置顶同步关闭）：服务端不携带 pinned（全部为 false），
        // 本地置顶必须保留。
        let incoming: Vec<Entry> = list_entries(&conn, &key)
            .unwrap()
            .iter()
            .map(|e| {
                let mut c = e.clone();
                c.pinned = false;
                c
            })
            .collect();
        replace_entries(&conn, &key, &incoming, false).unwrap();
        assert_eq!(
            titles(&list_entries(&conn, &key).unwrap()),
            ["b", "a", "c"],
            "拉取/合并不得清空本设备的置顶"
        );

        // 置顶同步开启：服务端下发的置顶状态应被采纳（b 仍置顶、a 新增置顶）。
        let mut incoming2: Vec<Entry> = list_entries(&conn, &key)
            .unwrap()
            .iter()
            .map(|e| {
                let mut c = e.clone();
                c.pinned = c.title == "a" || c.title == "b";
                c
            })
            .collect();
        // incoming2 中 b 的 pinned 来自服务端可能为 false 的场景也覆盖：先全部置 false，
        // 仅 a 置 true，验证 adopt 模式下 b 的本地置顶被服务端状态覆盖。
        for e in incoming2.iter_mut() {
            e.pinned = e.title == "a";
        }
        replace_entries(&conn, &key, &incoming2, true).unwrap();
        let after_adopt = list_entries(&conn, &key).unwrap();
        assert!(
            after_adopt.iter().find(|e| e.title == "a").unwrap().pinned,
            "adopt 模式应采纳服务端置顶"
        );
        assert!(
            !after_adopt.iter().find(|e| e.title == "b").unwrap().pinned,
            "adopt 模式不应保留本地置顶"
        );
        // 还原 a 的置顶，便于后续断言从全不置顶状态出发。
        if let Some(ea) = after_adopt.iter().find(|e| e.title == "a") {
            set_pinned(&conn, ea.id, false).unwrap();
        }

        set_pinned(&conn, b.id, false).unwrap();
        let after = list_entries(&conn, &key).unwrap();
        assert!(after.iter().all(|e| !e.pinned));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn reorder_resequences_within_each_group() {
        let path = temp_db("reorder");
        let conn = open_store(&path).unwrap();
        let key = [3u8; 32];
        let a = save_entry(&conn, &key, new_entry("a")).unwrap();
        let b = save_entry(&conn, &key, new_entry("b")).unwrap();
        let c = save_entry(&conn, &key, new_entry("c")).unwrap();

        set_pinned(&conn, b.id, true).unwrap();
        // 按「当前显示顺序」重排应幂等。
        reorder_entries(&conn, &[b.id, a.id, c.id]).unwrap();
        assert_eq!(titles(&list_entries(&conn, &key).unwrap()), ["b", "a", "c"]);

        // 交换两个普通条目：置顶条目仍居首。
        reorder_entries(&conn, &[b.id, c.id, a.id]).unwrap();
        assert_eq!(titles(&list_entries(&conn, &key).unwrap()), ["b", "c", "a"]);
        let _ = std::fs::remove_file(&path);
    }
}
