package main

import (
	"database/sql"
	"os"
	"path/filepath"
	"runtime"

	_ "modernc.org/sqlite"
)

// dbFilePath 返回数据库文件路径：
// macOS / Linux 放到用户配置目录（~/.config/CryPtBox/ 等，.app 与系统安装场景工作目录不可写），
// Windows 保持可执行文件同目录（兼容现有用户数据）。
func dbFilePath() string {
	if runtime.GOOS != "windows" {
		if dir, err := os.UserConfigDir(); err == nil {
			p := filepath.Join(dir, "CryPtBox")
			if err := os.MkdirAll(p, 0o700); err == nil {
				return filepath.Join(p, "passbook.db")
			}
		}
	}
	return "passbook.db"
}

// Entry 一条密码记录。Password / Notes 在内存中以明文出现（用于展示与同步），
// 落盘时被主密钥加密。
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
}

func openStore() (*sql.DB, error) {
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
			updated_at TEXT NOT NULL
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
	return db, nil
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
