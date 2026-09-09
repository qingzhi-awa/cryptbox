//go:build windows

package main

import (
	"os"

	"golang.org/x/sys/windows/registry"
)

const runKeyPath = `Software\Microsoft\Windows\CurrentVersion\Run`
const runValueName = "CryPtBox"

// setAutoStart 通过 HKCU 注册表 Run 项实现 Windows 开机自启动。
func setAutoStart(enable bool) error {
	k, err := registry.OpenKey(registry.CURRENT_USER, runKeyPath, registry.SET_VALUE|registry.QUERY_VALUE)
	if err != nil {
		return err
	}
	defer k.Close()
	if !enable {
		_ = k.DeleteValue(runValueName) // 不存在时忽略
		return nil
	}
	exe, err := os.Executable()
	if err != nil {
		return err
	}
	return k.SetStringValue(runValueName, `"`+exe+`"`)
}
