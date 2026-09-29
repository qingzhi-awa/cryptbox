<div align="center">

<!-- 📸 宣传图占位（后续添加宣传图用）：
     1. 把图片放进 docs/ 目录，例如 docs/banner.png
     2. 删除下面两行注释符，即可在页面顶部显示
![CryPtBox 宣传图](docs/banner.png)
-->

# 密匣 CryPtBox

**本地加密 · 跨平台 · 可选云同步的密码管理器**

`Windows` `macOS` `Linux` `Tauri 2` `Rust` `Vue 3` `AES-256-GCM` `飞牛 fnOS`

> 本仓库为 **CryPtBox 桌面客户端**（Tauri 2 + Vue 3，跨平台原生应用）。飞牛端服务端见 [cryptbox-fn](https://github.com/qingzhi-awa/cryptbox-fn)。

</div>

---

## 特性

- **本地优先**：所有密码条目使用 AES-256-GCM 加密落盘，主密码通过 scrypt 派生密钥，永不存储
- **主密码解锁**：单一主密码解锁本地密码库，锁定后主密钥即从内存清除
- **密码管理**：增删改查、分类、备注，网址 / 用户名 / 密码均支持一键复制
- **回收站**：删除数据走软删除，可单条恢复 / 彻底删除 / 一键清空，按保留天数自动清理过期数据
- **导入导出**：CSV / TXT 双向导入导出，附带导入模板，方便从其他密码管理器迁移
- **浏览器密码迁移**：直接导入 Chrome / Edge 导出的密码 CSV（`name,url,username,password` 格式自动识别映射）
- **云同步（可选）**：连接飞牛端服务端，支持登录 / 注册、一键上传 / 下载 / 合并，支持局域网自动扫描与域名 / 公网地址连接
- **托盘常驻**：关闭窗口最小化到系统托盘，托盘菜单可快速打开 / 退出；重复启动自动唤起已有窗口
- **多语言**：中文简体 / 繁体、英、日、韩、德、西、法、葡、俄 10 种语言

## 下载

| 平台 | 文件 | 说明 |
|------|------|------|
| Windows x64 | `CryPtBox_0.2.3_x64-setup.exe` | NSIS 安装包（中文） |
| Windows x64 | `CryPtBox_0.2.3_x64_zh-CN.msi` | MSI 安装包（中文） |
| Windows x64 | `CryPtBox_0.2.3_x64-setup.en.exe` | NSIS 安装包（英文） |
| Windows x64 | `CryPtBox_0.2.3_x64_en-US.msi` | MSI 安装包（英文） |

> 正式版见 [Releases](../../releases)。macOS / Linux 需在对应平台执行 `npx tauri build` 构建。

## 数据存放位置

| 平台 | 路径 |
|------|------|
| Windows（安装版） | `%APPDATA%\CryPtBox\app.db` |
| Windows（便携版） | 可执行文件同目录 `app.db`（exe 旁带 `portable.flag`） |
| macOS | `~/Library/Application Support/CryPtBox/app.db` |
| Linux | `~/.config/CryPtBox/app.db` |

> 旧版本数据库名为 `passbook.db`，升级后自动重命名为 `app.db`，无需手动迁移。

## 从源码构建

```bash
# 安装前端依赖
npm install

# 前端构建 + 打包（Windows：NSIS + MSI）
npx tauri build

# 英文版
npx tauri build --config src-tauri/tauri.en.conf.json
```

构建产物在 `src-tauri/target/release/bundle/`。

## 云同步与服务端

客户端可连接部署在飞牛 fnOS 上的 **CryPtBox 服务端** 实现多端同步，服务端源码见 [cryptbox-fn](https://github.com/qingzhi-awa/cryptbox-fn)。

- 支持局域网自动扫描发现服务端
- 支持域名 / 公网地址（HTTPS）连接
- 服务端支持 FN Connect / DDNS / 局域网访问

## 技术栈

Rust · Tauri 2 · Vue 3 · vue-i18n · SQLite（rusqlite bundled）· AES-256-GCM · scrypt

## 更新日志

各版本发布说明见 [Releases](../../releases)。

<!-- 📸 更多截图占位（后续添加）：
| 主界面 | 同步设置 |
|--------|----------|
| ![主界面](docs/screenshot-main.png) | ![同步设置](docs/screenshot-sync.png) |
-->
