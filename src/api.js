// Tauri IPC 适配层：保持旧 Wails 前端调用方式（api.Init() 等 PascalCase 方法名）。
// 命令名：Tauri 命令名即 Rust 函数名（snake_case），不会自动转 camelCase。
import { invoke } from '@tauri-apps/api/core'
import { open, save } from '@tauri-apps/plugin-dialog'

export default {
  Init: () => invoke('init_db'),
  SetupMaster: (password) => invoke('setup_master', { password }),
  Unlock: (password) => invoke('unlock', { password }),
  Lock: () => invoke('lock'),
  IsUnlocked: () => invoke('is_unlocked'),
  ListEntries: () => invoke('list_entries'),
  SaveEntry: (entry) => invoke('save_entry', { entry }),
  DeleteEntry: (id, soft) => invoke('delete_entry', { id, soft }),
  ListTrash: () => invoke('list_trash'),
  RestoreEntry: (id) => invoke('restore_entry', { id }),
  PurgeEntry: (id) => invoke('purge_entry', { id }),
  EmptyTrash: () => invoke('empty_trash'),

  // 导入 CSV：弹出文件选择框（支持 Chrome / Edge 导出的 CSV）
  ImportDialog: async () => {
    const path = await open({
      title: '导入密码（支持 Chrome / Edge 导出的 CSV）',
      multiple: false,
      filters: [{ name: 'CSV', extensions: ['csv'] }]
    })
    if (!path) return 0
    return invoke('import_csv', { path })
  },

  // 导入 TXT：弹出文件选择框
  ImportTextDialog: async () => {
    const path = await open({
      title: '导入密码 TXT',
      multiple: false,
      filters: [{ name: 'TXT', extensions: ['txt'] }]
    })
    if (!path) return 0
    return invoke('import_txt', { path })
  },

  // 保存文本文件：弹出保存框 + 写入内容
  SaveTextFile: async (content, filename) => {
    const path = await save({ title: '保存文件', defaultPath: filename })
    if (!path) return ''
    await invoke('save_text_file', { path, content })
    return path
  },

  // 下载 CSV 导入模板
  DownloadTemplateDialog: async () => {
    const path = await save({ title: '下载 CSV 模板', defaultPath: 'passbook-template.csv' })
    if (!path) return ''
    await invoke('save_text_file', { path, content: '\ufeffname,username,password,url,category,notes\n' })
    return path
  },

  GetServerConfig: () => invoke('get_server_config'),
  SyncRegister: (server, username, password, email, code) =>
    invoke('sync_register', { server, username, password, email, code }),
  SyncLogin: (server, username, password) => invoke('sync_login', { server, username, password }),
  SyncCheck: (server, token) => invoke('sync_check', { server, token }),
  SendRegisterCode: (server, email) => invoke('sync_send_code', { server, email }),
  PushVault: (server, token) => invoke('push_vault', { server, token }),
  PullVault: (server, token) => invoke('pull_vault', { server, token }),
  MergeVault: (server, token) => invoke('merge_vault', { server, token }),
  ScanLAN: () => invoke('scan_lan'),
  GetSettings: () => invoke('get_settings'),
  SaveSettings: (autostart, autosync, priority, recycle, recycleDays) =>
    invoke('save_settings', { autostart, autosync, priority, recycle, recycleDays })
}
