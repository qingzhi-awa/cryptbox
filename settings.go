package main

// AppSettings 应用设置，持久化于 meta 表：
//   set_autostart  开机自启动
//   set_autosync   启动解锁后自动与服务端同步
//   set_priority   同步覆盖策略：local（本地优先=上传覆盖服务端）| server（服务端优先=下载覆盖本地）

// GetSettings 返回当前设置（值均为字符串："1"/"0"、"local"/"server"）。
func (a *App) GetSettings() map[string]string {
	return map[string]string{
		"autostart": getMeta(a.db, "set_autostart"),
		"autosync":  getMeta(a.db, "set_autosync"),
		"priority":  getMeta(a.db, "set_priority"),
	}
}

// SaveSettings 保存设置；开机自启变化时同步注册/注销系统自启动项。
func (a *App) SaveSettings(autostart, autosync bool, priority string) error {
	if priority != "local" && priority != "server" {
		priority = "local"
	}
	if err := setMeta(a.db, "set_autostart", boolStr(autostart)); err != nil {
		return err
	}
	if err := setMeta(a.db, "set_autosync", boolStr(autosync)); err != nil {
		return err
	}
	if err := setMeta(a.db, "set_priority", priority); err != nil {
		return err
	}
	return setAutoStart(autostart)
}

func boolStr(b bool) string {
	if b {
		return "1"
	}
	return "0"
}
