package main

import (
	"context"
	"database/sql"
	"encoding/base64"
	"encoding/json"
	"errors"
	"net/http"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"time"
)

type Server struct {
	cfg    Config
	db     *sql.DB
	encKey []byte
}

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

type User struct {
	ID        int64  `json:"id"`
	Username  string `json:"username"`
	Email     string `json:"email"`
	Avatar    string `json:"avatar"`
	Role      string `json:"role"`
	Status    string `json:"status"`
	CreatedAt string `json:"created_at"`
}

// SiteConfig 站点信息（仅页脚文字）。
type SiteConfig struct {
	FooterText string `json:"footer_text"`
}

type ctxKey string

const claimsKey ctxKey = "claims"

func (s *Server) routes() http.Handler {
	mux := http.NewServeMux()

	// 公开
	mux.HandleFunc("GET /api/health", s.handleHealth)
	mux.HandleFunc("GET /api/status", s.handleStatus)
	mux.HandleFunc("POST /api/setup", s.handleSetup)
	mux.HandleFunc("GET /api/avatar/{id}", s.handleGetAvatar)
	mux.HandleFunc("POST /api/register", s.handleRegister)
	mux.HandleFunc("POST /api/register/send-code", s.handleSendRegisterCode)
	mux.HandleFunc("GET /api/settings/public", s.handlePublicSettings)
	mux.HandleFunc("POST /api/login", s.handleLogin)
	mux.HandleFunc("POST /api/reset/send-code", s.handleSendResetCode)
	mux.HandleFunc("POST /api/reset", s.handleResetPassword)

	// 登录用户
	mux.HandleFunc("GET /api/me", s.auth(s.handleMe))
	mux.HandleFunc("PUT /api/me", s.auth(s.handleUpdateMe))
	mux.HandleFunc("POST /api/me/avatar", s.auth(s.handleUploadAvatar))
	mux.HandleFunc("GET /api/entries", s.auth(s.handleListEntries))
	mux.HandleFunc("POST /api/entries", s.auth(s.handleCreateEntry))
	mux.HandleFunc("POST /api/entries/import", s.auth(s.handleImportEntries))
	mux.HandleFunc("POST /api/entries/import-text", s.auth(s.handleImportText))
	mux.HandleFunc("PUT /api/entries/{id}", s.auth(s.handleUpdateEntry))
	mux.HandleFunc("DELETE /api/entries/{id}", s.auth(s.handleDeleteEntry))
	mux.HandleFunc("GET /api/vault", s.auth(s.handleGetVault))
	mux.HandleFunc("PUT /api/vault", s.auth(s.handlePutVault))

	// 管理员
	mux.HandleFunc("GET /api/users", s.auth(s.admin(s.handleListUsers)))
	mux.HandleFunc("POST /api/users", s.auth(s.admin(s.handleCreateUser)))
	mux.HandleFunc("POST /api/users/import", s.auth(s.admin(s.handleImportUsers)))
	mux.HandleFunc("DELETE /api/users/{id}", s.auth(s.admin(s.handleDeleteUser)))
	mux.HandleFunc("PUT /api/users/{id}", s.auth(s.admin(s.handleUpdateUser)))
	mux.HandleFunc("PUT /api/users/{id}/status", s.auth(s.admin(s.handleUpdateUserStatus)))
	mux.HandleFunc("GET /api/logs", s.auth(s.admin(s.handleListLogs)))
	mux.HandleFunc("GET /api/settings", s.auth(s.admin(s.handleGetSettings)))
	mux.HandleFunc("PUT /api/settings", s.auth(s.admin(s.handleUpdateSettings)))

	return withCORS(mux)
}

// ---- 公开接口 ----

func (s *Server) handleHealth(w http.ResponseWriter, r *http.Request) {
	writeJSON(w, http.StatusOK, map[string]string{"status": "ok", "time": time.Now().Format(time.RFC3339)})
}

// handleStatus 返回是否已初始化（是否已有用户）。
func (s *Server) handleStatus(w http.ResponseWriter, r *http.Request) {
	writeJSON(w, http.StatusOK, map[string]bool{"initialized": isInitialized(s.db)})
}

// handleSetup 首次设置超级管理员账号（系统无用户时）。
func (s *Server) handleSetup(w http.ResponseWriter, r *http.Request) {
	var req struct {
		Username string `json:"username"`
		Password string `json:"password"`
	}
	if err := readJSON(r, &req); err != nil {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": "invalid body"})
		return
	}
	req.Username = strings.TrimSpace(req.Username)
	if req.Username == "" || len(req.Password) < 6 {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": "用户名不能为空，密码至少 6 位"})
		return
	}
	if isInitialized(s.db) {
		writeJSON(w, http.StatusConflict, map[string]string{"error": "系统已初始化"})
		return
	}
	var exists int
	if err := s.db.QueryRow(`SELECT COUNT(1) FROM users WHERE username = ?`, req.Username).Scan(&exists); err != nil {
		writeJSON(w, http.StatusInternalServerError, map[string]string{"error": "db error"})
		return
	}
	if exists > 0 {
		writeJSON(w, http.StatusConflict, map[string]string{"error": "用户名已存在"})
		return
	}
	id, err := setupAdmin(s.db, req.Username, req.Password)
	if err != nil {
		writeJSON(w, http.StatusInternalServerError, map[string]string{"error": "db error"})
		return
	}
	token, _ := generateToken(s.cfg.JWTSecret, id, req.Username, "superadmin")
	logAction(s.db, id, req.Username, "setup", "初始化超级管理员", clientIP(r))
	writeJSON(w, http.StatusOK, map[string]string{"token": token, "username": req.Username, "role": "superadmin", "avatar": avatarName(id)})
}

func (s *Server) handleRegister(w http.ResponseWriter, r *http.Request) {
	var req struct {
		Username string `json:"username"`
		Password string `json:"password"`
		Email    string `json:"email"`
		Code     string `json:"code"`
	}
	if err := readJSON(r, &req); err != nil {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": "invalid body"})
		return
	}
	req.Username = strings.TrimSpace(req.Username)
	req.Email = strings.TrimSpace(req.Email)
	if req.Username == "" {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": "用户名不能为空"})
		return
	}
	if !allowRegistration(s.db) {
		writeJSON(w, http.StatusForbidden, map[string]string{"error": "注册未开放，请联系管理员"})
		return
	}
	if err := validatePassword(s.db, req.Password); err != nil {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": err.Error()})
		return
	}
	if req.Email == "" {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": "请填写邮箱"})
		return
	}
	if !strings.Contains(req.Email, "@") || !strings.Contains(req.Email, ".") {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": "邮箱格式不正确"})
		return
	}
	if !checkVerification(s.db, req.Email, req.Code, "register") {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": "邮箱验证码错误或已过期"})
		return
	}
	var exists int
	if err := s.db.QueryRow(`SELECT COUNT(1) FROM users WHERE username = ?`, req.Username).Scan(&exists); err != nil {
		writeJSON(w, http.StatusInternalServerError, map[string]string{"error": "db error"})
		return
	}
	if exists > 0 {
		writeJSON(w, http.StatusConflict, map[string]string{"error": "用户名已存在"})
		return
	}
	hash, err := hashPassword(req.Password)
	if err != nil {
		writeJSON(w, http.StatusInternalServerError, map[string]string{"error": "hash error"})
		return
	}
	id, err := findNextUserID(s.db)
	if err != nil {
		writeJSON(w, http.StatusInternalServerError, map[string]string{"error": "db error"})
		return
	}
	_, err = s.db.Exec(`INSERT INTO users (id, username, password_hash, role, status, email) VALUES (?, ?, ?, 'user', 'active', ?)`, id, req.Username, hash, req.Email)
	if err != nil {
		writeJSON(w, http.StatusInternalServerError, map[string]string{"error": "db error"})
		return
	}
	token, _ := generateToken(s.cfg.JWTSecret, id, req.Username, "user")
	writeJSON(w, http.StatusOK, map[string]string{"token": token, "username": req.Username, "role": "user", "avatar": avatarName(id)})
}

func (s *Server) handleLogin(w http.ResponseWriter, r *http.Request) {
	var req struct {
		Username string `json:"username"`
		Password string `json:"password"`
	}
	if err := readJSON(r, &req); err != nil {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": "invalid body"})
		return
	}
	login := strings.TrimSpace(req.Username)
	var (
		id     int64
		uname  string
		hash   string
		role   string
		status string
	)
	err := s.db.QueryRow(`SELECT id, username, password_hash, role, status FROM users WHERE username = ? OR email = ?`, login, login).Scan(&id, &uname, &hash, &role, &status)
	if err == sql.ErrNoRows || (err == nil && !checkPassword(hash, req.Password)) {
		logAction(s.db, 0, login, "login_failed", "登录失败", clientIP(r))
		writeJSON(w, http.StatusUnauthorized, map[string]string{"error": "用户名或密码错误"})
		return
	}
	if err != nil {
		writeJSON(w, http.StatusInternalServerError, map[string]string{"error": "db error"})
		return
	}
	if status != "active" {
		writeJSON(w, http.StatusForbidden, map[string]string{"error": "账号已被停用"})
		return
	}
	logAction(s.db, id, uname, "login", "用户登录", clientIP(r))
	token, _ := generateToken(s.cfg.JWTSecret, id, uname, role)
	writeJSON(w, http.StatusOK, map[string]string{"token": token, "username": uname, "role": role, "avatar": avatarName(id)})
}

func (s *Server) handleMe(w http.ResponseWriter, r *http.Request) {
	claims := currentClaims(r)
	var username, email string
	_ = s.db.QueryRow(`SELECT username, email FROM users WHERE id = ?`, claims.UserID).Scan(&username, &email)
	writeJSON(w, http.StatusOK, map[string]interface{}{
		"id":       claims.UserID,
		"username": username,
		"email":    email,
		"avatar":   avatarName(claims.UserID),
		"role":     claims.Role,
	})
}

// ---- 密码条目 CRUD（仅本人） ----

func (s *Server) handleListEntries(w http.ResponseWriter, r *http.Request) {
	claims := currentClaims(r)
	list, err := s.listEntries(claims.UserID)
	if err != nil {
		writeJSON(w, http.StatusInternalServerError, map[string]string{"error": "db error"})
		return
	}
	writeJSON(w, http.StatusOK, map[string]interface{}{"entries": list})
}

func (s *Server) handleCreateEntry(w http.ResponseWriter, r *http.Request) {
	claims := currentClaims(r)
	var e Entry
	if err := readJSON(r, &e); err != nil {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": "invalid body"})
		return
	}
	saved, err := s.saveEntry(claims.UserID, e)
	if err != nil {
		writeJSON(w, http.StatusInternalServerError, map[string]string{"error": "db error"})
		return
	}
	logAction(s.db, claims.UserID, claims.Username, "add_entry", "添加密码 "+saved.Title, clientIP(r))
	writeJSON(w, http.StatusOK, saved)
}

func (s *Server) handleUpdateEntry(w http.ResponseWriter, r *http.Request) {
	claims := currentClaims(r)
	id, err := strconv.ParseInt(r.PathValue("id"), 10, 64)
	if err != nil {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": "invalid id"})
		return
	}
	var e Entry
	if err := readJSON(r, &e); err != nil {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": "invalid body"})
		return
	}
	e.ID = id
	saved, err := s.saveEntry(claims.UserID, e)
	if err != nil {
		writeJSON(w, http.StatusInternalServerError, map[string]string{"error": "db error"})
		return
	}
	logAction(s.db, claims.UserID, claims.Username, "update_entry", "修改密码 "+saved.Title, clientIP(r))
	writeJSON(w, http.StatusOK, saved)
}

func (s *Server) handleDeleteEntry(w http.ResponseWriter, r *http.Request) {
	claims := currentClaims(r)
	id, err := strconv.ParseInt(r.PathValue("id"), 10, 64)
	if err != nil {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": "invalid id"})
		return
	}
	var title string
	_ = s.db.QueryRow(`SELECT title FROM entries WHERE id = ? AND user_id = ?`, id, claims.UserID).Scan(&title)
	// 软删除：打墓碑标记并清空敏感字段，保留 id 与时间戳用于跨端同步传播删除。
	now := time.Now().Format(time.RFC3339)
	_, err = s.db.Exec(`UPDATE entries SET deleted = 1, password_enc = '', notes_enc = '', updated_at = ? WHERE id = ? AND user_id = ?`, now, id, claims.UserID)
	if err != nil {
		writeJSON(w, http.StatusInternalServerError, map[string]string{"error": "db error"})
		return
	}
	logAction(s.db, claims.UserID, claims.Username, "delete_entry", "删除密码 "+title, clientIP(r))
	writeJSON(w, http.StatusOK, map[string]string{"status": "ok"})
}

// handleImportEntries 批量导入 CSV 密码，返回导入条数。
func (s *Server) handleImportEntries(w http.ResponseWriter, r *http.Request) {
	claims := currentClaims(r)
	var req struct {
		CSV string `json:"csv"`
	}
	if err := readJSON(r, &req); err != nil {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": "invalid body"})
		return
	}
	entries, err := parseCSVEntries([]byte(req.CSV))
	if err != nil {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": err.Error()})
		return
	}
	count := 0
	for _, e := range entries {
		if _, err := s.saveEntry(claims.UserID, e); err != nil {
			writeJSON(w, http.StatusInternalServerError, map[string]string{"error": "db error"})
			return
		}
		count++
	}
	logAction(s.db, claims.UserID, claims.Username, "import_entries", "导入密码 "+strconv.Itoa(count)+" 条", clientIP(r))
	writeJSON(w, http.StatusOK, map[string]int{"count": count})
}

// handleImportText 从 TXT 批量导入密码。
func (s *Server) handleImportText(w http.ResponseWriter, r *http.Request) {
	claims := currentClaims(r)
	var req struct {
		Text string `json:"text"`
	}
	if err := readJSON(r, &req); err != nil {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": "invalid body"})
		return
	}
	entries, err := parseTXTEntries([]byte(req.Text))
	if err != nil {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": err.Error()})
		return
	}
	count := 0
	for _, e := range entries {
		if _, err := s.saveEntry(claims.UserID, e); err != nil {
			writeJSON(w, http.StatusInternalServerError, map[string]string{"error": "db error"})
			return
		}
		count++
	}
	logAction(s.db, claims.UserID, claims.Username, "import_entries", "导入密码 "+strconv.Itoa(count)+" 条", clientIP(r))
	writeJSON(w, http.StatusOK, map[string]int{"count": count})
}

// ---- 桌面客户端同步（整体上传/下载） ----

func (s *Server) handleGetVault(w http.ResponseWriter, r *http.Request) {
	claims := currentClaims(r)
	list, err := s.listEntriesAll(claims.UserID)
	if err != nil {
		writeJSON(w, http.StatusInternalServerError, map[string]string{"error": "db error"})
		return
	}
	writeJSON(w, http.StatusOK, map[string]interface{}{"entries": list})
}

func (s *Server) handlePutVault(w http.ResponseWriter, r *http.Request) {
	claims := currentClaims(r)
	var req struct {
		Entries []Entry `json:"entries"`
	}
	if err := readJSON(r, &req); err != nil {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": "invalid body"})
		return
	}
	if err := s.replaceEntries(claims.UserID, req.Entries); err != nil {
		writeJSON(w, http.StatusInternalServerError, map[string]string{"error": "db error"})
		return
	}
	logAction(s.db, claims.UserID, claims.Username, "upload_vault", "上传密码库 "+strconv.Itoa(len(req.Entries))+" 条", clientIP(r))
	writeJSON(w, http.StatusOK, map[string]string{"status": "ok"})
}

// ---- 用户管理（仅管理员） ----

func (s *Server) handleListUsers(w http.ResponseWriter, r *http.Request) {
	rows, err := s.db.Query(`SELECT id, username, email, role, status, created_at FROM users ORDER BY id`)
	if err != nil {
		writeJSON(w, http.StatusInternalServerError, map[string]string{"error": "db error"})
		return
	}
	defer rows.Close()
	list := []User{}
	for rows.Next() {
		var u User
		var created string
		if err := rows.Scan(&u.ID, &u.Username, &u.Email, &u.Role, &u.Status, &created); err != nil {
			writeJSON(w, http.StatusInternalServerError, map[string]string{"error": "db error"})
			return
		}
		u.CreatedAt = created
		u.Avatar = avatarName(u.ID)
		list = append(list, u)
	}
	writeJSON(w, http.StatusOK, map[string]interface{}{"users": list})
}

func (s *Server) handleCreateUser(w http.ResponseWriter, r *http.Request) {
	var req struct {
		Username string `json:"username"`
		Password string `json:"password"`
		Email    string `json:"email"`
		Role     string `json:"role"`
	}
	if err := readJSON(r, &req); err != nil {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": "invalid body"})
		return
	}
	req.Username = strings.TrimSpace(req.Username)
	if req.Username == "" || len(req.Password) < 6 {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": "用户名不能为空，密码至少 6 位"})
		return
	}
	if req.Role != "admin" && req.Role != "user" {
		req.Role = "user"
	}
	var exists int
	if err := s.db.QueryRow(`SELECT COUNT(1) FROM users WHERE username = ?`, req.Username).Scan(&exists); err != nil {
		writeJSON(w, http.StatusInternalServerError, map[string]string{"error": "db error"})
		return
	}
	if exists > 0 {
		writeJSON(w, http.StatusConflict, map[string]string{"error": "用户名已存在"})
		return
	}
	hash, err := hashPassword(req.Password)
	if err != nil {
		writeJSON(w, http.StatusInternalServerError, map[string]string{"error": "hash error"})
		return
	}
	id, err := findNextUserID(s.db)
	if err != nil {
		writeJSON(w, http.StatusInternalServerError, map[string]string{"error": "db error"})
		return
	}
	_, err = s.db.Exec(`INSERT INTO users (id, username, password_hash, role, status, email) VALUES (?, ?, ?, ?, 'active', ?)`, id, req.Username, hash, req.Role, req.Email)
	if err != nil {
		writeJSON(w, http.StatusInternalServerError, map[string]string{"error": "db error"})
		return
	}
	claims := currentClaims(r)
	logAction(s.db, claims.UserID, claims.Username, "create_user", "添加用户 "+req.Username, clientIP(r))
	writeJSON(w, http.StatusOK, map[string]string{"status": "ok"})
}

// handleImportUsers 批量导入用户（CSV）。表头需含用户名、密码列，邮箱、角色可选；
// 用户名已存在或密码不足 6 位的行跳过，返回导入与跳过条数。
func (s *Server) handleImportUsers(w http.ResponseWriter, r *http.Request) {
	claims := currentClaims(r)
	var req struct {
		CSV string `json:"csv"`
	}
	if err := readJSON(r, &req); err != nil {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": "invalid body"})
		return
	}
	users, err := parseCSVUsers([]byte(req.CSV))
	if err != nil {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": err.Error()})
		return
	}
	count, skipped := 0, 0
	for _, u := range users {
		username := strings.TrimSpace(u.Username)
		password := strings.TrimSpace(u.Password)
		if username == "" || len(password) < 6 {
			skipped++
			continue
		}
		role := "user"
		rl := strings.TrimSpace(u.Role)
		if rl == "admin" || rl == "管理员" || rl == "管理員" {
			role = "admin"
		}
		var exists int
		if err := s.db.QueryRow(`SELECT COUNT(1) FROM users WHERE username = ?`, username).Scan(&exists); err != nil {
			writeJSON(w, http.StatusInternalServerError, map[string]string{"error": "db error"})
			return
		}
		if exists > 0 {
			skipped++
			continue
		}
		hash, err := hashPassword(password)
		if err != nil {
			writeJSON(w, http.StatusInternalServerError, map[string]string{"error": "hash error"})
			return
		}
		id, err := findNextUserID(s.db)
		if err != nil {
			writeJSON(w, http.StatusInternalServerError, map[string]string{"error": "db error"})
			return
		}
		if _, err := s.db.Exec(`INSERT INTO users (id, username, password_hash, role, status, email) VALUES (?, ?, ?, ?, 'active', ?)`,
			id, username, hash, role, strings.TrimSpace(u.Email)); err != nil {
			writeJSON(w, http.StatusInternalServerError, map[string]string{"error": "db error"})
			return
		}
		count++
	}
	logAction(s.db, claims.UserID, claims.Username, "import_users", "导入用户 "+strconv.Itoa(count)+" 条", clientIP(r))
	writeJSON(w, http.StatusOK, map[string]int{"count": count, "skipped": skipped})
}

func (s *Server) handleDeleteUser(w http.ResponseWriter, r *http.Request) {
	claims := currentClaims(r)
	id, err := strconv.ParseInt(r.PathValue("id"), 10, 64)
	if err != nil {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": "invalid id"})
		return
	}
	target, err := s.getUser(id)
	if err == sql.ErrNoRows {
		writeJSON(w, http.StatusNotFound, map[string]string{"error": "用户不存在"})
		return
	}
	if err != nil {
		writeJSON(w, http.StatusInternalServerError, map[string]string{"error": "db error"})
		return
	}
	if !canManageUser(claims.Role, target.Role) {
		writeJSON(w, http.StatusForbidden, map[string]string{"error": "无权限操作该用户"})
		return
	}
	if id == claims.UserID {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": "不能删除自己"})
		return
	}
	_, err = s.db.Exec(`DELETE FROM entries WHERE user_id = ?`, id)
	if err != nil {
		writeJSON(w, http.StatusInternalServerError, map[string]string{"error": "db error"})
		return
	}
	_, err = s.db.Exec(`DELETE FROM users WHERE id = ?`, id)
	if err != nil {
		writeJSON(w, http.StatusInternalServerError, map[string]string{"error": "db error"})
		return
	}
	logAction(s.db, claims.UserID, claims.Username, "delete_user", "删除用户 "+target.Username, clientIP(r))
	writeJSON(w, http.StatusOK, map[string]string{"status": "ok"})
}

func (s *Server) handleUpdateUserStatus(w http.ResponseWriter, r *http.Request) {
	claims := currentClaims(r)
	id, err := strconv.ParseInt(r.PathValue("id"), 10, 64)
	if err != nil {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": "invalid id"})
		return
	}
	var req struct {
		Status string `json:"status"`
	}
	if err := readJSON(r, &req); err != nil {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": "invalid body"})
		return
	}
	if req.Status != "active" && req.Status != "disabled" {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": "status 必须为 active 或 disabled"})
		return
	}
	target, err := s.getUser(id)
	if err == sql.ErrNoRows {
		writeJSON(w, http.StatusNotFound, map[string]string{"error": "用户不存在"})
		return
	}
	if err != nil {
		writeJSON(w, http.StatusInternalServerError, map[string]string{"error": "db error"})
		return
	}
	if !canManageUser(claims.Role, target.Role) {
		writeJSON(w, http.StatusForbidden, map[string]string{"error": "无权限操作该用户"})
		return
	}
	if id == claims.UserID && req.Status == "disabled" {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": "不能停用自己"})
		return
	}
	_, err = s.db.Exec(`UPDATE users SET status = ? WHERE id = ?`, req.Status, id)
	if err != nil {
		writeJSON(w, http.StatusInternalServerError, map[string]string{"error": "db error"})
		return
	}
	logAction(s.db, claims.UserID, claims.Username, "update_user_status", "设置用户 "+target.Username+" 状态为 "+req.Status, clientIP(r))
	writeJSON(w, http.StatusOK, map[string]string{"status": "ok"})
}

// handleUpdateUser 管理员/超级管理员修改指定用户的信息。
func (s *Server) handleUpdateUser(w http.ResponseWriter, r *http.Request) {
	claims := currentClaims(r)
	id, err := strconv.ParseInt(r.PathValue("id"), 10, 64)
	if err != nil {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": "invalid id"})
		return
	}
	target, err := s.getUser(id)
	if err == sql.ErrNoRows {
		writeJSON(w, http.StatusNotFound, map[string]string{"error": "用户不存在"})
		return
	}
	if err != nil {
		writeJSON(w, http.StatusInternalServerError, map[string]string{"error": "db error"})
		return
	}
	if !canManageUser(claims.Role, target.Role) {
		writeJSON(w, http.StatusForbidden, map[string]string{"error": "无权限操作该用户"})
		return
	}
	if id == claims.UserID {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": "请通过「我的账号」修改自己"})
		return
	}
	var req struct {
		Username string `json:"username"`
		Email    string `json:"email"`
		Password string `json:"password"`
		Role     string `json:"role"`
		Status   string `json:"status"`
	}
	if err := readJSON(r, &req); err != nil {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": "invalid body"})
		return
	}
	username := strings.TrimSpace(req.Username)
	if username == "" {
		username = target.Username
	}
	role := target.Role
	if req.Role != "" {
		if req.Role != "admin" && req.Role != "user" {
			writeJSON(w, http.StatusBadRequest, map[string]string{"error": "role 只能为 admin 或 user"})
			return
		}
		role = req.Role
	}
	status := target.Status
	if req.Status != "" {
		if req.Status != "active" && req.Status != "disabled" {
			writeJSON(w, http.StatusBadRequest, map[string]string{"error": "status 必须为 active 或 disabled"})
			return
		}
		status = req.Status
	}
	if username != target.Username {
		var exists int
		if err := s.db.QueryRow(`SELECT COUNT(1) FROM users WHERE username = ? AND id != ?`, username, id).Scan(&exists); err != nil {
			writeJSON(w, http.StatusInternalServerError, map[string]string{"error": "db error"})
			return
		}
		if exists > 0 {
			writeJSON(w, http.StatusConflict, map[string]string{"error": "用户名已存在"})
			return
		}
	}
	email := req.Email
	if req.Password != "" {
		if len(req.Password) < 6 {
			writeJSON(w, http.StatusBadRequest, map[string]string{"error": "密码至少 6 位"})
			return
		}
		hash, err := hashPassword(req.Password)
		if err != nil {
			writeJSON(w, http.StatusInternalServerError, map[string]string{"error": "hash error"})
			return
		}
		if _, err := s.db.Exec(`UPDATE users SET username=?, email=?, password_hash=?, role=?, status=? WHERE id=?`, username, email, hash, role, status, id); err != nil {
			writeJSON(w, http.StatusInternalServerError, map[string]string{"error": "db error"})
			return
		}
	} else {
		if _, err := s.db.Exec(`UPDATE users SET username=?, email=?, role=?, status=? WHERE id=?`, username, email, role, status, id); err != nil {
			writeJSON(w, http.StatusInternalServerError, map[string]string{"error": "db error"})
			return
		}
	}
	logAction(s.db, claims.UserID, claims.Username, "update_user", "修改用户 "+target.Username, clientIP(r))
	writeJSON(w, http.StatusOK, map[string]string{"status": "ok"})
}

// handleUpdateMe 用户修改自己的用户名、邮箱与密码。
func (s *Server) handleUpdateMe(w http.ResponseWriter, r *http.Request) {
	claims := currentClaims(r)
	var req struct {
		Username        string `json:"username"`
		Email           string `json:"email"`
		CurrentPassword string `json:"current_password"`
		NewPassword     string `json:"new_password"`
	}
	if err := readJSON(r, &req); err != nil {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": "invalid body"})
		return
	}
	username := strings.TrimSpace(req.Username)
	if username == "" {
		username = claims.Username
	}
	if username != claims.Username {
		var exists int
		if err := s.db.QueryRow(`SELECT COUNT(1) FROM users WHERE username = ? AND id != ?`, username, claims.UserID).Scan(&exists); err != nil {
			writeJSON(w, http.StatusInternalServerError, map[string]string{"error": "db error"})
			return
		}
		if exists > 0 {
			writeJSON(w, http.StatusConflict, map[string]string{"error": "用户名已存在"})
			return
		}
	}
	if req.NewPassword != "" {
		if len(req.NewPassword) < 6 {
			writeJSON(w, http.StatusBadRequest, map[string]string{"error": "新密码至少 6 位"})
			return
		}
		var hash string
		if err := s.db.QueryRow(`SELECT password_hash FROM users WHERE id = ?`, claims.UserID).Scan(&hash); err != nil {
			writeJSON(w, http.StatusInternalServerError, map[string]string{"error": "db error"})
			return
		}
		if !checkPassword(hash, req.CurrentPassword) {
			writeJSON(w, http.StatusBadRequest, map[string]string{"error": "当前密码错误"})
			return
		}
		newHash, err := hashPassword(req.NewPassword)
		if err != nil {
			writeJSON(w, http.StatusInternalServerError, map[string]string{"error": "hash error"})
			return
		}
		if _, err := s.db.Exec(`UPDATE users SET username=?, email=?, password_hash=? WHERE id=?`, username, req.Email, newHash, claims.UserID); err != nil {
			writeJSON(w, http.StatusInternalServerError, map[string]string{"error": "db error"})
			return
		}
	} else {
		if _, err := s.db.Exec(`UPDATE users SET username=?, email=? WHERE id=?`, username, req.Email, claims.UserID); err != nil {
			writeJSON(w, http.StatusInternalServerError, map[string]string{"error": "db error"})
			return
		}
	}
	detail := "修改账号信息"
	if req.NewPassword != "" {
		detail = "修改密码"
	}
	logAction(s.db, claims.UserID, claims.Username, "update_me", detail, clientIP(r))
	writeJSON(w, http.StatusOK, map[string]string{"status": "ok"})
}

// ---- 数据访问辅助 ----

func (s *Server) listEntries(userID int64) ([]Entry, error) {
	rows, err := s.db.Query(`SELECT id, title, username, url, category, password_enc, notes_enc, created_at, updated_at FROM entries WHERE user_id = ? AND deleted = 0 ORDER BY updated_at DESC`, userID)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	list := []Entry{}
	for rows.Next() {
		var e Entry
		var pwEnc, notesEnc string
		if err := rows.Scan(&e.ID, &e.Title, &e.Username, &e.URL, &e.Category, &pwEnc, &notesEnc, &e.CreatedAt, &e.UpdatedAt); err != nil {
			return nil, err
		}
		if e.Password, err = aesDecryptString(s.encKey, pwEnc); err != nil {
			return nil, err
		}
		if e.Notes, err = aesDecryptString(s.encKey, notesEnc); err != nil {
			return nil, err
		}
		list = append(list, e)
	}
	return list, rows.Err()
}

// listEntriesAll 返回该用户的全部条目（含墓碑），用于同步 vault 上传/下载。
func (s *Server) listEntriesAll(userID int64) ([]Entry, error) {
	rows, err := s.db.Query(`SELECT id, title, username, url, category, password_enc, notes_enc, created_at, updated_at, deleted FROM entries WHERE user_id = ? ORDER BY updated_at DESC`, userID)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	list := []Entry{}
	for rows.Next() {
		var e Entry
		var pwEnc, notesEnc string
		var deleted int
		if err := rows.Scan(&e.ID, &e.Title, &e.Username, &e.URL, &e.Category, &pwEnc, &notesEnc, &e.CreatedAt, &e.UpdatedAt, &deleted); err != nil {
			return nil, err
		}
		if e.Password, err = aesDecryptString(s.encKey, pwEnc); err != nil {
			return nil, err
		}
		if e.Notes, err = aesDecryptString(s.encKey, notesEnc); err != nil {
			return nil, err
		}
		e.Deleted = deleted != 0
		list = append(list, e)
	}
	return list, rows.Err()
}

func (s *Server) saveEntry(userID int64, e Entry) (Entry, error) {
	pwEnc, err := aesEncryptString(s.encKey, e.Password)
	if err != nil {
		return e, err
	}
	notesEnc, err := aesEncryptString(s.encKey, e.Notes)
	if err != nil {
		return e, err
	}
	now := time.Now().Format(time.RFC3339)
	if e.ID == 0 {
		e.CreatedAt = now
		e.UpdatedAt = now
		res, err := s.db.Exec(`INSERT INTO entries (user_id, title, username, url, category, password_enc, notes_enc, created_at, updated_at) VALUES (?,?,?,?,?,?,?,?,?)`,
			userID, e.Title, e.Username, e.URL, e.Category, pwEnc, notesEnc, e.CreatedAt, e.UpdatedAt)
		if err != nil {
			return e, err
		}
		e.ID, _ = res.LastInsertId()
	} else {
		e.UpdatedAt = now
		_, err := s.db.Exec(`UPDATE entries SET title=?, username=?, url=?, category=?, password_enc=?, notes_enc=?, updated_at=? WHERE id=? AND user_id=?`,
			e.Title, e.Username, e.URL, e.Category, pwEnc, notesEnc, e.UpdatedAt, e.ID, userID)
		if err != nil {
			return e, err
		}
	}
	return e, nil
}

func (s *Server) replaceEntries(userID int64, list []Entry) error {
	tx, err := s.db.Begin()
	if err != nil {
		return err
	}
	if _, err := tx.Exec(`DELETE FROM entries WHERE user_id = ?`, userID); err != nil {
		_ = tx.Rollback()
		return err
	}
	for _, e := range list {
		pwEnc, err := aesEncryptString(s.encKey, e.Password)
		if err != nil {
			_ = tx.Rollback()
			return err
		}
		notesEnc, err := aesEncryptString(s.encKey, e.Notes)
		if err != nil {
			_ = tx.Rollback()
			return err
		}
		deleted := 0
		if e.Deleted {
			deleted = 1
		}
		_, err = tx.Exec(`INSERT INTO entries (id, user_id, title, username, url, category, password_enc, notes_enc, created_at, updated_at, deleted) VALUES (?,?,?,?,?,?,?,?,?,?,?)`,
			e.ID, userID, e.Title, e.Username, e.URL, e.Category, pwEnc, notesEnc, e.CreatedAt, e.UpdatedAt, deleted)
		if err != nil {
			_ = tx.Rollback()
			return err
		}
	}
	return tx.Commit()
}

// ---- 中间件 ----

func currentClaims(r *http.Request) *Claims {
	return r.Context().Value(claimsKey).(*Claims)
}

func (s *Server) auth(next http.HandlerFunc) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		h := r.Header.Get("Authorization")
		token := strings.TrimPrefix(h, "Bearer ")
		if token == "" || token == h {
			writeJSON(w, http.StatusUnauthorized, map[string]string{"error": "unauthorized"})
			return
		}
		claims, err := parseToken(s.cfg.JWTSecret, token)
		if err != nil {
			writeJSON(w, http.StatusUnauthorized, map[string]string{"error": "unauthorized"})
			return
		}
		// 从数据库取最新角色与状态，使停用/改角色即时生效。
		var role, status string
		err = s.db.QueryRow(`SELECT role, status FROM users WHERE id = ?`, claims.UserID).Scan(&role, &status)
		if err == sql.ErrNoRows || status != "active" {
			writeJSON(w, http.StatusUnauthorized, map[string]string{"error": "账号不存在或已停用"})
			return
		}
		if err != nil {
			writeJSON(w, http.StatusInternalServerError, map[string]string{"error": "db error"})
			return
		}
		claims.Role = role
		ctx := context.WithValue(r.Context(), claimsKey, claims)
		next(w, r.WithContext(ctx))
	}
}

func (s *Server) admin(next http.HandlerFunc) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		role := currentClaims(r).Role
		if role != "admin" && role != "superadmin" {
			writeJSON(w, http.StatusForbidden, map[string]string{"error": "需要管理员权限"})
			return
		}
		next(w, r)
	}
}

func withCORS(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Access-Control-Allow-Origin", "*")
		w.Header().Set("Access-Control-Allow-Methods", "GET, POST, PUT, DELETE, OPTIONS")
		w.Header().Set("Access-Control-Allow-Headers", "Content-Type, Authorization")
		if r.Method == http.MethodOptions {
			w.WriteHeader(http.StatusNoContent)
			return
		}
		next.ServeHTTP(w, r)
	})
}

func writeJSON(w http.ResponseWriter, status int, v interface{}) {
	w.Header().Set("Content-Type", "application/json; charset=utf-8")
	w.WriteHeader(status)
	_ = json.NewEncoder(w).Encode(v)
}

func readJSON(r *http.Request, v interface{}) error {
	defer r.Body.Close()
	return json.NewDecoder(r.Body).Decode(v)
}

// handleSendRegisterCode 发送注册验证码。
func (s *Server) handleSendRegisterCode(w http.ResponseWriter, r *http.Request) {
	var req struct {
		Email string `json:"email"`
	}
	if err := readJSON(r, &req); err != nil {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": "invalid body"})
		return
	}
	req.Email = strings.TrimSpace(req.Email)
	if req.Email == "" {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": "邮箱不能为空"})
		return
	}
	if !allowRegistration(s.db) {
		writeJSON(w, http.StatusForbidden, map[string]string{"error": "注册未开放，请联系管理员"})
		return
	}
	if err := sendVerifyCode(s.db, req.Email, "register"); err != nil {
		writeJSON(w, http.StatusInternalServerError, map[string]string{"error": err.Error()})
		return
	}
	writeJSON(w, http.StatusOK, map[string]string{"status": "ok"})
}

// handleSendResetCode 发送密码重置验证码。
func (s *Server) handleSendResetCode(w http.ResponseWriter, r *http.Request) {
	var req struct {
		Email string `json:"email"`
	}
	if err := readJSON(r, &req); err != nil {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": "invalid body"})
		return
	}
	req.Email = strings.TrimSpace(req.Email)
	if req.Email == "" {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": "邮箱不能为空"})
		return
	}
	var id int64
	err := s.db.QueryRow(`SELECT id FROM users WHERE email = ?`, req.Email).Scan(&id)
	if err == sql.ErrNoRows {
		writeJSON(w, http.StatusNotFound, map[string]string{"error": "该邮箱未注册"})
		return
	}
	if err != nil {
		writeJSON(w, http.StatusInternalServerError, map[string]string{"error": "db error"})
		return
	}
	if err := sendVerifyCode(s.db, req.Email, "reset"); err != nil {
		writeJSON(w, http.StatusInternalServerError, map[string]string{"error": err.Error()})
		return
	}
	writeJSON(w, http.StatusOK, map[string]string{"status": "ok"})
}

// handleResetPassword 重置密码。
func (s *Server) handleResetPassword(w http.ResponseWriter, r *http.Request) {
	var req struct {
		Email    string `json:"email"`
		Code     string `json:"code"`
		Password string `json:"password"`
	}
	if err := readJSON(r, &req); err != nil {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": "invalid body"})
		return
	}
	req.Email = strings.TrimSpace(req.Email)
	if req.Email == "" || len(req.Password) < 6 {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": "邮箱不能为空，新密码至少 6 位"})
		return
	}
	if emailVerifyMode(s.db) != "none" {
		if !checkVerification(s.db, req.Email, req.Code, "reset") {
			writeJSON(w, http.StatusBadRequest, map[string]string{"error": "验证码错误或已过期"})
			return
		}
	}
	hash, err := hashPassword(req.Password)
	if err != nil {
		writeJSON(w, http.StatusInternalServerError, map[string]string{"error": "hash error"})
		return
	}
	res, err := s.db.Exec(`UPDATE users SET password_hash = ? WHERE email = ?`, hash, req.Email)
	if err != nil {
		writeJSON(w, http.StatusInternalServerError, map[string]string{"error": "db error"})
		return
	}
	if n, _ := res.RowsAffected(); n == 0 {
		writeJSON(w, http.StatusNotFound, map[string]string{"error": "该邮箱未注册"})
		return
	}
	logAction(s.db, 0, req.Email, "reset_password", "重置密码", clientIP(r))
	writeJSON(w, http.StatusOK, map[string]string{"status": "ok"})
}

// loadSiteConfig 从 meta 读取站点信息（页脚文字）。
func loadSiteConfig(db *sql.DB) SiteConfig {
	return SiteConfig{
		FooterText: getMeta(db, "site_footer_text"),
	}
}

// smtpConfigured 返回是否已配置 SMTP。
func smtpConfigured(db *sql.DB) bool {
	cfg := loadSMTPConfig(db)
	return cfg.Host != "" && cfg.Port != 0
}

// allowRegistration 返回是否允许注册：需显式开启且已配置 SMTP（才能发送验证码）。
func allowRegistration(db *sql.DB) bool {
	if getMeta(db, "allow_registration") != "true" {
		return false
	}
	return smtpConfigured(db)
}

// passwordMinLength 返回注册密码最小长度（默认 6）。
func passwordMinLength(db *sql.DB) int {
	if v := getMeta(db, "password_min_length"); v != "" {
		if n, err := strconv.Atoi(v); err == nil && n > 0 {
			return n
		}
	}
	return 6
}

// validatePassword 校验注册密码是否满足要求。
func validatePassword(db *sql.DB, password string) error {
	minLen := passwordMinLength(db)
	if len(password) < minLen {
		return errors.New("密码至少 " + strconv.Itoa(minLen) + " 位")
	}
	if getMeta(db, "password_require_complex") == "true" {
		hasLetter, hasDigit := false, false
		for _, c := range password {
			if (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') {
				hasLetter = true
			} else if c >= '0' && c <= '9' {
				hasDigit = true
			}
		}
		if !hasLetter || !hasDigit {
			return errors.New("密码需同时包含字母和数字")
		}
	}
	return nil
}

// handlePublicSettings 公开返回邮箱验证模式与站点信息（用于注册/找回密码页面及页脚）。
func (s *Server) handlePublicSettings(w http.ResponseWriter, r *http.Request) {
	writeJSON(w, http.StatusOK, map[string]interface{}{
		"email_verify_mode":        emailVerifyMode(s.db),
		"allow_registration":       allowRegistration(s.db),
		"password_min_length":      passwordMinLength(s.db),
		"password_require_complex": getMeta(s.db, "password_require_complex") == "true",
		"site":                     loadSiteConfig(s.db),
	})
}

// handleGetSettings 返回系统设置（SMTP 配置 + 邮箱验证模式 + 站点信息）。
func (s *Server) handleGetSettings(w http.ResponseWriter, r *http.Request) {
	writeJSON(w, http.StatusOK, map[string]interface{}{
		"smtp":                     loadSMTPConfig(s.db),
		"email_verify_mode":        emailVerifyMode(s.db),
		"allow_registration":       getMeta(s.db, "allow_registration") == "true",
		"password_min_length":      passwordMinLength(s.db),
		"password_require_complex": getMeta(s.db, "password_require_complex") == "true",
		"site":                     loadSiteConfig(s.db),
	})
}

// handleUpdateSettings 更新系统设置。
func (s *Server) handleUpdateSettings(w http.ResponseWriter, r *http.Request) {
	claims := currentClaims(r)
	var req struct {
		Host                   string      `json:"host"`
		Port                   int         `json:"port"`
		Username               string      `json:"username"`
		Password               string      `json:"password"`
		From                   string      `json:"from"`
		SSL                    bool        `json:"ssl"`
		Vendor                 string      `json:"vendor"`
		Mode                   string      `json:"mode"`
		AllowRegistration      *bool       `json:"allow_registration"`
		PasswordMinLength      int         `json:"password_min_length"`
		PasswordRequireComplex *bool       `json:"password_require_complex"`
		Site                   *SiteConfig `json:"site"`
	}
	if err := readJSON(r, &req); err != nil {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": "invalid body"})
		return
	}
	if req.Mode != "" {
		if req.Mode != "code" && req.Mode != "link" && req.Mode != "none" {
			writeJSON(w, http.StatusBadRequest, map[string]string{"error": "mode 必须为 code/link/none"})
			return
		}
		_ = setMeta(s.db, "email_verify_mode", req.Mode)
	}
	allowedVendor := map[string]bool{
		"qq": true, "126": true, "163": true, "gmail": true, "outlook": true, "custom": true,
	}
	if req.Vendor != "" {
		if !allowedVendor[req.Vendor] {
			writeJSON(w, http.StatusBadRequest, map[string]string{"error": "vendor 非法"})
			return
		}
		_ = setMeta(s.db, "smtp_vendor", req.Vendor)
	}
	_ = setMeta(s.db, "smtp_host", req.Host)
	_ = setMeta(s.db, "smtp_port", strconv.Itoa(req.Port))
	_ = setMeta(s.db, "smtp_username", req.Username)
	_ = setMeta(s.db, "smtp_from", req.From)
	if req.Password != "" {
		_ = setMeta(s.db, "smtp_password", req.Password)
	}
	if req.SSL {
		_ = setMeta(s.db, "smtp_ssl", "true")
	} else {
		_ = setMeta(s.db, "smtp_ssl", "false")
	}
	if req.AllowRegistration != nil {
		if *req.AllowRegistration {
			_ = setMeta(s.db, "allow_registration", "true")
		} else {
			_ = setMeta(s.db, "allow_registration", "false")
		}
	}
	if req.PasswordMinLength > 0 {
		_ = setMeta(s.db, "password_min_length", strconv.Itoa(req.PasswordMinLength))
	}
	if req.PasswordRequireComplex != nil {
		if *req.PasswordRequireComplex {
			_ = setMeta(s.db, "password_require_complex", "true")
		} else {
			_ = setMeta(s.db, "password_require_complex", "false")
		}
	}
	if req.Site != nil {
		_ = setMeta(s.db, "site_footer_text", req.Site.FooterText)
	}
	logAction(s.db, claims.UserID, claims.Username, "config_update", "更新系统设置", clientIP(r))
	writeJSON(w, http.StatusOK, map[string]string{"status": "ok"})
}

// handleListLogs 返回最近的操作日志（仅管理员）。
func (s *Server) handleListLogs(w http.ResponseWriter, r *http.Request) {
	rows, err := s.db.Query(`SELECT id, user_id, username, action, detail, ip, created_at FROM logs ORDER BY id DESC LIMIT 999`)
	if err != nil {
		writeJSON(w, http.StatusInternalServerError, map[string]string{"error": "db error"})
		return
	}
	defer rows.Close()
	list := []LogEntry{}
	for rows.Next() {
		var e LogEntry
		var created string
		if err := rows.Scan(&e.ID, &e.UserID, &e.Username, &e.Action, &e.Detail, &e.IP, &created); err != nil {
			writeJSON(w, http.StatusInternalServerError, map[string]string{"error": "db error"})
			return
		}
		e.CreatedAt = created
		list = append(list, e)
	}
	writeJSON(w, http.StatusOK, map[string]interface{}{"logs": list})
}

// getUser 按 ID 查询用户。
func (s *Server) getUser(id int64) (*User, error) {
	var u User
	var created string
	err := s.db.QueryRow(`SELECT id, username, email, role, status, created_at FROM users WHERE id = ?`, id).
		Scan(&u.ID, &u.Username, &u.Email, &u.Role, &u.Status, &created)
	if err != nil {
		return nil, err
	}
	u.CreatedAt = created
	u.Avatar = avatarName(u.ID)
	return &u, nil
}

// canManageUser 判断 operatorRole 是否有权管理 targetRole。
func canManageUser(operatorRole, targetRole string) bool {
	if operatorRole == "superadmin" {
		return true
	}
	if operatorRole == "admin" {
		return targetRole == "user" || targetRole == "admin"
	}
	return false
}

// ---- 头像 ----

func avatarDir() string {
	return filepath.Join(dataDir(), "uploads", "avatars")
}

// findAvatar 返回指定用户头像文件的路径，不存在则返回空字符串。
func findAvatar(userID int64) string {
	matches, _ := filepath.Glob(filepath.Join(avatarDir(), strconv.FormatInt(userID, 10)+".*"))
	if len(matches) > 0 {
		return matches[0]
	}
	return ""
}

// avatarName 返回用户头像文件名（如 1.png），无头像返回空字符串。
func avatarName(userID int64) string {
	if p := findAvatar(userID); p != "" {
		return filepath.Base(p)
	}
	return ""
}

// handleGetAvatar 返回用户头像（公开）。
func (s *Server) handleGetAvatar(w http.ResponseWriter, r *http.Request) {
	id, err := strconv.ParseInt(r.PathValue("id"), 10, 64)
	if err != nil {
		http.NotFound(w, r)
		return
	}
	p := findAvatar(id)
	if p == "" {
		http.NotFound(w, r)
		return
	}
	data, err := os.ReadFile(p)
	if err != nil {
		http.NotFound(w, r)
		return
	}
	ext := strings.TrimPrefix(strings.ToLower(filepath.Ext(p)), ".")
	ct := "image/" + ext
	if ext == "svg" {
		ct = "image/svg+xml"
	} else if ext == "jpg" {
		ct = "image/jpeg"
	}
	w.Header().Set("Content-Type", ct)
	w.Header().Set("Cache-Control", "no-cache")
	_, _ = w.Write(data)
}

// handleUploadAvatar 上传当前用户的头像（body: base64 数据 + 扩展名）。
func (s *Server) handleUploadAvatar(w http.ResponseWriter, r *http.Request) {
	claims := currentClaims(r)
	var req struct {
		Data string `json:"data"`
		Ext  string `json:"ext"`
	}
	if err := readJSON(r, &req); err != nil {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": "invalid body"})
		return
	}
	data, err := base64.StdEncoding.DecodeString(req.Data)
	if err != nil {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": "图片数据无效"})
		return
	}
	ext := strings.ToLower(strings.TrimPrefix(req.Ext, "."))
	allowed := map[string]bool{"png": true, "jpg": true, "jpeg": true, "gif": true, "svg": true, "webp": true}
	if !allowed[ext] {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": "不支持的图片格式"})
		return
	}
	if len(data) > 2*1024*1024 {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": "图片不能超过 2MB"})
		return
	}
	if err := os.MkdirAll(avatarDir(), 0o755); err != nil {
		writeJSON(w, http.StatusInternalServerError, map[string]string{"error": "db error"})
		return
	}
	if old := findAvatar(claims.UserID); old != "" {
		_ = os.Remove(old)
	}
	filename := strconv.FormatInt(claims.UserID, 10) + "." + ext
	if err := os.WriteFile(filepath.Join(avatarDir(), filename), data, 0o644); err != nil {
		writeJSON(w, http.StatusInternalServerError, map[string]string{"error": "db error"})
		return
	}
	writeJSON(w, http.StatusOK, map[string]string{"avatar": filename})
}
