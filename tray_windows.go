//go:build windows

package main

import (
	_ "embed"

	"github.com/getlantern/systray"
	"github.com/wailsapp/wails/v2/pkg/runtime"
)

//go:embed build/windows/icon.ico
var trayIconICO []byte

// startTray 启动 Windows 系统托盘：图标常驻，左键点击打开，菜单提供「打开」「退出」。
func startTray(app *App) {
	go systray.Run(func() {
		systray.SetIcon(trayIconICO)
		systray.SetTitle("CryPtBox")
		systray.SetTooltip("CryPtBox 密匣 - 后台常驻")
		mShow := systray.AddMenuItem("打开 密匣", "显示主窗口")
		systray.AddSeparator()
		mQuit := systray.AddMenuItem("退出", "退出应用程序")
		go func() {
			for range mShow.ClickedCh {
				showMainWindow(app)
			}
		}()
		go func() {
			for range mQuit.ClickedCh {
				quitApp(app)
			}
		}()
	}, nil)
}

func quitApp(app *App) {
	app.setQuitting()
	systray.Quit()
	runtime.Quit(app.ctx)
}

// platformBeforeClose Windows 下点击关闭按钮时隐藏到托盘，返回 true 阻止默认退出。
func platformBeforeClose(app *App) bool {
	if app.isQuitting() {
		return false
	}
	if app.ctx != nil {
		runtime.WindowHide(app.ctx)
	}
	return true
}

func traySupported() bool { return true }
