<div align="center">

<!-- 📸 宣传图占位（后续添加宣传图用）：
     1. 把图片放进 docs/ 目录，例如 docs/banner.png
     2. 删除下面两行注释符，即可在页面顶部显示
![CryPtBox 宣传图](docs/banner.png)
-->

# 密匣 CryPtBox

**本地优先 · 端到端加密 · 可选云同步的跨平台密码管理器**

`Windows` `macOS` `Linux` `Tauri 2` `Rust` `Vue 3` `AES-256-GCM` `scrypt` `飞牛 fnOS`

> 本仓库为 **CryPtBox 桌面客户端**（Tauri 2 + Rust + Vue 3，跨平台原生应用）。飞牛端服务端见 [cryptbox-fn](https://github.com/qingzhi-awa/cryptbox-fn)。

</div>

---

## 特性

### 密码管理

- **本地优先**：所有密码条目使用 AES-256-GCM 加密落盘，主密码经 scrypt（N=2¹⁵, r=8, p=1）派生密钥，永不存储
- **主密码解锁**：单一主密码解锁本地密码库，锁定后主密钥即从内存清除
- **条目操作**：增删改查、分类、备注，网址 / 用户名 / 密码均支持一键复制
- **置顶与排序**：条目可置顶，支持拖拽调整顺序，偏好随密码库保存
- **回收站**：删除数据走软删除，可单条恢复 / 彻底删除 / 一键清空，按保留天数自动清理过期数据

### 导入导出

- **CSV / TXT 双向导入导出**，附带导入模板，方便从其他密码管理器迁移
- **浏览器密码迁移**：直接导入 Chrome / Edge 导出的密码 CSV（`name,url,username,password` 格式自动识别映射）
- **公式注入防护**：导出 CSV 时对 `=` `+` `-` `@` 等开头的字段前置单引号，避免被 Excel / WPS 当公式执行

### 云同步（可选）

- **连接飞牛端服务端**：支持登录 / 注册（邮箱验证码）、一键上传 / 下载 / 合并密码库
- **发现方式**：局域网自动扫描，或手动填写域名 / 公网地址
- **默认加密传输**：同步地址默认 HTTPS，并校验服务器证书指纹（首次连接需确认，证书变更即拒绝）
- **跨设备置顶**：账号级「置顶参与同步」开关，开启后置顶状态随密码库同步到所有设备
- **凭据安全**：登录令牌由 Rust 侧保管并存于系统凭据库，前端不持有原始 JWT；头像经带认证的请求代理拉取

### 体验

- **托盘常驻**：关闭窗口最小化到系统托盘，托盘菜单可快速打开 / 退出；重复启动自动唤起已有窗口
- **主题**：自动 / 浅色 / 深色三种颜色模式
- **便携模式**：可执行文件旁放置 `portable.flag`，数据落在 exe 同目录，适合随身 U 盘
- **无法解锁恢复**：服务端改密后可选择「用旧密码恢复原密码库」或「清空后重新开始」
- **多语言**：中文简体 / 繁体、英、日、韩、德、西、法、葡、俄 10 种语言

## 下载

| 平台 | 文件 | 说明 |
|------|------|------|
| Windows x64 | `CryPtBox_0.2.3_x64-setup.exe` | NSIS 安装包 |
| Windows x64 | `CryPtBox_0.2.3_x64_zh-CN.msi` | MSI 安装包 |
| Windows x64 | `cryptbox.exe` | 绿色单文件（可配合 `portable.flag` 便携使用） |

> 正式版见 [Releases](../../releases)。macOS / Linux 需在对应平台执行 `npx tauri build` 构建。

## 加密边界（重要）

为避免误解，这里明确说明端到端加密的**覆盖范围**：

| 数据 | 本地落盘 | 同步到服务端 | 服务端可见 |
|------|----------|--------------|------------|
| 口令（password） | 密文（主密钥加密） | **端到端密文**（vault key 加密） | 否 |
| 备注（notes） | 密文（主密钥加密） | **端到端密文**（vault key 加密） | 否 |
| 标题 / 用户名 / 网址 / 分类 | 明文 | 明文 | **是** |
| 同步密钥（vault key） | 系统凭据库（Windows 凭据管理器 / macOS 钥匙串 / Linux 密钥环） | 连主密钥加密后上传 | 否 |

- 标题、用户名、网址、分类作为**索引元数据**保持明文，用于本机搜索与排序，因此服务端（以及任何拿到服务端数据库的人）可以看到「你在哪些网站有哪些账号」，但看不到口令与备注内容。
- 同步地址默认走 HTTPS，并校验服务器证书指纹（首次连接需确认，证书变更会拒绝连接）。
- 主密码与派生密钥永不落盘；同步口令（vault key）交由操作系统凭据库保管。

## 数据存放位置

| 平台 | 路径 |
|------|------|
| Windows（安装版） | `%APPDATA%\CryPtBox\app.db` |
| Windows（便携版） | 可执行文件同目录 `app.db`（exe 旁带 `portable.flag`） |
| macOS | `~/Library/Application Support/CryPtBox/app.db` |
| Linux | `~/.config/CryPtBox/app.db` |


### 数据目录保密要求

该目录包含**加密后的密码库**与系统凭据库的降级文件（若密钥环不可用），请：

- 不要将数据目录同步到网盘、代码仓库或共享文件夹；
- 类 Unix 系统上应用已自动把 `app.db`、`vault_key.bin` 等文件权限收紧为 `0600`；
  Windows 上请确保数据目录仅当前登录用户可访问（安装包默认按当前用户安装）；
- 同步密钥保存在操作系统凭据库（Windows 凭据管理器 / macOS 钥匙串 / Linux 密钥环）中，
  备份数据目录**不会**带走该密钥，这是有意的设计。

### 已知取舍：无系统凭据库时的降级行为

Linux 桌面密钥环不可用（无头服务器、容器、精简桌面环境）时，同步密钥无法交给系统凭据库保管，应用按以下方式降级：

- **同步密钥（vault key）** 写入数据目录下的 `vault_key.bin`（类 Unix 系统上权限 `0600`）。
  这是必要的取舍——不持久化将导致重启后无法解密服务端密码库；
- **会话令牌（JWT）只保留在内存**，不写入任何文件，也不写 WebView 存储。令牌是可直接调用
  服务端 API 的凭据、且重新登录即可恢复，不值得为「免登录」把裸 JWT 明文落盘。

**带来的可见行为**：降级环境下每次重启应用都需重新登录（密码库本身不受影响，仍凭主密码解锁）。
同步面板在检测到降级时会给出提示。

## 安全设计

- **加密参数三端一致**：scrypt（N=2¹⁵, r=8, p=1, len=32）+ AES-256-GCM，密文格式 `base64(nonce12 ‖ ciphertext)`，与服务端 Go 实现、网页端 JS 实现保持完全一致
- **随机数来源**：盐、同步密钥、条目 UUID、AES nonce 一律取自操作系统 CSPRNG（`OsRng`），不使用可预测的伪随机数
- **内容安全策略**：生产构建固定为 `script-src 'self'`，不使用 `unsafe-eval`；i18n 采用构建期预编译（AST 解释器）而非运行时 `new Function`
- **传输校验**：同步默认 HTTPS，并做服务器证书指纹固定（TOFU + 变更即拒），校验时仍走真实的证书签名验证
- **令牌隔离**：JWT 不写入 WebView 存储（内存 / localStorage），由 Rust 侧保管并存入系统凭据库
- **条目身份**：每条记录使用随机 UUIDv4 作为全局身份，多设备合并时按 UUID 归并，避免覆盖丢数据

> 安全问题的反馈与复测记录随仓库文档维护；发现问题欢迎提 Issue。

## 从源码构建

```bash
# 安装前端依赖
npm install

# 前端构建 + 打包（Windows：NSIS + MSI）
npx tauri build

# 仅编译可执行文件（最快）
npx tauri build --no-bundle

# 英文版
npx tauri build --config src-tauri/tauri.en.conf.json
```

构建产物在 `src-tauri/target/release/`（可执行文件）与 `src-tauri/target/release/bundle/`（安装包）。

> 环境要求：Node.js 18+、Rust 稳定工具链；Windows 打包 NSIS/MSI 需 WebView2 与 WiX。

## 云同步与服务端

客户端可连接部署在飞牛 fnOS 上的 **CryPtBox 服务端** 实现多端同步，服务端源码见 [cryptbox-fn](https://github.com/qingzhi-awa/cryptbox-fn)。

- 支持局域网自动扫描发现服务端
- 支持域名 / 公网地址（HTTPS）连接
- 服务端支持 FN Connect / DDNS / 局域网访问

## 目录结构

```
src/                  # Vue 3 前端
├── App.vue           # 主界面
├── api.js            # Tauri 命令封装（前端不持有令牌）
└── i18n/             # 10 种语言词条
src-tauri/            # Rust 后端
├── src/
│   ├── app.rs        # 命令层：解锁 / 条目 / 会话 / 恢复
│   ├── crypto/       # scrypt 派生 + AES-256-GCM
│   ├── store/        # SQLite 存储、便携模式、权限收紧
│   ├── sync/         # 同步协议、证书固定、局域网扫描
│   ├── network/      # HTTP 客户端（超时 / 证书校验）
│   ├── secret/       # 系统凭据库封装（含降级）
│   ├── import_export/# CSV / TXT 导入导出
│   ├── settings/     # 本地设置
│   └── tray/         # 系统托盘
├── tauri.conf.json   # 窗口 / 打包 / CSP 配置
└── Cargo.toml
```

## 技术栈

Rust · Tauri 2 · Vue 3 · vue-i18n · SQLite（rusqlite bundled）· AES-256-GCM · scrypt · rustls

## 更新日志

各版本发布说明见 [Releases](../../releases)。

**行为变更提示（0.2.3 起）**：在无系统凭据库的降级环境中，会话令牌不再持久化——重启应用后需重新登录（同步密钥仍写入 `vault_key.bin`，密码库解密不受影响）。详见上文「已知取舍」。

**行为变更提示（配合服务端 R13-02 修复）**：服务端现在要求「改写既有解锁材料必须二次验证当前口令」——当账号已有 `vault_key_enc` 时，`PUT|POST /api/vault-key` 与 `PUT|POST /api/me`（携带 `kdf_salt`/`vault_key_enc`）都必须附带 `current_password`，否则返回 400。本客户端已同步更新（首次启用不要求口令；「旧密码恢复」路径会带上当前口令）。**若客户端版本早于该修复，连接新版服务端时「旧密码恢复」会被拒绝，请一并升级。**

<!-- 📸 更多截图占位（后续添加）：
| 主界面 | 同步设置 |
|--------|----------|
| ![主界面](docs/screenshot-main.png) | ![同步设置](docs/screenshot-sync.png) |
-->
