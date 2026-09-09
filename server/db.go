package main

import (
	"database/sql"
	"time"

	_ "modernc.org/sqlite"
)

func openDB(cfg Config) (*sql.DB, error) {
	db, err := sql.Open("sqlite", cfg.DBDSN)
	if err != nil {
		return nil, err
	}
	db.SetMaxOpenConns(10)
	db.SetConnMaxLifetime(time.Hour)
	if err := db.Ping(); err != nil {
		return nil, err
	}
	if err := migrate(db); err != nil {
		return nil, err
	}
	if err := seedAdmin(db); err != nil {
		return nil, err
	}
	return db, nil
}

func migrate(db *sql.DB) error {
	idClause := "INTEGER PRIMARY KEY AUTOINCREMENT"

	stmts := []string{
		`CREATE TABLE IF NOT EXISTS users (
			id ` + idClause + `,
			username VARCHAR(128) NOT NULL UNIQUE,
			password_hash VARCHAR(255) NOT NULL,
			role VARCHAR(16) NOT NULL DEFAULT 'user',
			status VARCHAR(16) NOT NULL DEFAULT 'active',
			email VARCHAR(255) NOT NULL DEFAULT '',
			created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
		)`,
		`CREATE TABLE IF NOT EXISTS entries (
			id ` + idClause + `,
			user_id INTEGER NOT NULL,
			title VARCHAR(255) NOT NULL,
			username VARCHAR(255) NOT NULL DEFAULT '',
			url VARCHAR(512) NOT NULL DEFAULT '',
			category VARCHAR(128) NOT NULL DEFAULT '',
			password_enc TEXT NOT NULL,
			notes_enc TEXT NOT NULL,
			created_at VARCHAR(64) NOT NULL,
			updated_at VARCHAR(64) NOT NULL,
			deleted INTEGER NOT NULL DEFAULT 0
		)`,
		`CREATE TABLE IF NOT EXISTS meta (
			key VARCHAR(128) PRIMARY KEY,
			value TEXT NOT NULL
		)`,
		`CREATE TABLE IF NOT EXISTS logs (
			id ` + idClause + `,
			user_id INTEGER NOT NULL DEFAULT 0,
			username VARCHAR(128) NOT NULL DEFAULT '',
			action VARCHAR(64) NOT NULL,
			detail VARCHAR(512) NOT NULL DEFAULT '',
			ip VARCHAR(64) NOT NULL DEFAULT '',
			created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
		)`,
		`CREATE TABLE IF NOT EXISTS email_verifications (
			id ` + idClause + `,
			email VARCHAR(255) NOT NULL,
			code VARCHAR(64) NOT NULL,
			purpose VARCHAR(32) NOT NULL DEFAULT 'register',
			expires_at TIMESTAMP,
			created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
		)`,
	}
	for _, s := range stmts {
		if _, err := db.Exec(s); err != nil {
			return err
		}
	}
	// 兼容旧库：为已存在的 users 表补充 email 列。
	if err := ensureColumn(db, "users", "email", "VARCHAR(255) NOT NULL DEFAULT ''"); err != nil {
		return err
	}
	// 兼容旧库：为已存在的 entries 表补充 deleted 墓碑列。
	if err := ensureColumn(db, "entries", "deleted", "INTEGER NOT NULL DEFAULT 0"); err != nil {
		return err
	}
	return nil
}

// ensureColumn 在列不存在时为表新增列（SQLite）。
func ensureColumn(db *sql.DB, table, column, def string) error {
	exists, err := columnExists(db, table, column)
	if err != nil {
		return err
	}
	if exists {
		return nil
	}
	_, err = db.Exec("ALTER TABLE " + table + " ADD COLUMN " + column + " " + def)
	return err
}

// columnExists 检查表中是否存在指定列。
func columnExists(db *sql.DB, table, column string) (bool, error) {
	rows, err := db.Query("PRAGMA table_info(" + table + ")")
	if err != nil {
		return false, err
	}
	defer rows.Close()
	for rows.Next() {
		var cid int
		var name, ctype string
		var notnull int
		var dflt sql.NullString
		var pk int
		if err := rows.Scan(&cid, &name, &ctype, &notnull, &dflt, &pk); err != nil {
			return false, err
		}
		if name == column {
			return true, nil
		}
	}
	return false, rows.Err()
}

// findNextUserID 返回当前最小可用的用户 ID，用于删除用户后回收复用 ID。
func findNextUserID(db *sql.DB) (int64, error) {
	rows, err := db.Query(`SELECT id FROM users ORDER BY id`)
	if err != nil {
		return 0, err
	}
	defer rows.Close()
	var expected int64 = 1
	for rows.Next() {
		var id int64
		if err := rows.Scan(&id); err != nil {
			return 0, err
		}
		if id > expected {
			return expected, nil
		}
		expected = id + 1
	}
	return expected, rows.Err()
}

// seedAdmin 不再创建默认账号；仅兼容旧库：将历史 admin 账号的 role 升级为 superadmin。
func seedAdmin(db *sql.DB) error {
	_, err := db.Exec(`UPDATE users SET role = 'superadmin' WHERE username = 'admin' AND role = 'admin'`)
	return err
}

// isInitialized 返回是否已有用户（即是否已完成初始管理员设置）。
func isInitialized(db *sql.DB) bool {
	var count int
	if err := db.QueryRow(`SELECT COUNT(1) FROM users`).Scan(&count); err != nil {
		return false
	}
	return count > 0
}

// setupAdmin 首次创建超级管理员，返回新用户 ID。
func setupAdmin(db *sql.DB, username, password string) (int64, error) {
	hash, err := hashPassword(password)
	if err != nil {
		return 0, err
	}
	res, err := db.Exec(`INSERT INTO users (username, password_hash, role, status) VALUES (?, ?, 'superadmin', 'active')`, username, hash)
	if err != nil {
		return 0, err
	}
	return res.LastInsertId()
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
