package main

import (
	"database/sql"
	"os"
	"path/filepath"
	"runtime"

	_ "modernc.org/sqlite"
)

// isPortable 判断是否为便携模式：exe 同目录存在 portable.flag 标记文件（便携版 zip 打包时附带）。
func isPortable() bool {
	exe, err := os.Executable()
	if err != nil {
		return false
	}
	_, err = os.Stat(filepath.Join(filepath.Dir(exe), "portable.flag"))
	return err == nil
}

// dbFilePath 返回数据库文件路径：
//
//	Windows 便携模式（exe 旁有 portable.flag）→ exe 同目录 passbook.db
//	Windows 安装模式 → %APPDATA%\CryPtBox\passbook.db
//	macOS / Linux → 用户配置目录（~/.config/CryPtBox/ 等）
func dbFilePath() string {
	if runtime.GOOS == "windows" {
		if isPortable() {
			if exe, err := os.Executable(); err == nil {
				return filepath.Join(filepath.Dir(exe), "passbook.db")
			}
			return "passbook.db"
		}
		if dir, err := os.UserConfigDir(); err == nil {
			p := filepath.Join(dir, "CryPtBox")
			if err := os.MkdirAll(p, 0o700); err == nil {
				return filepath.Join(p, "passbook.db")
			}
		}
		return "passbook.db"
	}
	if dir, err := os.UserConfigDir(); err == nil {
		p := filepath.Join(dir, "CryPtBox")
		if err := os.MkdirAll(p, 0o700); err == nil {
			return filepath.Join(p, "passbook.db")
		}
	}
	return "passbook.db"
}

// migrateLegacyData 安装模式首次启动时，把 exe 同目录的旧便携数据迁移到 APPDATA。
// 迁移成功后旧文件重命名为 .migrated.bak 保留备份。
func migrateLegacyData() {
	if runtime.GOOS != "windows" || isPortable() {
		return
	}
	exe, err := os.Executable()
	if err != nil {
		return
	}
	legacy := filepath.Join(filepath.Dir(exe), "passbook.db")
	if _, err := os.Stat(legacy); err != nil {
		return // 无旧便携数据
	}
	target := dbFilePath()
	if target == legacy {
		return
	}
	if _, err := os.Stat(target); err == nil {
		return // 目标已存在，不覆盖
	}
	if err := copyFile(legacy, target); err != nil {
		return
	}
	_ = os.Rename(legacy, legacy+".migrated.bak")
}

// copyFile 复制单个文件（用于数据迁移）。
func copyFile(src, dst string) error {
	data, err := os.ReadFile(src)
	if err != nil {
		return err
	}
	return os.WriteFile(dst, data, 0o600)
}

// Entry 一条密码记录。Password / Notes 在内存中以明文出现（用于展示与同步），
// 落盘时被主密钥加密。Deleted 为墓碑标记：软删除后保留 id + 时间戳用于跨端同步传播删除。
type Entry struct {
	ID        int64  `json:"id"`
	Title     string `json:"title"`
	Username  string `json:"username"`
	Password  string `json:"password"`
	URL       string `json:"url"`
	Category  string `json:"category"`
	Notes     string `json:"notes"`
	CreatedAt string `json:"created_at"`
	UpdatedAt string `json:"updated_at"`
	Deleted   bool   `json:"deleted"`
}

func openStore() (*sql.DB, error) {
	migrateLegacyData()
	db, err := sql.Open("sqlite", dbFilePath())
	if err != nil {
		return nil, err
	}
	if err := db.Ping(); err != nil {
		return nil, err
	}
	stmts := []string{
		`CREATE TABLE IF NOT EXISTS entries (
			id INTEGER PRIMARY KEY AUTOINCREMENT,
			title TEXT NOT NULL,
			username TEXT NOT NULL DEFAULT '',
			password_enc TEXT NOT NULL DEFAULT '',
			url TEXT NOT NULL DEFAULT '',
			category TEXT NOT NULL DEFAULT '',
			notes_enc TEXT NOT NULL DEFAULT '',
			created_at TEXT NOT NULL,
			updated_at TEXT NOT NULL,
			deleted INTEGER NOT NULL DEFAULT 0
		)`,
		`CREATE TABLE IF NOT EXISTS meta (
			key TEXT PRIMARY KEY,
			value TEXT NOT NULL
		)`,
	}
	for _, s := range stmts {
		if _, err := db.Exec(s); err != nil {
			return nil, err
		}
	}
	// 迁移：为旧库补充 deleted 列（用于同步删除传播的墓碑标记）。
	if !columnExists(db, "entries", "deleted") {
		if _, err := db.Exec(`ALTER TABLE entries ADD COLUMN deleted INTEGER NOT NULL DEFAULT 0`); err != nil {
			return nil, err
		}
	}
	return db, nil
}

// columnExists 判断表中是否存在指定列（SQLite 无 ADD COLUMN IF NOT EXISTS）。
func columnExists(db *sql.DB, table, column string) bool {
	rows, err := db.Query(`PRAGMA table_info(` + table + `)`)
	if err != nil {
		return false
	}
	defer rows.Close()
	for rows.Next() {
		var cid int
		var name, ctype string
		var notnull, pk int
		var dflt sql.NullString
		if err := rows.Scan(&cid, &name, &ctype, &notnull, &dflt, &pk); err != nil {
			return false
		}
		if name == column {
			return true
		}
	}
	return false
}

func setMeta(db *sql.DB, key, value string) error {
	_, err := db.Exec(`INSERT OR REPLACE INTO meta (key, value) VALUES (?, ?)`, key, value)
	return err
}

func getMeta(db *sql.DB, key string) string {
	var v string
	_ = db.QueryRow(`SELECT value FROM meta WHERE key = ?`, key).Scan(&v)
	return v
}
