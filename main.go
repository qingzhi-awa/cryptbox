package main

import (
	"embed"
	"os"

	"github.com/wailsapp/wails/v2"
	"github.com/wailsapp/wails/v2/pkg/options"
	"github.com/wailsapp/wails/v2/pkg/options/assetserver"
	"github.com/wailsapp/wails/v2/pkg/options/windows"
)

//go:embed all:frontend/dist
var assets embed.FS

// debugMode 通过命令行参数 --debug 开启前端调试诊断框。
var debugMode bool

func main() {
	for _, arg := range os.Args[1:] {
		if arg == "--debug" || arg == "-debug" {
			debugMode = true
		}
	}

	app := NewApp()

	err := wails.Run(&options.App{
		Title:  "CryPtBox",
		Width:  1024,
		Height: 720,
		AssetServer: &assetserver.Options{
			Assets: assets,
		},
		BackgroundColour: &options.RGBA{R: 27, G: 38, B: 54, A: 1},
		OnStartup:        app.startup,
		Windows: &windows.Options{
			WebviewGpuIsDisabled:                true,
			WebviewDisableRendererCodeIntegrity: true,
		},
		Bind: []interface{}{
			app,
		},
	})
	if err != nil {
		println("Error:", err.Error())
	}
}
