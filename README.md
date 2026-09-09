<div align="center">

<!-- 📸 宣传图占位（后续添加宣传图用）：
     1. 把图片放进 docs/ 目录，例如 docs/banner.png
     2. 删除下面两行注释符，即可在页面顶部显示
![CryPtBox 宣传图](docs/banner.png)
-->

# 密匣 CryPtBox

**本地加密 · 跨平台 · 可选云同步的密码管理器**

`Windows` `macOS (Apple Silicon / Intel)` `飞牛 fnOS` `AES-256-GCM` `Wails`

> 本仓库包含 **客户端**（本目录，Windows / macOS 原生应用）与 **[服务端](server/README.md)**（`server/` 目录，飞牛 fnOS 应用，提供 Web 管理后台与多端同步 API）。

</div>

---

## 特性

- **本地优先**：所有密码条目使用 AES-256-GCM 加密落盘，主密码通过 scrypt 派生密钥，永不存储
- **跨平台客户端**：一套代码，Windows 与 macOS 原生窗口（Wails），GitHub Actions 自动双端构建
- **一键复制**：网址 / 用户名 / 密码均支持一键复制，点击字段值也可直接复制
- **托盘常驻（Windows）**：点击关闭按钮最小化到系统托盘后台运行，托盘菜单可快速打开 / 退出；重复启动自动唤起已有窗口
- **导入导出**：CSV / TXT 双向导入导出，附带导入模板，方便从其他密码管理器迁移
- **浏览器密码迁移**：直接导入 Chrome / Edge 浏览器导出的密码 CSV（`name,url,username,password` 格式自动识别映射），从浏览器「设置 → 密码 → 导出」获取文件即可
- **云同步（可选）**：局域网自动扫描同步服务器，密码库一键上传 / 下载，服务端加密存储
- **多语言**：中文简体 / 繁体、英、日、韩、德、西、法、葡、俄

## 下载

| 平台 | 文件 | 说明 |
|------|------|------|
| Windows x64 | `CryPtBox-windows-amd64.zip` | 解压即用 |
| macOS (Universal) | `CryPtBox-macos.dmg` | 含 Intel + Apple Silicon |
| macOS (Universal) | `CryPtBox-macos.zip` | dmg 的备用格式 |

- **正式版**：[Releases](../../releases)（推送 `v*` 标签自动发布）
- **开发版**：[Actions](../../actions) 每次构建的 Artifacts 中下载

### macOS 首次运行

应用未做代码签名，首次打开如被 Gatekeeper 拦截，在终端执行：

```bash
xattr -cr /Applications/CryPtBox.app
chmod +x /Applications/CryPtBox.app/Contents/MacOS/CryPtBox
```

或在「系统设置 → 隐私与安全性」中点击「仍要打开」。

## 数据存放位置

| 平台 | 路径 |
|------|------|
| Windows | 可执行文件同目录 `passbook.db` |
| macOS | `~/Library/Application Support/CryPtBox/passbook.db` |

## 从源码构建

```bash
# 安装 Wails CLI（一次性）
go install github.com/wailsapp/wails/v2/cmd/wails@v2.15.0

# Windows
wails build -platform windows/amd64 -ldflags "-s -w"

# macOS（universal：同时含 amd64 + arm64）
wails build -platform darwin/universal -ldflags "-s -w"
```

构建产物在 `build/bin/`。推送 `v*` 标签或手动触发 workflow（`.github/workflows/build.yml`）即可在 GitHub Actions 上自动完成双端构建与发版。

## 云同步与服务端

客户端可连接部署在飞牛 fnOS 上的 **CryPtBox 服务端**（本仓库 `server/` 目录）实现多端同步：

- 支持统一网关访问：FN Connect 内网穿透 / DDNS / 局域网均可打开 Web 后台
- 支持局域网自动扫描发现服务端
- 安装包 `cryptbox.fpk` 与构建方法见 [server/README.md](server/README.md)

## 技术栈

Go + Wails v2 + Vue 3 + vue-i18n + SQLite（modernc 纯 Go 驱动，无 CGO）

<!-- 📸 更多截图占位（后续添加）：
| 主界面 | 同步设置 |
|--------|----------|
| ![主界面](docs/screenshot-main.png) | ![同步设置](docs/screenshot-sync.png) |
-->
