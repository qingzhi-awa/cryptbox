package main

import (
	"bytes"
	"encoding/csv"
	"errors"
	"strings"
	"unicode/utf8"

	"golang.org/x/text/encoding/simplifiedchinese"
	"golang.org/x/text/transform"
)

// decodeCSV 处理编码：UTF-8 直接使用，否则按 GBK（中文 Excel 常见）解码。
func decodeCSV(data []byte) ([]byte, error) {
	if utf8.Valid(data) {
		return data, nil
	}
	decoded, _, err := transform.Bytes(simplifiedchinese.GBK.NewDecoder(), data)
	if err != nil {
		return nil, errors.New("无法识别的文件编码，请保存为 UTF-8 或 GBK 的 CSV")
	}
	return decoded, nil
}

// normalizeHeader 规范化表头用于模糊匹配。
func normalizeHeader(s string) string {
	s = strings.TrimPrefix(s, "\ufeff") // 去掉 UTF-8 BOM
	s = strings.ToLower(strings.TrimSpace(s))
	r := strings.NewReplacer(" ", "", "_", "", "-", "", "(", "", ")", "", "/", "", "\\", "", ".", "", ":", "", "：", "")
	return r.Replace(s)
}

func matchField(h string) string {
	h = normalizeHeader(h)
	checks := []struct {
		field   string
		aliases []string
	}{
		{"password", []string{"password", "pass", "pwd", "密码", "口令", "密碼", "パスワード", "비밀번호", "contraseña", "senha", "пароль"}},
		{"username", []string{"username", "user", "login", "account", "用户名", "账号", "账户", "登录", "使用者名稱", "ユーザー名", "사용자명", "utilisateur", "benutzername", "usuario", "логин"}},
		{"url", []string{"url", "uri", "website", "网址", "網址", "链接", "地址"}},
		{"notes", []string{"note", "remark", "备注", "说明", "注释", "備註", "メモ", "메모", "notizen", "notas", "заметки"}},
		{"category", []string{"category", "group", "folder", "分类", "分组", "目录", "分類", "분류", "catégorie", "kategorie", "categoría", "категория", "categoria"}},
		{"title", []string{"title", "name", "名称", "标题", "网站", "站点", "应用", "服务", "標題", "タイトル", "제목", "titre", "titel", "título", "название"}},
	}
	for _, c := range checks {
		for _, alias := range c.aliases {
			if strings.Contains(h, normalizeHeader(alias)) {
				return c.field
			}
		}
	}
	return ""
}

// parseCSVEntries 解析 CSV 内容为密码条目。首行为表头，自动识别常见中英文列名。
func parseCSVEntries(data []byte) ([]Entry, error) {
	decoded, err := decodeCSV(data)
	if err != nil {
		return nil, err
	}
	reader := csv.NewReader(bytes.NewReader(decoded))
	reader.FieldsPerRecord = -1
	records, err := reader.ReadAll()
	if err != nil {
		return nil, err
	}
	if len(records) == 0 {
		return nil, errors.New("文件为空")
	}

	m := map[string]int{}
	for i, h := range records[0] {
		f := matchField(h)
		if f != "" {
			if _, ok := m[f]; !ok {
				m[f] = i
			}
		}
	}
	// 表头无法识别时，按常见顺序兜底：标题、用户名、密码、网址、备注、分类
	if _, ok := m["title"]; !ok {
		if _, ok2 := m["password"]; !ok2 {
			m["title"] = 0
			m["username"] = 1
			m["password"] = 2
			m["url"] = 3
			m["notes"] = 4
			m["category"] = 5
		}
	}

	get := func(row []string, key string) string {
		if i, ok := m[key]; ok && i < len(row) {
			return strings.TrimSpace(row[i])
		}
		return ""
	}

	var entries []Entry
	for _, row := range records[1:] {
		e := Entry{
			Title:    get(row, "title"),
			Username: get(row, "username"),
			Password: get(row, "password"),
			URL:      get(row, "url"),
			Category: get(row, "category"),
			Notes:    get(row, "notes"),
		}
		if e.Title == "" && e.Username == "" && e.Password == "" {
			continue
		}
		entries = append(entries, e)
	}
	return entries, nil
}

// isSepLine 判断是否为 TXT 导出中的分隔线（全由 - 或 = 组成）。
func isSepLine(s string) bool {
	s = strings.TrimSpace(s)
	if s == "" {
		return false
	}
	return strings.Trim(s, "-") == "" || strings.Trim(s, "=") == ""
}

// parseTXTEntries 解析 TXT 内容为密码条目，兼容本应用导出的「字段名: 值」格式。
func parseTXTEntries(data []byte) ([]Entry, error) {
	decoded, err := decodeCSV(data)
	if err != nil {
		return nil, err
	}
	text := strings.ReplaceAll(string(decoded), "\r\n", "\n")
	text = strings.ReplaceAll(text, "\r", "\n")
	lines := strings.Split(text, "\n")

	var entries []Entry
	var cur Entry
	hasCur := false

	flush := func() {
		if hasCur && (cur.Title != "" || cur.Username != "" || cur.Password != "") {
			entries = append(entries, cur)
		}
		cur = Entry{}
		hasCur = false
	}

	for _, line := range lines {
		line = strings.TrimSpace(line)
		if line == "" {
			continue
		}
		if isSepLine(line) {
			if strings.Contains(line, "-") {
				flush()
			}
			continue
		}
		idx := strings.IndexAny(line, ":：")
		if idx < 0 {
			continue
		}
		key := strings.TrimSpace(line[:idx])
		val := strings.TrimSpace(line[idx+1:])
		field := matchField(key)
		switch field {
		case "title":
			flush()
			cur.Title = val
			hasCur = true
		case "username":
			cur.Username = val
			hasCur = true
		case "password":
			cur.Password = val
			hasCur = true
		case "url":
			cur.URL = val
			hasCur = true
		case "category":
			cur.Category = val
			hasCur = true
		case "notes":
			cur.Notes = val
			hasCur = true
		}
	}
	flush()
	return entries, nil
}
