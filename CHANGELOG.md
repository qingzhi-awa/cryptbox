# 更新日志

本文件记录密匣 CryPtBox 各版本的发布说明。项目遵循 [语义化版本](https://semver.org/lang/zh-CN/) 规范。

---

## v0.1.0（2026-09-09）· 首次正式发版

密匣 CryPtBox 首个正式版本发布。这是一个**本地优先、跨平台、可选云同步**的密码管理器，客户端与服务端共同开源。

### 客户端

- **Windows x64**
  - 安装版 `CryPtBox-setup.exe`（NSIS，含开始菜单 / 桌面快捷方式 / 卸载器）
  - 便携版 `CryPtBox-portable.zip`（解压即用，数据随 exe 存放）
  - 安装版首次启动自动迁移旧便携数据到 `%APPDATA%\CryPtBox`
- **macOS（Universal）**：`CryPtBox-macos.dmg` / `.zip`，同时支持 Apple Silicon 与 Intel
- **Linux x64**：`CryPtBox-linux-amd64.tar.gz`，兼容 Ubuntu / Debian 及统信 UOS、银河麒麟、中标麒麟等国产操作系统
- 托盘常驻（Windows）：关闭窗口最小化到系统托盘后台运行，重复启动自动唤起已有窗口
- 开机自启、解锁后自动同步
- 三种同步策略：本地优先 / 服务端优先 / 按时间戳记录级合并
- 导入 Chrome / Edge 浏览器导出的密码 CSV
- CSV / TXT 导入导出（附导入模板）
- 10 语言界面：简体 / 繁体、英、日、韩、德、西、法、葡、俄

### 服务端（飞牛 fnOS）

- Web 管理后台 + 多端同步 API
- 多用户与角色权限、自助注册（SMTP 邮箱验证，可关闭）、头像上传
- 操作日志：登录、密码操作、用户管理、设置变更全留痕（最多 999 条 / 365 天，逐条淘汰）
- 支持统一网关访问：FN Connect 内网穿透 / DDNS / 局域网均可打开
- amd64 / arm64 双架构，监听端口 5201

### 安全与同步

- AES-256-GCM 本地加密落盘，主密码经 scrypt 派生，永不存储
- 记录级时间戳合并同步，软删除墓碑传播删除
- 服务端加密存储，保留客户端条目 ID 作为合并键

### 说明

- macOS 应用未做代码签名，首次运行如被 Gatekeeper 拦截，请按 [README 说明](README.md#macos-首次运行) 放行
- Windows 安装包与便携版共用同一份源码，由 GitHub Actions 自动构建
- 飞牛服务端安装包 `cryptbox.fpk` 及构建方式见 [server/README.md](server/README.md)

---

> 后续版本将在此基础上追加，格式保持「版本号（日期）· 主题 + 分平台/模块变更要点」。
