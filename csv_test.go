package main

import "testing"

// 验证 Chrome / Edge 浏览器导出的 CSV 能被正确识别导入
func TestParseCSVEntriesBrowserFormats(t *testing.T) {
	cases := []struct {
		name string
		data string
	}{
		{"Chrome", "name,url,username,password\r\nGitHub,https://github.com,tom,pass1\r\n\"Google, Inc.\",https://google.com,jerry,pass2\r\n"},
		{"Edge", "name,url,username,password\r\n微博,https://weibo.com,alice,pw-1\r\n"},
		{"Chrome带备注", "name,url,username,password,note\r\nGitHub,https://github.com,tom,pass1,some note\r\n"},
	}
	for _, c := range cases {
		es, err := parseCSVEntries([]byte(c.data))
		if err != nil {
			t.Fatalf("%s: %v", c.name, err)
		}
		if len(es) != 2 && len(es) != 1 {
			t.Fatalf("%s: 条目数错误: %d", c.name, len(es))
		}
		e := es[0]
		if e.Title == "" || e.URL == "" || e.Username == "" || e.Password == "" {
			t.Fatalf("%s: 字段映射失败: %+v", c.name, e)
		}
		t.Logf("%s OK: %+v", c.name, e)
	}
}
