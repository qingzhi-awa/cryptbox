package main

import "testing"

// 验证按时间戳记录级合并与墓碑传播。
func TestMergeEntries(t *testing.T) {
	local := []Entry{
		{ID: 1, Title: "本地新", UpdatedAt: "2026-09-09T10:00:00+08:00"},
		{ID: 3, Title: "仅本地", UpdatedAt: "2026-09-09T08:00:00+08:00"},
		{ID: 4, Title: "已删", UpdatedAt: "2026-09-09T12:00:00+08:00", Deleted: true},
	}
	remote := []Entry{
		{ID: 1, Title: "服务端旧", UpdatedAt: "2026-09-09T09:00:00+08:00"},
		{ID: 2, Title: "仅服务端", UpdatedAt: "2026-09-09T08:00:00+08:00"},
		{ID: 4, Title: "已删旧", UpdatedAt: "2026-09-09T11:00:00+08:00", Deleted: false},
	}
	merged := mergeEntries(local, remote)

	m := map[int64]Entry{}
	for _, e := range merged {
		m[e.ID] = e
	}

	// id=1 本地更新时间较新，保留本地标题
	if m[1].Title != "本地新" {
		t.Fatalf("id=1 应保留本地较新记录，got %+v", m[1])
	}
	// id=2 仅服务端，保留
	if m[2].Title != "仅服务端" {
		t.Fatalf("id=2 应保留服务端记录，got %+v", m[2])
	}
	// id=3 仅本地，保留
	if m[3].Title != "仅本地" {
		t.Fatalf("id=3 应保留本地记录，got %+v", m[3])
	}
	// id=4 本地墓碑更新时间更晚，应保持删除标记
	if !m[4].Deleted {
		t.Fatalf("id=4 应传播删除墓碑，got %+v", m[4])
	}
	if len(merged) != 4 {
		t.Fatalf("合并后应保留 4 条，got %d", len(merged))
	}
}

func TestNewer(t *testing.T) {
	if !newer("2026-09-09T10:00:00+08:00", "2026-09-09T09:00:00+08:00") {
		t.Fatal("较晚时间应判定为 newer")
	}
	// 跨时区但同一时刻：+08:00 的 10:00 == +09:00 的 11:00
	if newer("2026-09-09T10:00:00+08:00", "2026-09-09T11:00:00+09:00") {
		t.Fatal("同一时刻不同时区不应误判 newer")
	}
}
