<template>
  <select v-model="lang" class="lang-select lang-fixed" @change="changeLang">
    <option value="zh-CN">简体中文</option>
    <option value="zh-TW">繁體中文</option>
    <option value="en">English</option>
    <option value="ja">日本語</option>
    <option value="ko">한국어</option>
    <option value="fr">Français</option>
    <option value="de">Deutsch</option>
    <option value="es">Español</option>
    <option value="ru">Русский</option>
    <option value="pt">Português</option>
  </select>
  <!-- 未解锁：设置主密码 / 解锁 -->
  <div v-if="!vaultUnlocked" class="gate">
    <div class="gate-card">
      <h1>{{ $t('app.title') }}</h1>
      <p class="sub">{{ vaultInitialized ? $t('gate.unlockHint') : $t('gate.setupHint') }}</p>
      <input
        v-model="vaultPassword"
        type="password"
        :placeholder="vaultInitialized ? $t('gate.passwordPlaceholder') : $t('gate.setupPlaceholder')"
        @keyup.enter="submitMaster"
      />
      <button class="btn-primary" @click="submitMaster">
        {{ vaultInitialized ? $t('gate.unlock') : $t('gate.create') }}
      </button>
      <p v-if="error" class="error">{{ error }}</p>
    </div>
  </div>

  <!-- 已解锁：主界面 -->
  <div v-else class="app">
    <header class="topbar">
      <h1>{{ $t('app.title') }}</h1>
      <div class="actions">
        <button v-if="!serverToken" class="btn-primary" @click="openSync">{{ $t('sync.login') }}</button>
        <div v-else class="user-menu">
          <button class="user-trigger" @click="userMenuOpen = !userMenuOpen">
            <img v-if="myAvatar" :src="avatarUrl" class="avatar" alt="" />
            <span v-else class="avatar avatar-fallback">{{ (serverUsername || '?').charAt(0).toUpperCase() }}</span>
            <span class="server-user">{{ serverUsername }}</span>
            <span class="caret">▾</span>
          </button>
          <div v-if="userMenuOpen" class="user-dropdown">
            <div class="dd-server">{{ server }}</div>
            <button class="dd-item" @click="doLogout">{{ $t('sync.logout') }}</button>
          </div>
        </div>
        <button class="btn-danger" @click="lock">{{ $t('header.lock') }}</button>
      </div>
    </header>
    <div v-if="userMenuOpen" class="menu-overlay" @click="userMenuOpen = false"></div>

    <div class="toolbar">
      <input v-model="search" :placeholder="$t('toolbar.search')" />
      <button class="btn-primary" @click="startAdd">{{ $t('toolbar.add') }}</button>
      <select v-model="importFormat" class="btn-ghost menu-select" @change="onImport">
        <option value="" selected>{{ $t('toolbar.import') }}</option>
        <option value="csv">CSV</option>
        <option value="txt">TXT</option>
      </select>
      <select v-model="exportFormat" class="btn-ghost menu-select" @change="onExport">
        <option value="" selected>{{ $t('header.export') }}</option>
        <option value="csv">CSV</option>
        <option value="txt">TXT</option>
      </select>
      <button class="btn-ghost" @click="downloadTemplate">{{ $t('toolbar.template') }}</button>
      <template v-if="serverToken">
        <button class="btn-primary" @click="doPush">{{ $t('sync.upload') }}</button>
        <button class="btn-ghost" @click="doPull">{{ $t('sync.download') }}</button>
      </template>
    </div>

    <div v-if="msg" class="toast toast-msg">{{ msg }}</div>
    <div v-if="error" class="toast toast-error">{{ error }}</div>

    <div class="list">
      <div v-for="e in filtered" :key="e.id" class="item">
        <div class="item-main">
          <div class="title">{{ e.title }}</div>
          <div class="meta">
            <span class="field" @click="copyText(e.username)">{{ e.username || '—' }}</span>
            <button v-if="e.username" class="copy-btn" @click="copyText(e.username)">{{ $t('list.copy') }}</button>
            <span v-if="e.url" class="field" @click="copyText(e.url)">{{ e.url }}</span>
            <button v-if="e.url" class="copy-btn" @click="copyText(e.url)">{{ $t('list.copy') }}</button>
            <span v-if="e.category" class="tag">{{ e.category }}</span>
          </div>
        </div>
        <div class="pw">
          <span class="mono field" @click="copyText(e.password)">{{ revealed.has(e.id) ? e.password : '••••••••' }}</span>
          <button class="mini" @click="toggleReveal(e.id)">
            {{ revealed.has(e.id) ? $t('list.hide') : $t('list.show') }}
          </button>
          <button class="copy-btn" @click="copyText(e.password)">{{ $t('list.copy') }}</button>
        </div>
        <div class="ops">
          <button class="btn-ghost" @click="startEdit(e)">{{ $t('list.edit') }}</button>
          <button class="btn-danger" @click="remove(e)">{{ $t('list.delete') }}</button>
        </div>
      </div>
      <div v-if="filtered.length === 0" class="empty">{{ $t('list.empty') }}</div>
    </div>

    <!-- 新增 / 编辑弹窗 -->
    <div v-if="editing !== null" class="mask" @click.self="editing = null">
      <div class="modal">
        <h2>{{ form.id ? $t('modal.editTitle') : $t('modal.addTitle') }}</h2>
        <label>{{ $t('modal.title') }}</label>
        <input v-model="form.title" />
        <label>{{ $t('modal.username') }}</label>
        <input v-model="form.username" />
        <label>{{ $t('modal.password') }}</label>
        <div class="pw-row">
          <input v-model="form.password" :type="formShow ? 'text' : 'password'" />
          <button class="mini" @click="formShow = !formShow">{{ formShow ? $t('list.hide') : $t('list.show') }}</button>
        </div>
        <label>{{ $t('modal.url') }}</label>
        <input v-model="form.url" />
        <label>{{ $t('modal.category') }}</label>
        <input v-model="form.category" />
        <label>{{ $t('modal.notes') }}</label>
        <textarea v-model="form.notes" rows="3"></textarea>
        <div class="modal-actions">
          <button class="btn-ghost" @click="editing = null">{{ $t('modal.cancel') }}</button>
          <button class="btn-primary" @click="save">{{ $t('modal.save') }}</button>
        </div>
      </div>
    </div>

    <!-- 登录 / 注册弹窗 -->
    <div v-if="serverLoginOpen" class="mask" @click.self="serverLoginOpen = false">
      <div class="modal">
        <h2>{{ $t('sync.title') }}</h2>
        <label>{{ $t('sync.server') }}</label>
        <div class="server-row">
          <input v-model="server" placeholder="http://127.0.0.1:5201" />
          <button class="btn-ghost scan-btn" @click="scanLan">{{ $t('sync.scan') }}</button>
        </div>
        <div v-if="scanning" class="scan-status">{{ $t('sync.scanning') }}</div>
        <div v-if="lanServers && lanServers.length" class="lan-list">
          <button v-for="s in lanServers" :key="s" class="lan-item" @click="server = s">{{ s }}</button>
        </div>
        <label>{{ $t('sync.username') }}</label>
        <input v-model="serverUsername" />
        <template v-if="serverLoginMode === 'register'">
          <label>{{ $t('sync.email') }}</label>
          <input v-model="serverEmail" />
          <label>{{ $t('sync.password') }}</label>
          <input v-model="serverPassword" type="password" />
          <div class="pw-row">
            <input v-model="serverCode" :placeholder="$t('sync.code')" />
            <button class="btn-ghost" @click="sendRegCode">{{ $t('sync.sendCode') }}</button>
          </div>
        </template>
        <template v-else>
          <label>{{ $t('sync.password') }}</label>
          <input v-model="serverPassword" type="password" @keyup.enter="doLogin" />
          <label class="checkbox-row">
            <input type="checkbox" v-model="rememberMe" />
            <span>{{ $t('sync.remember') }}</span>
          </label>
        </template>
        <div class="modal-actions" style="margin-top: 8px">
          <template v-if="serverLoginMode === 'register'">
            <button class="btn-ghost" @click="serverLoginMode = 'login'">{{ $t('sync.login') }}</button>
            <button class="btn-primary" @click="doRegister">{{ $t('sync.register') }}</button>
          </template>
          <template v-else>
            <button class="btn-ghost" @click="serverLoginMode = 'register'">{{ $t('sync.register') }}</button>
            <button class="btn-primary" @click="doLogin">{{ $t('sync.login') }}</button>
          </template>
        </div>
        <p v-if="error" class="error inline">{{ error }}</p>
        <p v-if="msg" class="msg inline">{{ msg }}</p>
      </div>
    </div>
  </div>
</template>

<script>
import api from './api'

function csvEscape(v) {
  v = String(v == null ? '' : v)
  if (/[",\n\r]/.test(v)) v = '"' + v.replace(/"/g, '""') + '"'
  return v
}

export default {
  data() {
    return {
      vaultInitialized: false,
      vaultUnlocked: false,
      vaultPassword: '',
      entries: [],
      search: '',
      error: '',
      msg: '',
      revealed: new Set(),
      editing: null,
      form: this.emptyForm(),
      formShow: false,
      serverLoginOpen: false,
      userMenuOpen: false,
      serverLoginMode: 'login',
      server: '',
      serverUsername: '',
      serverPassword: '',
      serverEmail: '',
      serverCode: '',
      rememberMe: false,
      lanServers: [],
      scanning: false,
      serverToken: localStorage.getItem('serverToken') || '',
      myAvatar: localStorage.getItem('myAvatar') || '',
      importFormat: '',
      exportFormat: '',
      toastTimer: null,
      lang: localStorage.getItem('locale') || 'zh-CN'
    }
  },
  computed: {
    filtered() {
      const q = this.search.trim().toLowerCase()
      if (!q) return this.entries
      return this.entries.filter((e) =>
        [e.title, e.username, e.url, e.category].some((s) => (s || '').toLowerCase().includes(q))
      )
    },
    avatarUrl() {
      if (!this.myAvatar) return ''
      const id = this.myAvatar.replace(/\.[^.]+$/, '')
      return this.server.replace(/\/$/, '') + '/api/avatar/' + id
    }
  },
  watch: {
    msg(v) { if (v) this.autoClearToast('msg') },
    error(v) { if (v) this.autoClearToast('error') }
  },
  async mounted() {
    try {
      this.vaultInitialized = await api.Init()
      const cfg = await api.GetServerConfig()
      this.server = localStorage.getItem('syncServer') || cfg.server || ''
      this.serverUsername = localStorage.getItem('syncUsername') || cfg.username || ''
      const savedPassword = localStorage.getItem('syncPassword') || ''
      if (savedPassword) {
        this.serverPassword = savedPassword
        this.rememberMe = true
      }
      if (this.serverToken && this.server) {
        try {
          await api.SyncCheck(this.server, this.serverToken)
        } catch (e) {
          this.serverToken = ''
          localStorage.removeItem('serverToken')
          localStorage.removeItem('myAvatar')
        }
      }
    } catch (e) {
      this.error = String(e)
    }
  },
  methods: {
    emptyForm() {
      return { id: 0, title: '', username: '', password: '', url: '', category: '', notes: '' }
    },
    autoClearToast(key) {
      clearTimeout(this.toastTimer)
      this.toastTimer = setTimeout(() => {
        this[key] = ''
      }, 3000)
    },
    changeLang() {
      this.$i18n.locale = this.lang
      localStorage.setItem('locale', this.lang)
    },
    fieldName(key) {
      return this.$t(key).replace(' *', '')
    },
    async submitMaster() {
      this.error = ''
      try {
        const ok = this.vaultInitialized
          ? await api.Unlock(this.vaultPassword)
          : await api.SetupMaster(this.vaultPassword)
        if (ok) {
          this.vaultUnlocked = true
          this.vaultPassword = ''
          await this.loadEntries()
        } else {
          this.error = this.$t('gate.wrongPassword')
        }
      } catch (e) {
        this.error = String(e)
      }
    },
    async loadEntries() {
      this.entries = (await api.ListEntries()) || []
    },
    async lock() {
      await api.Lock()
      this.vaultUnlocked = false
      this.entries = []
      this.revealed.clear()
    },
    startAdd() {
      this.form = this.emptyForm()
      this.formShow = false
      this.editing = {}
    },
    startEdit(e) {
      this.form = { ...e }
      this.formShow = false
      this.editing = { id: e.id }
    },
    async save() {
      if (!this.form.title) {
        this.error = this.$t('msg.titleRequired')
        return
      }
      try {
        await api.SaveEntry(this.form)
        this.editing = null
        this.msg = this.$t('msg.saved')
        await this.loadEntries()
        await this.autoSync()
      } catch (e) {
        this.error = String(e)
      }
    },
    async remove(e) {
      if (!confirm(this.$t('msg.confirmDelete', { title: e.title }))) return
      try {
        await api.DeleteEntry(e.id)
        this.msg = this.$t('msg.deleted')
        await this.loadEntries()
        await this.autoSync()
      } catch (err) {
        this.error = String(err)
      }
    },
    async autoSync() {
      if (!this.serverToken) return
      try {
        await api.PushVault(this.server, this.serverToken)
      } catch (e) {
        /* 自动同步失败不阻断本地操作 */
      }
    },
    toggleReveal(id) {
      if (this.revealed.has(id)) this.revealed.delete(id)
      else this.revealed.add(id)
    },
    async copyText(text) {
      if (!text) return
      try {
        const value = String(text)
        if (navigator.clipboard && window.isSecureContext) {
          await navigator.clipboard.writeText(value)
        } else {
          const ta = document.createElement('textarea')
          ta.value = value
          ta.style.position = 'fixed'
          ta.style.opacity = '0'
          document.body.appendChild(ta)
          ta.select()
          document.execCommand('copy')
          document.body.removeChild(ta)
        }
        this.msg = this.$t('msg.copied')
      } catch (e) {
        this.error = String(e)
      }
    },
    async exportTxt() {
      try {
        const lines = [this.$t('app.title'), '='.repeat(60), '']
        for (const e of this.entries) {
          lines.push(this.fieldName('modal.title') + ': ' + e.title)
          lines.push(this.fieldName('modal.username') + ': ' + e.username)
          lines.push(this.fieldName('modal.password') + ': ' + e.password)
          lines.push(this.fieldName('modal.url') + ': ' + e.url)
          lines.push(this.fieldName('modal.category') + ': ' + e.category)
          lines.push(this.fieldName('modal.notes') + ': ' + e.notes)
          lines.push('-'.repeat(60))
        }
        const path = await api.SaveTextFile(lines.join('\n'), this.$t('app.title') + '.txt')
        if (path) this.msg = this.$t('msg.exported', { n: this.entries.length, path })
      } catch (e) {
        this.error = String(e)
      }
    },
    async importCsv() {
      try {
        const n = await api.ImportDialog()
        if (n > 0) this.msg = this.$t('msg.imported', { n })
        await this.loadEntries()
      } catch (e) {
        this.error = String(e)
      }
    },
    async importTxt() {
      try {
        const n = await api.ImportTextDialog()
        if (n > 0) this.msg = this.$t('msg.imported', { n })
        await this.loadEntries()
      } catch (e) {
        this.error = String(e)
      }
    },
    onImport() {
      const f = this.importFormat
      if (f === 'csv') this.importCsv()
      else if (f === 'txt') this.importTxt()
      this.importFormat = ''
    },
    onExport() {
      const f = this.exportFormat
      if (f === 'txt') this.exportTxt()
      else if (f === 'csv') this.exportCsv()
      this.exportFormat = ''
    },
    async exportCsv() {
      try {
        const keys = ['title', 'username', 'password', 'url', 'category', 'notes']
        const header = keys.map((k) => this.fieldName('modal.' + k))
        const rows = [header.map(csvEscape).join(',')]
        for (const e of this.entries) {
          rows.push([e.title, e.username, e.password, e.url, e.category, e.notes].map(csvEscape).join(','))
        }
        const csv = '\ufeff' + rows.join('\r\n')
        const path = await api.SaveTextFile(csv, this.$t('app.title') + '.csv')
        if (path) this.msg = this.$t('msg.exportedCsv', { n: this.entries.length })
      } catch (e) {
        this.error = String(e)
      }
    },
    async downloadTemplate() {
      try {
        const keys = ['title', 'username', 'password', 'url', 'category', 'notes']
        const header = keys.map((k) => this.fieldName('modal.' + k)).join(',')
        const path = await api.SaveTextFile('\ufeff' + header + '\r\n', this.$t('app.title') + ' - ' + this.$t('file.template') + '.csv')
        if (path) this.msg = this.$t('msg.saved')
      } catch (e) {
        this.error = String(e)
      }
    },
    openSync() {
      this.error = ''
      this.msg = ''
      this.serverLoginMode = 'login'
      this.lanServers = []
      this.scanning = false
      const savedPassword = localStorage.getItem('syncPassword') || ''
      this.serverPassword = savedPassword
      this.rememberMe = !!savedPassword
      this.serverLoginOpen = true
    },
    async scanLan() {
      this.error = ''
      this.scanning = true
      this.lanServers = []
      try {
        this.lanServers = (await api.ScanLAN()) || []
        if (this.lanServers.length === 0) {
          this.msg = this.$t('sync.noServer')
        }
      } catch (e) {
        this.error = String(e)
      } finally {
        this.scanning = false
      }
    },
    async doRegister() {
      await this.auth(true)
    },
    async doLogin() {
      await this.auth(false)
    },
    doLogout() {
      this.serverToken = ''
      this.serverPassword = ''
      this.myAvatar = ''
      this.userMenuOpen = false
      localStorage.removeItem('serverToken')
      localStorage.removeItem('myAvatar')
      this.msg = this.$t('sync.loggedOut')
    },
    async auth(isRegister) {
      this.error = ''
      try {
        const r = isRegister
          ? await api.SyncRegister(this.server, this.serverUsername, this.serverPassword, this.serverEmail, this.serverCode)
          : await api.SyncLogin(this.server, this.serverUsername, this.serverPassword)
        this.serverToken = r.token
        this.myAvatar = r.avatar || ''
        localStorage.setItem('serverToken', r.token)
        localStorage.setItem('myAvatar', r.avatar || '')
        localStorage.setItem('syncServer', this.server)
        localStorage.setItem('syncUsername', this.serverUsername)
        if (this.rememberMe) {
          localStorage.setItem('syncPassword', this.serverPassword)
        } else {
          localStorage.removeItem('syncPassword')
        }
        this.msg = isRegister ? this.$t('msg.registerSuccess') : this.$t('msg.loginSuccess')
        this.serverLoginOpen = false
        this.serverPassword = ''
        this.serverEmail = ''
        this.serverCode = ''
      } catch (e) {
        this.error = String(e)
      }
    },
    async sendRegCode() {
      this.error = ''
      try {
        await api.SendRegisterCode(this.server, this.serverEmail)
        this.msg = this.$t('sync.codeSent')
      } catch (e) {
        this.error = String(e)
      }
    },
    async doPush() {
      this.error = ''
      try {
        await api.PushVault(this.server, this.serverToken)
        this.msg = this.$t('msg.uploaded')
      } catch (e) {
        this.error = String(e)
      }
    },
    async doPull() {
      this.error = ''
      try {
        const n = await api.PullVault(this.server, this.serverToken)
        this.msg = this.$t('msg.downloaded', { n })
        await this.loadEntries()
      } catch (e) {
        this.error = String(e)
      }
    }
  }
}
</script>

<style scoped>
.gate {
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
}
.gate-card {
  width: 340px;
  background: #ffffff;
  border: 1px solid #e5e7eb;
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.06);
  padding: 32px;
  border-radius: 12px;
  text-align: center;
  display: flex;
  flex-direction: column;
  gap: 14px;
}
.gate-card h1 {
  font-size: 24px;
}
.sub {
  color: #6b7280;
}
.error {
  color: #dc2626;
}
.msg {
  color: #16a34a;
}
.toast {
  position: fixed;
  left: 50%;
  top: 50%;
  transform: translate(-50%, -50%);
  z-index: 10000;
  padding: 14px 24px;
  border-radius: 10px;
  font-size: 14px;
  max-width: 70%;
  text-align: center;
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.35);
  pointer-events: none;
}
.toast-msg {
  background: rgba(16, 185, 129, 0.95);
  color: #fff;
}
.toast-error {
  background: rgba(220, 38, 38, 0.95);
  color: #fff;
}
.inline {
  margin: 8px 0;
}
.lang-select {
  background: #eef2f7;
  color: #1f2937;
  border: 1px solid #d1d5db;
  border-radius: 6px;
  padding: 6px 8px;
  font-size: 13px;
  outline: none;
  width: auto;
  min-width: 96px;
}
.menu-select {
  width: auto;
  min-width: 96px;
  cursor: pointer;
}
.menu-label {
  align-self: center;
  color: #6b7280;
  font-size: 13px;
}
.lang-fixed {
  position: fixed;
  top: 14px;
  right: 14px;
  z-index: 9999;
}
.app {
  height: 100%;
  display: flex;
  flex-direction: column;
  padding: 16px;
}
.topbar {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 14px;
  padding-right: 120px;
}
.actions {
  display: flex;
  gap: 8px;
  align-items: center;
}
.server-user {
  color: #6b7280;
  font-size: 13px;
  max-width: 160px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.avatar {
  width: 28px;
  height: 28px;
  border-radius: 50%;
  object-fit: cover;
  flex-shrink: 0;
}
.avatar-fallback {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  background: var(--fnos-primary);
  color: #fff;
  font-weight: 600;
  font-size: 13px;
}
.toolbar {
  display: flex;
  gap: 8px;
  margin-bottom: 14px;
}
.toolbar input {
  flex: 1;
}
.list {
  flex: 1;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.item {
  display: flex;
  align-items: center;
  gap: 16px;
  background: #ffffff;
  border: 1px solid #e5e7eb;
  padding: 12px 16px;
  border-radius: 8px;
}
.item-main {
  flex: 1;
  min-width: 0;
}
.title {
  font-weight: 600;
}
.meta {
  display: flex;
  gap: 8px;
  color: #6b7280;
  font-size: 12px;
  margin-top: 4px;
  align-items: center;
  flex-wrap: wrap;
}
.tag {
  background: #eef2f7;
  padding: 1px 8px;
  border-radius: 10px;
}
.field {
  cursor: pointer;
}
.field:hover {
  text-decoration: underline;
}
.copy-btn {
  background: transparent;
  color: var(--fnos-primary);
  border: 1px solid var(--fnos-primary);
  padding: 2px 8px;
  font-size: 12px;
  border-radius: 4px;
  flex-shrink: 0;
}
.copy-btn:hover {
  background: var(--fnos-primary-light);
}
.pw {
  display: flex;
  align-items: center;
  gap: 8px;
}
.mono {
  font-family: 'Consolas', monospace;
}
.ops {
  display: flex;
  gap: 6px;
}
.mini {
  background: #eef2f7;
  color: #1f2937;
  padding: 4px 8px;
  font-size: 12px;
}
.empty {
  text-align: center;
  color: #6b7280;
  margin-top: 40px;
}
.mask {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.5);
  display: flex;
  align-items: center;
  justify-content: center;
}
.modal {
  width: 420px;
  max-height: 90%;
  overflow-y: auto;
  background: #ffffff;
  border: 1px solid #d1d5db;
  border-radius: 12px;
  padding: 24px;
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.modal h2 {
  margin-bottom: 8px;
}
.modal label {
  color: #6b7280;
  font-size: 12px;
  margin-top: 6px;
}
.pw-row {
  display: flex;
  gap: 8px;
}
.pw-row input {
  flex: 1;
}
.modal-actions {
  display: flex;
  gap: 8px;
  justify-content: flex-end;
  margin-top: 12px;
}
.user-menu {
  position: relative;
}
.user-trigger {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  background: transparent;
  border: 1px solid #d1d5db;
  padding: 4px 10px;
  border-radius: 6px;
}
.user-trigger:hover {
  opacity: 1;
  background: #eef2f7;
}
.caret {
  color: #6b7280;
  font-size: 12px;
}
.user-dropdown {
  position: absolute;
  top: calc(100% + 6px);
  right: 0;
  min-width: 200px;
  background: #ffffff;
  border: 1px solid #e5e7eb;
  border-radius: 8px;
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.12);
  z-index: 5001;
  overflow: hidden;
}
.dd-server {
  padding: 8px 12px;
  font-size: 12px;
  color: #6b7280;
  border-bottom: 1px solid #eef2f7;
  word-break: break-all;
}
.dd-item {
  display: block;
  width: 100%;
  text-align: left;
  background: transparent;
  color: #dc2626;
  padding: 10px 12px;
  border-radius: 0;
}
.dd-item:hover {
  background: #fef2f2;
  opacity: 1;
}
.menu-overlay {
  position: fixed;
  inset: 0;
  z-index: 5000;
}
.checkbox-row {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-top: 4px;
  cursor: pointer;
  color: #374151;
  font-size: 13px;
}
.checkbox-row input {
  width: auto;
  cursor: pointer;
}
.server-row {
  display: flex;
  gap: 8px;
}
.server-row input {
  flex: 1;
}
.scan-btn {
  flex-shrink: 0;
  white-space: nowrap;
}
.scan-status {
  color: #6b7280;
  font-size: 12px;
}
.lan-list {
  display: flex;
  flex-direction: column;
  gap: 4px;
  max-height: 120px;
  overflow-y: auto;
}
.lan-item {
  text-align: left;
  background: #eef2f7;
  color: #1f2937;
  padding: 6px 10px;
  border-radius: 6px;
  font-size: 13px;
}
.lan-item:hover {
  background: var(--fnos-primary-light);
  opacity: 1;
}
</style>
