package main

import (
	"context"
	"database/sql"
	"encoding/base64"
	"encoding/csv"
	"errors"
	"os"
	"strings"
	"sync/atomic"
	"time"

	"github.com/wailsapp/wails/v2/pkg/runtime"
)

const verifierPlain = "passbook-verifier-v1"

// App 是暴露给前端调用的绑定对象。
type App struct {
	ctx      context.Context
	db       *sql.DB
	key      []byte // 解锁后持有的 AES-256 主密钥
	quitting atomic.Bool
}

func NewApp() *App {
	return &App{}
}

func (a *App) isQuitting() bool   { return a.quitting.Load() }
func (a *App) setQuitting()       { a.quitting.Store(true) }

func (a *App) startup(ctx context.Context) {
	a.ctx = ctx
	db, err := openStore()
	if err != nil {
		panic(err)
	}
	a.db = db
	// Windows 下启动系统托盘（后台常驻）
	startTray(a)
}

// beforeClose 拦截窗口关闭：Windows 下隐藏到托盘，其余平台正常退出。
func (a *App) beforeClose(ctx context.Context) bool {
	return platformBeforeClose(a)
}

// showMainWindow 显示并激活主窗口（托盘/二次启动调用）。
func showMainWindow(a *App) {
	if a.ctx == nil {
		return
	}
	runtime.WindowUnminimise(a.ctx)
	runtime.WindowShow(a.ctx)
}

// ---- 主密码 / 解锁 ----

// Init 返回是否已设置主密码。
func (a *App) Init() bool {
	return getMeta(a.db, "salt") != ""
}

// SetupMaster 首次设置主密码。
func (a *App) SetupMaster(password string) (bool, error) {
	if len(password) < 6 {
		return false, errors.New("主密码至少 6 位")
	}
	salt, err := newSalt()
	if err != nil {
		return false, err
	}
	key, err := deriveKey(password, salt)
	if err != nil {
		return false, err
	}
	verifier, err := encryptString(key, verifierPlain)
	if err != nil {
		return false, err
	}
	if err := setMeta(a.db, "salt", base64.StdEncoding.EncodeToString(salt)); err != nil {
		return false, err
	}
	if err := setMeta(a.db, "verifier", verifier); err != nil {
		return false, err
	}
	a.key = key
	return true, nil
}

// Unlock 用主密码解锁。
func (a *App) Unlock(password string) (bool, error) {
	saltB64 := getMeta(a.db, "salt")
	if saltB64 == "" {
		return false, errors.New("尚未初始化，请先设置主密码")
	}
	salt, err := base64.StdEncoding.DecodeString(saltB64)
	if err != nil {
		return false, err
	}
	key, err := deriveKey(password, salt)
	if err != nil {
		return false, err
	}
	plain, err := decryptString(key, getMeta(a.db, "verifier"))
	if err != nil || plain != verifierPlain {
		return false, nil // 主密码错误
	}
	a.key = key
	return true, nil
}

func (a *App) Lock() {
	a.key = nil
}

func (a *App) IsUnlocked() bool {
	return a.key != nil
}

// IsDebug 返回是否以调试模式启动（命令行参数 --debug）。
func (a *App) IsDebug() bool {
	return debugMode
}

func (a *App) requireUnlock() error {
	if a.key == nil {
		return errors.New("未解锁")
	}
	return nil
}

// ---- 密码条目 ----

func (a *App) ListEntries() ([]Entry, error) {
	if err := a.requireUnlock(); err != nil {
		return nil, err
	}
	rows, err := a.db.Query(`SELECT id, title, username, password_enc, url, category, notes_enc, created_at, updated_at FROM entries ORDER BY updated_at DESC`)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	list := []Entry{}
	for rows.Next() {
		var e Entry
		var pwEnc, notesEnc string
		if err := rows.Scan(&e.ID, &e.Title, &e.Username, &pwEnc, &e.URL, &e.Category, &notesEnc, &e.CreatedAt, &e.UpdatedAt); err != nil {
			return nil, err
		}
		e.Password, _ = decryptString(a.key, pwEnc)
		e.Notes, _ = decryptString(a.key, notesEnc)
		list = append(list, e)
	}
	return list, rows.Err()
}

func (a *App) SaveEntry(e Entry) (Entry, error) {
	if err := a.requireUnlock(); err != nil {
		return e, err
	}
	now := time.Now().Format(time.RFC3339)
	pwEnc, err := encryptString(a.key, e.Password)
	if err != nil {
		return e, err
	}
	notesEnc, err := encryptString(a.key, e.Notes)
	if err != nil {
		return e, err
	}
	if e.ID == 0 {
		e.CreatedAt = now
		e.UpdatedAt = now
		res, err := a.db.Exec(`INSERT INTO entries (title, username, password_enc, url, category, notes_enc, created_at, updated_at) VALUES (?,?,?,?,?,?,?,?)`,
			e.Title, e.Username, pwEnc, e.URL, e.Category, notesEnc, e.CreatedAt, e.UpdatedAt)
		if err != nil {
			return e, err
		}
		e.ID, _ = res.LastInsertId()
	} else {
		e.UpdatedAt = now
		_, err := a.db.Exec(`UPDATE entries SET title=?, username=?, password_enc=?, url=?, category=?, notes_enc=?, updated_at=? WHERE id=?`,
			e.Title, e.Username, pwEnc, e.URL, e.Category, notesEnc, e.UpdatedAt, e.ID)
		if err != nil {
			return e, err
		}
	}
	return e, nil
}

func (a *App) DeleteEntry(id int64) error {
	if err := a.requireUnlock(); err != nil {
		return err
	}
	_, err := a.db.Exec(`DELETE FROM entries WHERE id = ?`, id)
	return err
}

// ExportToTxt 将所有密码导出为 txt 文件，返回导出条数。
func (a *App) ExportToTxt(path string) (int, error) {
	list, err := a.ListEntries()
	if err != nil {
		return 0, err
	}
	var b strings.Builder
	b.WriteString("密匣导出\n")
	b.WriteString("导出时间: " + time.Now().Format("2006-01-02 15:04:05") + "\n")
	b.WriteString(strings.Repeat("=", 60) + "\n\n")
	for _, e := range list {
		b.WriteString("标题: " + e.Title + "\n")
		b.WriteString("用户名: " + e.Username + "\n")
		b.WriteString("密码: " + e.Password + "\n")
		b.WriteString("网址: " + e.URL + "\n")
		b.WriteString("分类: " + e.Category + "\n")
		b.WriteString("备注: " + e.Notes + "\n")
		b.WriteString(strings.Repeat("-", 60) + "\n")
	}
	if err := os.WriteFile(path, []byte(b.String()), 0o600); err != nil {
		return 0, err
	}
	return len(list), nil
}

// ExportDialog 弹出系统保存对话框并导出 txt，返回 (路径, 条数, 错误)。
func (a *App) ExportDialog() (string, int, error) {
	path, err := runtime.SaveFileDialog(a.ctx, runtime.SaveDialogOptions{
		Title:           "导出密码",
		DefaultFilename: "passbook.txt",
	})
	if err != nil {
		return "", 0, err
	}
	if path == "" {
		return "", 0, nil
	}
	n, err := a.ExportToTxt(path)
	return path, n, err
}

// ExportCSV 导出 CSV 密码文件，返回条数。
func (a *App) ExportCSV(path string) (int, error) {
	list, err := a.ListEntries()
	if err != nil {
		return 0, err
	}
	f, err := os.Create(path)
	if err != nil {
		return 0, err
	}
	defer f.Close()
	// 写入 UTF-8 BOM，便于 Excel 正确识别中文
	if _, err := f.WriteString("\ufeff"); err != nil {
		return 0, err
	}
	w := csv.NewWriter(f)
	if err := w.Write([]string{"name", "username", "password", "url", "category", "notes"}); err != nil {
		return 0, err
	}
	for _, e := range list {
		if err := w.Write([]string{e.Title, e.Username, e.Password, e.URL, e.Category, e.Notes}); err != nil {
			return 0, err
		}
	}
	w.Flush()
	if err := w.Error(); err != nil {
		return 0, err
	}
	return len(list), nil
}

// ExportCSVDialog 弹出保存对话框并导出 CSV，返回 (路径, 条数, 错误)。
func (a *App) ExportCSVDialog() (string, int, error) {
	path, err := runtime.SaveFileDialog(a.ctx, runtime.SaveDialogOptions{
		Title:           "导出密码 CSV",
		DefaultFilename: "passbook.csv",
	})
	if err != nil {
		return "", 0, err
	}
	if path == "" {
		return "", 0, nil
	}
	n, err := a.ExportCSV(path)
	return path, n, err
}

// DownloadTemplateDialog 下载 CSV 导入模板，返回保存路径。
func (a *App) DownloadTemplateDialog() (string, error) {
	path, err := runtime.SaveFileDialog(a.ctx, runtime.SaveDialogOptions{
		Title:           "下载 CSV 模板",
		DefaultFilename: "passbook-template.csv",
	})
	if err != nil {
		return "", err
	}
	if path == "" {
		return "", nil
	}
	content := "\ufeffname,username,password,url,category,notes\n"
	if err := os.WriteFile(path, []byte(content), 0o600); err != nil {
		return "", err
	}
	return path, nil
}

// SaveTextFile 弹出保存对话框并写入文本内容，返回保存路径。
func (a *App) SaveTextFile(content, filename string) (string, error) {
	path, err := runtime.SaveFileDialog(a.ctx, runtime.SaveDialogOptions{
		Title:           "保存文件",
		DefaultFilename: filename,
	})
	if err != nil {
		return "", err
	}
	if path == "" {
		return "", nil
	}
	if err := os.WriteFile(path, []byte(content), 0o600); err != nil {
		return "", err
	}
	return path, nil
}

// ImportCSV 从 CSV 文件批量导入密码，返回导入条数。
func (a *App) ImportCSV(path string) (int, error) {
	if err := a.requireUnlock(); err != nil {
		return 0, err
	}
	data, err := os.ReadFile(path)
	if err != nil {
		return 0, err
	}
	entries, err := parseCSVEntries(data)
	if err != nil {
		return 0, err
	}
	count := 0
	for _, e := range entries {
		if _, err := a.SaveEntry(e); err != nil {
			return count, err
		}
		count++
	}
	return count, nil
}

// ImportDialog 弹出文件选择框并导入 CSV，返回导入条数。
func (a *App) ImportDialog() (int, error) {
	path, err := runtime.OpenFileDialog(a.ctx, runtime.OpenDialogOptions{
		Title: "导入密码（支持 Chrome / Edge 导出的 CSV）",
		Filters: []runtime.FileFilter{
			{DisplayName: "密码 CSV（Chrome / Edge / 密匣）", Pattern: "*.csv"},
			{DisplayName: "所有文件 (*.*)", Pattern: "*.*"},
		},
	})
	if err != nil {
		return 0, err
	}
	if path == "" {
		return 0, nil
	}
	return a.ImportCSV(path)
}

// ImportText 从 TXT 文件批量导入密码，返回导入条数。
func (a *App) ImportText(path string) (int, error) {
	if err := a.requireUnlock(); err != nil {
		return 0, err
	}
	data, err := os.ReadFile(path)
	if err != nil {
		return 0, err
	}
	entries, err := parseTXTEntries(data)
	if err != nil {
		return 0, err
	}
	count := 0
	for _, e := range entries {
		if _, err := a.SaveEntry(e); err != nil {
			return count, err
		}
		count++
	}
	return count, nil
}

// ImportTextDialog 弹出文件选择框并导入 TXT，返回导入条数。
func (a *App) ImportTextDialog() (int, error) {
	path, err := runtime.OpenFileDialog(a.ctx, runtime.OpenDialogOptions{
		Title: "导入密码",
		Filters: []runtime.FileFilter{
			{DisplayName: "TXT 文件 (*.txt)", Pattern: "*.txt"},
			{DisplayName: "所有文件 (*.*)", Pattern: "*.*"},
		},
	})
	if err != nil {
		return 0, err
	}
	if path == "" {
		return 0, nil
	}
	return a.ImportText(path)
}

// ---- 同步 ----

func (a *App) SyncRegister(server, username, password, email, code string) (map[string]string, error) {
	c := NewSyncClient(server)
	r, err := c.Register(username, password, email, code)
	if err != nil {
		return nil, err
	}
	_ = setMeta(a.db, "server_url", server)
	_ = setMeta(a.db, "server_username", username)
	return map[string]string{"token": r.Token, "avatar": r.Avatar}, nil
}

// SendRegisterCode 请求服务端发送注册验证码。
func (a *App) SendRegisterCode(server, email string) error {
	c := NewSyncClient(server)
	return c.SendRegisterCode(email)
}

// SyncCheck 校验已保存的 token 是否有效。
func (a *App) SyncCheck(server, token string) error {
	c := NewSyncClient(server)
	c.Token = token
	return c.Check()
}

func (a *App) SyncLogin(server, username, password string) (map[string]string, error) {
	c := NewSyncClient(server)
	r, err := c.Login(username, password)
	if err != nil {
		return nil, err
	}
	_ = setMeta(a.db, "server_url", server)
	_ = setMeta(a.db, "server_username", username)
	return map[string]string{"token": r.Token, "avatar": r.Avatar}, nil
}

func (a *App) GetServerConfig() map[string]string {
	return map[string]string{
		"server":   getMeta(a.db, "server_url"),
		"username": getMeta(a.db, "server_username"),
	}
}

// PushVault 将本地密码条目整体上传到服务器（服务端加密存储）。
func (a *App) PushVault(server, token string) error {
	list, err := a.ListEntries()
	if err != nil {
		return err
	}
	c := NewSyncClient(server)
	c.Token = token
	return c.Push(list)
}

// PullVault 从服务器下载密码条目并替换本地数据，返回条数。
func (a *App) PullVault(server, token string) (int, error) {
	c := NewSyncClient(server)
	c.Token = token
	list, err := c.Pull()
	if err != nil {
		return 0, err
	}
	if err := a.replaceEntries(list); err != nil {
		return 0, err
	}
	return len(list), nil
}

func (a *App) replaceEntries(list []Entry) error {
	tx, err := a.db.Begin()
	if err != nil {
		return err
	}
	if _, err := tx.Exec(`DELETE FROM entries`); err != nil {
		_ = tx.Rollback()
		return err
	}
	for _, e := range list {
		pwEnc, err := encryptString(a.key, e.Password)
		if err != nil {
			_ = tx.Rollback()
			return err
		}
		notesEnc, err := encryptString(a.key, e.Notes)
		if err != nil {
			_ = tx.Rollback()
			return err
		}
		_, err = tx.Exec(`INSERT INTO entries (id, title, username, password_enc, url, category, notes_enc, created_at, updated_at) VALUES (?,?,?,?,?,?,?,?,?)`,
			e.ID, e.Title, e.Username, pwEnc, e.URL, e.Category, notesEnc, e.CreatedAt, e.UpdatedAt)
		if err != nil {
			_ = tx.Rollback()
			return err
		}
	}
	return tx.Commit()
}
