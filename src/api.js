// Tauri IPC 适配层：保持旧 Wails 前端调用方式（api.Init() 等 PascalCase 方法名）。
// 命令名：Tauri 命令名即 Rust 函数名（snake_case），不会自动转 camelCase。
import { invoke } from '@tauri-apps/api/core'

export default {
  Init: () => invoke('init_db'),
  SetupMaster: (password) => invoke('setup_master', { password }),
  Unlock: (password) => invoke('unlock', { password }),
  Lock: () => invoke('lock'),
  IsUnlocked: () => invoke('is_unlocked'),
  ListEntries: () => invoke('list_entries'),
  SaveEntry: (entry) => invoke('save_entry', { entry }),
  DeleteEntry: (id, soft) => invoke('delete_entry', { id, soft }),
  // 置顶与顺序调整：置顶仅影响本设备显示顺序；拖拽排序写入 sort_order（随同步传播）。
  PinEntry: (id, pinned) => invoke('pin_entry', { id, pinned }),
  ReorderEntries: (ids) => invoke('reorder_entries', { ids }),
  ListTrash: () => invoke('list_trash'),
  RestoreEntry: (id) => invoke('restore_entry', { id }),
  PurgeEntry: (id) => invoke('purge_entry', { id }),
  EmptyTrash: () => invoke('empty_trash'),

  // 导入 / 导出：文件对话框与读写均在 Rust 侧完成（PT-08），
  // 渲染层不再传递任何路径，因此无法让后端读写任意文件。
  ImportDialog: () => invoke('import_csv'),

  ImportTextDialog: () => invoke('import_txt'),

  // 保存文本文件（导出 / 模板下载共用）：filename 仅用于对话框默认文件名
  SaveTextFile: (content, filename) => invoke('save_text_file', { content, filename }),

  // 下载 CSV 导入模板（示例行与服务端模板保持一致）
  DownloadTemplateDialog: () =>
    invoke('save_text_file', {
      content:
        '\ufeffname,username,password,url,category,notes\n' +
        '示例网站,myuser,MyPass123,https://example.com,常用,这是一条示例备注，可删除\n',
      filename: 'passbook-template.csv'
    }),

  GetServerConfig: () => invoke('get_server_config'),
  // 会话令牌由 Rust 侧保管并经系统凭据库持久化，前端只查询登录态（PT-06）。
  SessionInfo: () => invoke('session_info'),
  ClearSession: () => invoke('clear_session'),
  SyncRegister: (server, username, password, email, code) =>
    invoke('sync_register', { server, username, password, email, code }),
  SyncLogin: (server, username, password) => invoke('sync_login', { server, username, password }),
  // 密码重置后的恢复路径：旧密码恢复（数据无损）或清空重建
  RecoverVault: (server, username, currentPassword, oldPassword) =>
    invoke('recover_vault', { server, username, currentPassword, oldPassword }),
  ResetVaultRemote: (server, username, password) =>
    invoke('reset_vault_remote', { server, username, password }),
  // 以下命令不再接收令牌：统一使用 Rust 侧保存的当前会话。
  SyncCheck: (server) => invoke('sync_check', { server }),
  // 头像端点要求登录，<img> 无法携带 JWT，由 Rust 侧代理拉取并转 data URL。
  FetchAvatar: (server, id) => invoke('fetch_avatar', { server, id }),
  SendRegisterCode: (server, email) => invoke('sync_send_code', { server, email }),
  PushVault: (server) => invoke('push_vault', { server }),
  PullVault: (server) => invoke('pull_vault', { server }),
  MergeVault: (server) => invoke('merge_vault', { server }),
  ScanLAN: () => invoke('scan_lan'),
  // 服务器信任管理（PT-02 / PT-11）：
  // 「首次连接确认」「地址白名单」「指纹变更确认」均由 Rust 侧原生对话框完成，
  // 渲染层没有直接写入信任记录的能力。
  ListAllowedServers: () => invoke('list_allowed_servers'),
  RemoveAllowedServer: (server) => invoke('remove_allowed_server', { server }),
  ForgetServerTrust: (server) => invoke('forget_server_trust', { server }),
  GetTrustedFingerprint: (server) => invoke('get_trusted_fingerprint', { server }),
  // 凭据库后端（"keyring" / "file"），用于提示「已降级为文件存储」。
  GetSecretBackend: () => invoke('get_secret_backend'),
  GetSettings: () => invoke('get_settings'),
  SaveSettings: (autostart, autosync, priority, recycle, recycleDays) =>
    invoke('save_settings', { autostart, autosync, priority, recycle, recycleDays }),
  // 置顶同步开关（账号级，保存在服务端，桌面端/网页端共用）。
  GetPinSync: (server) => invoke('get_pin_sync', { server }),
  SetPinSync: (server, enabled) => invoke('set_pin_sync', { server, enabled }),
  // 本地操作日志（client.log）：读取最近 N 条（新在前）与清空。
  ReadLogs: (limit) => invoke('read_client_logs', { limit }),
  ClearLogs: () => invoke('clear_client_logs'),
  // 用系统默认浏览器打开外部链接（关于页 / GitHub 入口）。
  OpenExternal: (url) => invoke('open_external', { url })
}
