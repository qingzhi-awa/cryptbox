//go:build linux

package main

import (
	"fmt"
	"os"
	"path/filepath"
)

// setAutoStart 通过 XDG autostart 桌面项实现 Linux 开机自启动（兼容 GNOME/KDE/UKUI/DDE 等）。
func setAutoStart(enable bool) error {
	cfg, err := os.UserConfigDir() // ~/.config
	if err != nil {
		return err
	}
	dir := filepath.Join(cfg, "autostart")
	if err := os.MkdirAll(dir, 0o755); err != nil {
		return err
	}
	p := filepath.Join(dir, "cryptbox.desktop")
	if !enable {
		_ = os.Remove(p)
		return nil
	}
	exe, err := os.Executable()
	if err != nil {
		return err
	}
	content := fmt.Sprintf(`[Desktop Entry]
Type=Application
Name=CryPtBox
Exec=%q
Terminal=false
X-GNOME-Autostart-enabled=true
`, exe)
	return os.WriteFile(p, []byte(content), 0o644)
}
