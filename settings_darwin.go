//go:build darwin

package main

import (
	"fmt"
	"os"
	"path/filepath"
)

const launchAgentLabel = "com.cryptbox.passbook"

// setAutoStart 通过 LaunchAgents plist 实现 macOS 开机自启动。
func setAutoStart(enable bool) error {
	home, err := os.UserHomeDir()
	if err != nil {
		return err
	}
	dir := filepath.Join(home, "Library", "LaunchAgents")
	if err := os.MkdirAll(dir, 0o755); err != nil {
		return err
	}
	p := filepath.Join(dir, launchAgentLabel+".plist")
	if !enable {
		_ = os.Remove(p)
		return nil
	}
	exe, err := os.Executable()
	if err != nil {
		return err
	}
	xml := fmt.Sprintf(`<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>%s</string>
    <key>ProgramArguments</key>
    <array>
        <string>%s</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
</dict>
</plist>
`, launchAgentLabel, exe)
	return os.WriteFile(p, []byte(xml), 0o644)
}
