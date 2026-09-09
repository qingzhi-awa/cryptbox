# 密匣服务端（CryPtBox Server for fnOS）

CryPtBox 密匣的飞牛 fnOS 服务端，提供密码管理 Web 后台与多端同步 API。与 [CryPtBox 客户端](../README.md)（Windows / macOS / Linux，含统信 UOS、银河麒麟等国产系统）配合使用。

## 功能

- 密码管理：添加 / 编辑 / 删除 / 分类，AES-256-GCM 加密存储
- 多用户：管理员后台、自助注册（SMTP 邮箱验证码，可关闭）、头像上传
- 同步 API：客户端密码库一键上传 / 下载
- 导入导出：CSV / TXT 导入导出、用户批量导入（附模板）
- 操作日志：登录、密码操作、用户管理、设置变更全留痕（最多 999 条 / 365 天，逐条淘汰）
- 站点设置：标题、页脚、ICP / 公安备案号

## 支持环境

| 项目 | 要求 |
|------|------|
| 系统 | 飞牛 fnOS（支持统一网关访问， FN Connect / DDNS 内网穿透下桌面图标可直接打开） |
| 架构 | x86_64 (amd64) / ARM64 |
| 端口 | 5201（同时兼容统一网关反代与局域网直连） |
| 数据 | SQLite，存储于应用 var 目录（升级自动备份，卸载可选择是否保留） |

## 安装

在飞牛应用中心手动上传 `cryptbox.fpk` 安装。

- Web 管理后台：桌面「密匣」图标，或 `http://NAS_IP:5201`
- 首次访问按提示创建超级管理员

## 从源码构建

```powershell
# 需要 Go 1.22+ 与 Node.js（构建 web 前端并打包 fpk）
.\build.ps1
```

产物：`cryptbox.fpk`（含 amd64 / arm64 双架构二进制）。

## 技术栈

Go（标准库 net/http + modernc SQLite 纯 Go 驱动，无 CGO） + Vue 3 + vue-i18n（10 语言）

## 开源许可

基于 [Apache License 2.0](../LICENSE) 开源。
