// 延迟访问 Wails 绑定，避免在 window.go 注入前访问导致报错
const app = () => window.go.main.App

export default {
  Init: () => app().Init(),
  SetupMaster: (password) => app().SetupMaster(password),
  Unlock: (password) => app().Unlock(password),
  Lock: () => app().Lock(),
  ListEntries: () => app().ListEntries(),
  SaveEntry: (entry) => app().SaveEntry(entry),
  DeleteEntry: (id) => app().DeleteEntry(id),
  ExportDialog: () => app().ExportDialog(),
  ExportCSVDialog: () => app().ExportCSVDialog(),
  ImportDialog: () => app().ImportDialog(),
  ImportTextDialog: () => app().ImportTextDialog(),
  DownloadTemplateDialog: () => app().DownloadTemplateDialog(),
  SaveTextFile: (content, filename) => app().SaveTextFile(content, filename),
  GetServerConfig: () => app().GetServerConfig(),
  SyncRegister: (server, username, password, email, code) => app().SyncRegister(server, username, password, email, code),
  SendRegisterCode: (server, email) => app().SendRegisterCode(server, email),
  SyncCheck: (server, token) => app().SyncCheck(server, token),
  ScanLAN: () => app().ScanLAN(),
  SyncLogin: (server, username, password) => app().SyncLogin(server, username, password),
  PushVault: (server, token) => app().PushVault(server, token),
  PullVault: (server, token) => app().PullVault(server, token),
  MergeVault: (server, token) => app().MergeVault(server, token),
  GetSettings: () => app().GetSettings(),
  SaveSettings: (autostart, autosync, priority) => app().SaveSettings(autostart, autosync, priority)
}
