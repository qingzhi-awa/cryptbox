//go:build !windows

package main

// 非 Windows 平台暂未实现系统托盘：点击关闭按钮正常退出。
func startTray(app *App)                {}
func platformBeforeClose(app *App) bool { return false }
func traySupported() bool               { return false }
