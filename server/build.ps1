# CryPtBox 飞牛服务端一键打包脚本
# 用法：在 PowerShell 中运行 .\build.ps1
# 产物：fnos-server\cryptbox.fpk

$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $MyInvocation.MyCommand.Path
$FpkDir = Join-Path $Root "cryptbox"

Write-Host "==> 1/3 构建 web 前端 ..."
Push-Location (Join-Path $Root "web")
npm.cmd install | Out-Null
npm.cmd run build
if ($LASTEXITCODE -ne 0) { throw "前端构建失败" }
Pop-Location

Write-Host "==> 2/3 交叉编译 Linux 二进制 (amd64/arm64) ..."
Push-Location $Root
$env:CGO_ENABLED = "0"
$env:GOOS = "linux"
$env:GOARCH = "amd64"
go build -o (Join-Path $FpkDir "app\cryptbox-server") .
if ($LASTEXITCODE -ne 0) { throw "amd64 编译失败" }
$env:GOARCH = "arm64"
go build -o (Join-Path $FpkDir "app\cryptbox-server-arm64") .
if ($LASTEXITCODE -ne 0) { throw "arm64 编译失败" }
Pop-Location

Write-Host "==> 3/3 打包 fpk ..."
fnpack build -d $FpkDir
if ($LASTEXITCODE -ne 0) { throw "fpk 打包失败" }

Write-Host "完成！输出：$(Join-Path $Root 'cryptbox.fpk')"
