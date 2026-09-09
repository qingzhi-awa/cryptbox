package main

import (
	"database/sql"
	"fmt"
	"net/http"
	"strings"
)

// LogEntry 日志条目。
type LogEntry struct {
	ID        int64  `json:"id"`
	UserID    int64  `json:"user_id"`
	Username  string `json:"username"`
	Action    string `json:"action"`
	Detail    string `json:"detail"`
	IP        string `json:"ip"`
	CreatedAt string `json:"created_at"`
}

// 日志保留策略：最多保留条数与天数，达到任意一个限制即淘汰最旧日志。
const (
	logMaxEntries = 999
	logMaxAgeDays = 365
)

// logAction 记录一条操作日志（尽力而为，不阻塞主流程），
// 写入后按策略裁剪：超过 logMaxEntries 条或超过 logMaxAgeDays 天的旧日志逐条删除。
func logAction(db *sql.DB, userID int64, username, action, detail, ip string) {
	if db == nil {
		return
	}
	_, _ = db.Exec(`INSERT INTO logs (user_id, username, action, detail, ip) VALUES (?, ?, ?, ?, ?)`,
		userID, username, action, detail, ip)
	// 条数超限：仅保留最新 logMaxEntries 条（新日志进一条，最旧的一条被替换掉）
	_, _ = db.Exec(`DELETE FROM logs WHERE id <= (SELECT id FROM logs ORDER BY id DESC LIMIT 1 OFFSET ?)`, logMaxEntries-1)
	// 天数超限：删除超过保留天数的旧日志
	_, _ = db.Exec(`DELETE FROM logs WHERE created_at < datetime('now', ?)`, fmt.Sprintf("-%d days", logMaxAgeDays))
}

// clientIP 从请求中提取客户端 IP。
func clientIP(r *http.Request) string {
	if fwd := r.Header.Get("X-Forwarded-For"); fwd != "" {
		return strings.TrimSpace(strings.Split(fwd, ",")[0])
	}
	ip := r.RemoteAddr
	if i := strings.LastIndex(ip, ":"); i >= 0 {
		ip = ip[:i]
	}
	return strings.TrimSpace(ip)
}
