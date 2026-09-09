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
  <!-- 登录 / 注册 / 找回密码 -->
  <div v-if="!token" class="gate">
    <div class="gate-card">
      <h1>{{ gateTitle }}</h1>

      <template v-if="gateMode === 'setup'">
        <p class="sub">{{ $t('login.setupHint') }}</p>
        <input v-model="setupForm.username" :placeholder="$t('login.username')" />
        <input v-model="setupForm.password" type="password" :placeholder="$t('login.password')" @keyup.enter="doSetup" />
        <button class="btn-primary" @click="doSetup">{{ $t('login.setupSubmit') }}</button>
      </template>

      <template v-else-if="gateMode === 'login'">
        <input v-model="loginForm.username" :placeholder="$t('login.usernameOrEmail')" @keyup.enter="login" />
        <input v-model="loginForm.password" type="password" :placeholder="$t('login.password')" @keyup.enter="login" />
        <button class="btn-primary" @click="login">{{ $t('login.submit') }}</button>
        <div v-if="!isAdminLogin" class="gate-links">
          <button v-if="allowReg" class="link" @click="switchGate('register')">{{ $t('login.toRegister') }}</button>
          <button class="link" @click="switchGate('forgot')">{{ $t('login.forgot') }}</button>
        </div>
      </template>

      <template v-else-if="gateMode === 'register'">
        <p v-if="!allowReg" class="sub">{{ $t('login.registerClosed') }}</p>
        <template v-else>
          <input v-model="regForm.username" :placeholder="$t('login.username')" />
          <input v-model="regForm.email" :placeholder="$t('users.email')" />
          <input v-model="regForm.password" type="password" :placeholder="$t('login.password')" />
          <div class="code-row">
            <input v-model="regForm.code" :placeholder="$t('login.code')" />
            <button class="btn-ghost" @click="sendRegCode">{{ $t('login.sendCode') }}</button>
          </div>
          <button class="btn-primary" @click="register">{{ $t('login.register') }}</button>
        </template>
        <div class="gate-links">
          <button class="link" @click="switchGate('login')">{{ $t('login.back') }}</button>
        </div>
      </template>

      <template v-else>
        <input v-model="resetForm.email" :placeholder="$t('users.email')" />
        <div v-if="verifyMode !== 'none'" class="code-row">
          <input v-model="resetForm.code" :placeholder="$t('login.code')" />
          <button class="btn-ghost" @click="sendResetCode">{{ $t('login.sendCode') }}</button>
        </div>
        <input v-model="resetForm.password" type="password" :placeholder="$t('login.newPassword')" />
        <button class="btn-primary" @click="resetPassword">{{ $t('login.resetSubmit') }}</button>
        <div class="gate-links">
          <button class="link" @click="switchGate('login')">{{ $t('login.back') }}</button>
        </div>
      </template>

      <p v-if="error" class="error">{{ error }}</p>
      <p v-else-if="msg" class="msg">{{ msg }}</p>
    </div>
  </div>

  <!-- 主界面 -->
  <div v-else class="app">
    <header class="topbar">
      <h1>{{ $t('app.title') }}</h1>
      <div class="user-info">
        <div class="user-menu" @click="menuOpen = !menuOpen">
          <img v-if="myAvatar" :src="avatarUrl(myId)" class="avatar" alt="" />
          <span v-else class="avatar avatar-fallback">{{ (username || '?').charAt(0).toUpperCase() }}</span>
          <span class="user-name">{{ username }}</span>
          <span class="caret">▾</span>
          <div v-if="menuOpen" class="user-dropdown" @click.stop>
            <button class="dropdown-item" @click="openMe(); menuOpen = false">{{ $t('me.edit') }}</button>
            <button class="dropdown-item" @click="logout">{{ $t('header.logout') }}</button>
          </div>
        </div>
      </div>
    </header>
    <div v-if="menuOpen" class="menu-backdrop" @click="menuOpen = false"></div>

    <div class="layout">
      <nav class="sidebar">
        <button :class="{ active: tab === 'passwords' }" @click="switchTab('passwords')">{{ $t('tabs.passwords') }}</button>
        <button v-if="isAdmin" :class="{ active: tab === 'users' }" @click="switchTab('users')">
          {{ $t('tabs.users') }}
        </button>
        <button v-if="isAdmin" :class="{ active: tab === 'logs' }" @click="switchTab('logs')">{{ $t('tabs.logs') }}</button>
        <button v-if="isAdmin" :class="{ active: tab === 'settings' }" @click="switchTab('settings')">{{ $t('tabs.settings') }}</button>
      </nav>

      <div class="content">
        <p v-if="error" class="error inline">{{ error }}</p>
        <p v-if="msg" class="msg inline">{{ msg }}</p>

    <!-- 密码管理 -->
    <div v-if="tab === 'passwords'" class="panel">
      <div class="toolbar">
        <div class="toolbar-main">
          <input v-model="search" :placeholder="$t('toolbar.search')" />
          <button class="btn-primary" @click="startAdd">{{ $t('toolbar.add') }}</button>
        </div>
        <div class="toolbar-sub">
          <select v-model="ioFormat" class="btn-ghost menu-select" @change="onIO">
            <option value="" selected>{{ $t('toolbar.io') }}</option>
            <option value="import-csv">{{ $t('toolbar.import') }} CSV</option>
            <option value="import-txt">{{ $t('toolbar.import') }} TXT</option>
            <option value="export-csv">{{ $t('toolbar.export') }} CSV</option>
            <option value="export-txt">{{ $t('toolbar.export') }} TXT</option>
          </select>
          <button class="btn-ghost" @click="downloadTemplate">{{ $t('toolbar.template') }}</button>
        </div>
        <input ref="fileInput" type="file" accept=".csv,.txt,text/csv,text/plain" style="display:none" @change="importFile" />
      </div>

      <div class="list">
        <div v-for="e in filtered" :key="e.id" class="entry">
          <div class="entry-head">
            <div class="title">{{ e.title }}</div>
            <div class="ops">
              <button class="btn-ghost" @click="startEdit(e)">{{ $t('list.edit') }}</button>
              <button class="btn-danger" @click="remove(e)">{{ $t('list.delete') }}</button>
            </div>
          </div>
          <div class="entry-meta">
            <div class="meta-line">
              <span class="label">{{ $t('modal.url') }}:</span>
              <span class="field" @click="copyText(e.url)">{{ e.url || '—' }}</span>
              <button v-if="e.url" class="copy-btn" @click="copyText(e.url)">{{ $t('list.copy') }}</button>
            </div>
            <div class="meta-line">
              <span class="label">{{ $t('modal.username') }}:</span>
              <span class="field" @click="copyText(e.username)">{{ e.username || '—' }}</span>
              <button v-if="e.username" class="copy-btn" @click="copyText(e.username)">{{ $t('list.copy') }}</button>
              <span class="label">{{ $t('modal.password') }}:</span>
              <span class="mono field" @click="copyText(e.password)">{{ revealed.has(e.id) ? e.password : '••••••••' }}</span>
              <button class="mini" @click="toggleReveal(e.id)">
                {{ revealed.has(e.id) ? $t('list.hide') : $t('list.show') }}
              </button>
              <button class="copy-btn" @click="copyText(e.password)">{{ $t('list.copy') }}</button>
            </div>
            <div v-if="e.category" class="meta-line">
              <span class="label">{{ $t('modal.category') }}:</span>
              <span class="field" @click="copyText(e.category)">{{ e.category }}</span>
            </div>
            <div v-if="e.notes" class="meta-line">
              <span class="label">{{ $t('modal.notes') }}:</span>
              <span class="field" @click="copyText(e.notes)">{{ e.notes }}</span>
            </div>
          </div>
        </div>
        <div v-if="filtered.length === 0" class="empty">{{ $t('list.empty') }}</div>
      </div>
    </div>

    <!-- 用户管理 -->
    <div v-if="tab === 'users'" class="panel">
      <div class="toolbar">
        <div class="add-user">
          <input v-model="newUser.username" :placeholder="$t('users.username')" />
          <input v-model="newUser.email" :placeholder="$t('users.email')" />
          <input v-model="newUser.password" type="password" :placeholder="$t('users.password')" />
          <select v-model="newUser.role">
            <option value="user">{{ $t('users.roleUser') }}</option>
            <option value="admin">{{ $t('users.roleAdmin') }}</option>
          </select>
          <button class="btn-primary" @click="createUser">{{ $t('users.add') }}</button>
        </div>
        <div class="toolbar-sub">
          <button class="btn-ghost" @click="$refs.userFileInput.click()">{{ $t('users.import') }}</button>
          <button class="btn-ghost" @click="downloadUserTemplate">{{ $t('users.template') }}</button>
          <input ref="userFileInput" type="file" accept=".csv,text/csv" style="display:none" @change="importUsersFile" />
        </div>
      </div>
      <p v-if="userError" class="error inline">{{ userError }}</p>

      <div class="list">
        <div v-for="u in users" :key="u.id" class="row">
          <img v-if="u.avatar" :src="avatarUrl(u.id)" class="avatar" alt="" />
          <span v-else class="avatar avatar-fallback">{{ (u.username || '?').charAt(0).toUpperCase() }}</span>
          <div class="main">
            <div class="title">#{{ u.id }} {{ u.username }}</div>
            <div class="meta">
              <span class="tag">{{ roleLabel(u.role) }}</span>
              <span :class="u.status === 'active' ? 'ok' : 'off'">
                {{ u.status === 'active' ? $t('users.active') : $t('users.disabled') }}
              </span>
              <span v-if="u.email">{{ u.email }}</span>
            </div>
          </div>
          <div class="ops" v-if="canManageUser(u)">
            <button class="btn-ghost" @click="startEditUser(u)">{{ $t('users.edit') }}</button>
            <button class="btn-ghost" @click="toggleStatus(u)">
              {{ u.status === 'active' ? $t('users.disable') : $t('users.enable') }}
            </button>
            <button class="btn-danger" @click="removeUser(u)">{{ $t('list.delete') }}</button>
          </div>
        </div>
        <div v-if="users.length === 0" class="empty">{{ $t('users.empty') }}</div>
      </div>
    </div>

    <!-- 日志 -->
    <div v-if="tab === 'logs'" class="panel">
      <div class="list">
        <div v-for="l in logs" :key="l.id" class="row">
          <div class="main">
            <div class="title"><span class="tag">{{ l.username || '—' }}</span> {{ l.action }}</div>
            <div class="meta">
              <span>{{ l.detail }}</span>
              <span>{{ l.ip }}</span>
              <span>{{ l.created_at }}</span>
            </div>
          </div>
        </div>
        <div v-if="logs.length === 0" class="empty">{{ $t('logs.empty') }}</div>
      </div>
    </div>

    <!-- 系统设置 -->
    <div v-if="tab === 'settings'" class="panel">
      <div class="settings-card">
        <div class="card-header">
          <h3>{{ $t('settings.smtp') }}</h3>
        </div>
        <div class="card-body">
          <label class="form-label">{{ $t('settings.vendor') }}</label>
          <select v-model="settings.smtp.vendor" class="select-block" @change="onVendorChange">
            <option v-for="v in smtpVendors" :key="v.value" :value="v.value">{{ v.label }}</option>
          </select>

          <template v-if="settings.smtp.vendor !== 'custom'">
            <label class="form-label">{{ $t('settings.email') }}</label>
            <input v-model="settings.smtp.from" :placeholder="$t('settings.emailPlaceholder')" />
            <label class="form-label">
              {{ $t('settings.authCode') }}
              <a class="link-inline" href="#" @click.prevent="showVendorAuthHelp">{{ $t('settings.howToAuthCode') }}</a>
            </label>
            <input v-model="settings.smtp.password" type="password" :placeholder="$t('settings.authCodePlaceholder')" />
            <div v-if="vendorTip" class="tip">{{ vendorTip }}</div>
          </template>

          <template v-else>
            <label class="form-label">{{ $t('settings.host') }}</label>
            <input v-model="settings.smtp.host" :placeholder="$t('settings.hostPlaceholder')" />

            <div class="form-grid">
              <div class="form-col">
                <label class="form-label">{{ $t('settings.secureMode') }}</label>
                <select v-model="settingsSecureMode" class="select-block" @change="onSecureChange">
                  <option value="ssl">{{ $t('settings.sslSecure') }}</option>
                  <option value="tls">{{ $t('settings.tlsSecure') }}</option>
                  <option value="none">{{ $t('settings.noSecure') }}</option>
                </select>
              </div>
              <div class="form-col">
                <label class="form-label">{{ $t('settings.port') }}</label>
                <input v-model.number="settings.smtp.port" type="number" @input="onPortEdit" />
              </div>
            </div>

            <div class="form-grid">
              <div class="form-col">
                <label class="form-label">{{ $t('settings.username') }}</label>
                <input v-model="settings.smtp.username" :placeholder="$t('settings.usernamePlaceholder')" />
              </div>
              <div class="form-col">
                <label class="form-label">{{ $t('settings.password') }}</label>
                <input v-model="settings.smtp.password" type="password" :placeholder="$t('settings.passwordHint')" />
              </div>
            </div>

            <label class="form-label">{{ $t('settings.from') }}</label>
            <input v-model="settings.smtp.from" :placeholder="$t('settings.fromPlaceholder')" />
          </template>
        </div>

        <div class="card-divider" />

        <div class="card-header">
          <h3>{{ $t('settings.verifyMode') }}</h3>
        </div>
        <div class="card-body">
          <select v-model="settings.email_verify_mode" class="select-block">
            <option value="code">{{ $t('settings.modeCode') }}</option>
            <option value="none">{{ $t('settings.modeNone') }}</option>
          </select>
        </div>

        <div class="card-divider" />

        <div class="card-header">
          <h3>{{ $t('settings.registerTitle') }}</h3>
        </div>
        <div class="card-body">
          <div class="form-grid">
            <div class="form-col">
              <label class="form-label">{{ $t('settings.allowRegistration') }}</label>
              <select v-model="settings.allow_registration" class="select-block">
                <option :value="false">{{ $t('settings.off') }}</option>
                <option :value="true">{{ $t('settings.on') }}</option>
              </select>
            </div>
            <div class="form-col">
              <label class="form-label">{{ $t('settings.passwordMinLength') }}</label>
              <input v-model.number="settings.password_min_length" type="number" min="4" max="64" />
            </div>
          </div>
          <div class="form-grid">
            <div class="form-col">
              <label class="form-label">{{ $t('settings.requireComplex') }}</label>
              <select v-model="settings.password_require_complex" class="select-block">
                <option :value="false">{{ $t('settings.off') }}</option>
                <option :value="true">{{ $t('settings.on') }}</option>
              </select>
            </div>
          </div>
        </div>

        <div class="card-divider" />

        <div class="card-header">
          <h3>{{ $t('settings.siteTitle') }}</h3>
        </div>
        <div class="card-body">
          <label class="form-label">{{ $t('settings.footerText') }}</label>
          <input v-model="site.footer_text" />
        </div>

        <div class="card-footer">
          <button class="btn-primary" @click="saveSettings">{{ $t('modal.save') }}</button>
        </div>
      </div>
    </div>
      </div>
    </div>

    <!-- 站点页脚 -->
    <footer class="site-footer">
      <p v-if="site.footer_text" class="footer-line">{{ site.footer_text }}</p>
    </footer>

    <!-- 新增/编辑密码弹窗 -->
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

    <!-- 编辑用户弹窗 -->
    <div v-if="editingUser !== null" class="mask" @click.self="editingUser = null">
      <div class="modal">
        <h2>{{ $t('users.editTitle') }} #{{ editingUser.id }}</h2>
        <label>{{ $t('users.username') }}</label>
        <input v-model="userForm.username" />
        <label>{{ $t('users.email') }}</label>
        <input v-model="userForm.email" />
        <label>{{ $t('users.password') }}</label>
        <input v-model="userForm.password" type="password" :placeholder="$t('users.passwordHint')" />
        <label>{{ $t('users.role') }}</label>
        <select v-model="userForm.role">
          <option value="user">{{ $t('users.roleUser') }}</option>
          <option value="admin">{{ $t('users.roleAdmin') }}</option>
        </select>
        <label>{{ $t('users.active') }}</label>
        <select v-model="userForm.status">
          <option value="active">{{ $t('users.active') }}</option>
          <option value="disabled">{{ $t('users.disabled') }}</option>
        </select>
        <div class="modal-actions">
          <button class="btn-ghost" @click="editingUser = null">{{ $t('modal.cancel') }}</button>
          <button class="btn-primary" @click="saveUser">{{ $t('modal.save') }}</button>
        </div>
      </div>
    </div>

    <!-- 我的账号弹窗 -->
    <div v-if="showMe" class="mask" @click.self="showMe = false">
      <div class="modal">
        <h2>{{ $t('me.title') }}</h2>
        <div class="avatar-row">
          <img v-if="myAvatar" :src="avatarUrl(myId)" class="avatar avatar-lg" alt="" />
          <span v-else class="avatar avatar-lg avatar-fallback">{{ (username || '?').charAt(0).toUpperCase() }}</span>
          <button class="btn-ghost" @click="triggerAvatar">{{ $t('me.changeAvatar') }}</button>
          <input ref="avatarInput" type="file" accept="image/*" style="display:none" @change="uploadAvatar" />
        </div>
        <label>{{ $t('me.username') }}</label>
        <input v-model="meForm.username" />
        <label>{{ $t('me.email') }}</label>
        <input v-model="meForm.email" />
        <label>{{ $t('me.currentPassword') }}</label>
        <input v-model="meForm.current_password" type="password" />
        <label>{{ $t('me.newPassword') }}</label>
        <input v-model="meForm.new_password" type="password" :placeholder="$t('users.passwordHint')" />
        <div class="modal-actions">
          <button class="btn-ghost" @click="showMe = false">{{ $t('modal.cancel') }}</button>
          <button class="btn-primary" @click="saveMe">{{ $t('modal.save') }}</button>
        </div>
      </div>
    </div>

    <!-- 删除确认弹窗 -->
    <div v-if="confirmBox" class="mask" @click.self="confirmBox = null">
      <div class="modal modal-confirm">
        <h2>{{ $t('modal.confirmTitle') }}</h2>
        <p class="confirm-text">{{ confirmBox.text }}</p>
        <div class="modal-actions">
          <button class="btn-ghost" @click="confirmBox = null">{{ $t('modal.cancel') }}</button>
          <button class="btn-danger" @click="confirmBox.onOk(); confirmBox = null">{{ $t('modal.ok') }}</button>
        </div>
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
      token: localStorage.getItem('token') || '',
      username: localStorage.getItem('username') || '',
      role: localStorage.getItem('role') || '',
      myId: 0,
      meEmail: '',
      myAvatar: '',
      avatarVersion: 0,
      userError: '',
      loginForm: { username: '', password: '' },
      setupForm: { username: '', password: '' },
      isAdminLogin: location.pathname.startsWith('/admin-login'),
      gateMode: 'login',
      initialized: true,
      verifyMode: 'none',
      regForm: { username: '', email: '', password: '', code: '' },
      resetForm: { email: '', code: '', password: '' },
      error: '',
      msg: '',
      tab: 'passwords',
      logs: [],
      settings: { smtp: { host: '', port: 465, username: '', password: '', from: '', ssl: false, vendor: 'qq' }, email_verify_mode: 'none', allow_registration: false, password_min_length: 6, password_require_complex: false },
      site: { footer_text: '' },
      allowReg: false,
      portAuto: true,
      settingsSecureMode: 'ssl',
      entries: [],
      search: '',
      revealed: new Set(),
      editing: null,
      form: this.emptyForm(),
      formShow: false,
      users: [],
      newUser: { username: '', email: '', password: '', role: 'user' },
      editingUser: null,
      userForm: { username: '', email: '', password: '', role: 'user', status: 'active' },
      showMe: false,
      confirmBox: null,
      menuOpen: false,
      meForm: { username: '', email: '', current_password: '', new_password: '' },
      importFormat: '',
      ioFormat: '',
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
    isAdmin() {
      return this.role === 'admin' || this.role === 'superadmin'
    },
    gateTitle() {
      if (this.gateMode === 'setup') return this.$t('login.setupTitle')
      if (this.gateMode === 'register') return this.$t('login.register')
      if (this.gateMode === 'forgot') return this.$t('login.forgot')
      return this.isAdminLogin ? this.$t('login.adminTitle') : this.$t('app.title')
    },
    smtpVendors() {
      const t = this.$t
      return [
        { value: 'qq', label: 'QQ邮箱' },
        { value: '126', label: '126邮箱' },
        { value: '163', label: '163邮箱' },
        { value: 'gmail', label: 'Gmail' },
        { value: 'outlook', label: 'Outlook' },
        { value: 'custom', label: t('settings.vendorCustom') }
      ]
    },
    vendorTip() {
      const map = {
        qq: this.$t('settings.qqTip'),
        '126': this.$t('settings.126Tip'),
        '163': this.$t('settings.163Tip'),
        gmail: this.$t('settings.gmailTip'),
        outlook: this.$t('settings.outlookTip')
      }
      return map[this.settings.smtp.vendor] || ''
    }
  },
  async mounted() {
    this.loadPublicSettings()
    try {
      const st = await api.status()
      this.initialized = st.initialized
      if (!st.initialized) this.gateMode = 'setup'
    } catch (e) {
      /* ignore */
    }
    if (this.token) await this.init()
  },
  methods: {
    emptyForm() {
      return { id: 0, title: '', username: '', password: '', url: '', category: '', notes: '' }
    },
    changeLang() {
      this.$i18n.locale = this.lang
      localStorage.setItem('locale', this.lang)
    },
    fieldName(key) {
      return this.$t(key).replace(' *', '')
    },
    async init() {
      try {
        const me = await api.me(this.token)
        this.username = me.username
        this.role = me.role
        this.myId = me.id
        this.meEmail = me.email || ''
        this.myAvatar = me.avatar || ''
        await this.loadEntries()
        if (this.isAdmin) await this.loadUsers()
      } catch (e) {
        this.clearAuth()
        this.error = String(e.message || e)
      }
    },
    async loadPublicSettings() {
      try {
        const r = await api.publicSettings()
        this.verifyMode = r.email_verify_mode || 'none'
        this.allowReg = !!r.allow_registration
        if (r.site) this.site = this.normalizeSite(r.site)
      } catch (e) {
        /* ignore */
      }
    },
    normalizeSite(s) {
      return { footer_text: s.footer_text || '' }
    },
    switchGate(m) {
      this.gateMode = m
      this.error = ''
      this.msg = ''
    },
    async doSetup() {
      this.error = ''
      try {
        const r = await api.setup(this.setupForm.username, this.setupForm.password)
        this.applyAuth(r)
        await this.init()
      } catch (e) {
        this.error = String(e.message || e)
      }
    },
    async sendRegCode() {
      this.error = ''
      try {
        await api.sendRegisterCode(this.regForm.email)
        this.msg = this.$t('msg.codeSent')
      } catch (e) {
        this.error = String(e.message || e)
      }
    },
    async sendResetCode() {
      this.error = ''
      try {
        await api.sendResetCode(this.resetForm.email)
        this.msg = this.$t('msg.codeSent')
      } catch (e) {
        this.error = String(e.message || e)
      }
    },
    async register() {
      this.error = ''
      try {
        const r = await api.register(this.regForm)
        this.applyAuth(r)
        await this.init()
      } catch (e) {
        this.error = String(e.message || e)
      }
    },
    async resetPassword() {
      this.error = ''
      try {
        await api.resetPassword(this.resetForm)
        this.msg = this.$t('msg.resetDone')
        this.resetForm = { email: '', code: '', password: '' }
        this.gateMode = 'login'
      } catch (e) {
        this.error = String(e.message || e)
      }
    },
    applyAuth(r) {
      this.token = r.token
      this.username = r.username
      this.role = r.role
      localStorage.setItem('token', r.token)
      localStorage.setItem('username', r.username)
      localStorage.setItem('role', r.role)
    },
    async login() {
      this.error = ''
      try {
        const r = await api.login(this.loginForm.username, this.loginForm.password)
        if (this.isAdminLogin && r.role === 'user') {
          this.error = this.$t('login.adminOnly')
          return
        }
        this.applyAuth(r)
        await this.init()
      } catch (e) {
        this.error = String(e.message || e)
      }
    },
    logout() {
      this.clearAuth()
      this.entries = []
      this.users = []
    },
    clearAuth() {
      this.token = ''
      this.username = ''
      this.role = ''
      localStorage.removeItem('token')
      localStorage.removeItem('username')
      localStorage.removeItem('role')
    },
    switchTab(t) {
      this.tab = t
      this.error = ''
      this.msg = ''
      if (t === 'logs') this.loadLogs()
      if (t === 'settings') this.loadSettings()
    },
    async loadEntries() {
      const r = await api.listEntries(this.token)
      this.entries = r.entries || []
    },
    async loadUsers() {
      const r = await api.listUsers(this.token)
      this.users = r.users || []
    },
    async loadLogs() {
      try {
        const r = await api.listLogs(this.token)
        this.logs = r.logs || []
      } catch (e) {
        this.error = String(e.message || e)
      }
    },
    async loadSettings() {
      try {
        const r = await api.getSettings(this.token)
        const smtp = r.smtp || { host: '', port: 465, username: '', password: '', from: '', ssl: false, vendor: 'qq' }
        if (!smtp.vendor) smtp.vendor = 'qq'
        this.settings = {
          smtp,
          email_verify_mode: r.email_verify_mode || 'none',
          allow_registration: !!r.allow_registration,
          password_min_length: r.password_min_length || 6,
          password_require_complex: !!r.password_require_complex
        }
        if (r.site) this.site = this.normalizeSite(r.site)
        // 根据当前配置推算显示用的加密模式
        if (smtp.ssl) this.settingsSecureMode = 'ssl'
        else if (smtp.port === 587 || smtp.port === 2525) this.settingsSecureMode = 'tls'
        else this.settingsSecureMode = 'none'
        this.portAuto = true
      } catch (e) {
        this.error = String(e.message || e)
      }
    },
    onVendorChange() {
      const v = this.settings.smtp.vendor
      const presets = {
        qq:     { host: 'smtp.qq.com',      port: 465, ssl: true,  secure: 'ssl' },
        '126':  { host: 'smtp.126.com',     port: 465, ssl: true,  secure: 'ssl' },
        '163':  { host: 'smtp.163.com',     port: 465, ssl: true,  secure: 'ssl' },
        gmail:  { host: 'smtp.gmail.com',   port: 587, ssl: false, secure: 'tls' },
        outlook:{ host: 'smtp.office365.com', port: 587, ssl: false, secure: 'tls' },
        custom: { host: this.settings.smtp.host || '', port: this.settings.smtp.port || 465, ssl: !!this.settings.smtp.ssl, secure: this.settingsSecureMode || 'ssl' }
      }
      const p = presets[v]
      this.settings.smtp.host = p.host
      this.settings.smtp.port = p.port
      this.settings.smtp.ssl = p.ssl
      this.settingsSecureMode = p.secure
      this.portAuto = true
      // 非自定义模式：用户名与邮箱地址联动
      if (v !== 'custom') {
        this.settings.smtp.username = this.settings.smtp.from || this.settings.smtp.username
      }
    },
    onSecureChange() {
      if (!this.portAuto) return
      const m = this.settingsSecureMode
      const map = { ssl: 465, tls: 587, none: 25 }
      this.settings.smtp.port = map[m]
      this.settings.smtp.ssl = m === 'ssl'
    },
    onPortEdit() {
      this.portAuto = false
    },
    showVendorAuthHelp() {
      alert(this.$t('settings.vendorAuthHelp'))
    },
    onIconError(e) {
      // 图片服务器维护时可能加载失败，隐藏图标保留文字。
      e.target.style.display = 'none'
    },
    async saveSettings() {
      try {
        const s = this.settings.smtp
        // 非自定义厂商：用户名/发件人=邮箱地址，host/port/ssl 按厂商预设
        if (s.vendor !== 'custom') {
          s.username = s.from
          const presets = {
            qq:     { host: 'smtp.qq.com',      port: 465, ssl: true },
            '126':  { host: 'smtp.126.com',     port: 465, ssl: true },
            '163':  { host: 'smtp.163.com',     port: 465, ssl: true },
            gmail:  { host: 'smtp.gmail.com',   port: 587, ssl: false },
            outlook:{ host: 'smtp.office365.com', port: 587, ssl: false }
          }
          const p = presets[s.vendor]
          if (p) { s.host = p.host; s.port = p.port; s.ssl = p.ssl }
        }
        await api.updateSettings({
          vendor: s.vendor,
          host: s.host,
          port: s.port,
          username: s.username,
          password: s.password,
          from: s.from,
          ssl: s.ssl,
          mode: this.settings.email_verify_mode,
          allow_registration: this.settings.allow_registration,
          password_min_length: this.settings.password_min_length,
          password_require_complex: this.settings.password_require_complex,
          site: this.site
        }, this.token)
        this.msg = this.$t('msg.saved')
        await this.loadSettings()
      } catch (e) {
        this.error = String(e.message || e)
      }
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
        if (this.form.id) await api.updateEntry(this.form, this.token)
        else await api.createEntry(this.form, this.token)
        this.editing = null
        this.msg = this.$t('msg.saved')
        await this.loadEntries()
      } catch (e) {
        this.error = String(e.message || e)
      }
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
    askConfirm(text, fn) {
      this.confirmBox = { text, onOk: fn }
    },
    async remove(e) {
      this.askConfirm(this.$t('msg.confirmDelete', { title: e.title }), async () => {
        try {
          await api.deleteEntry(e.id, this.token)
          this.msg = this.$t('msg.deleted')
          await this.loadEntries()
        } catch (err) {
          this.error = String(err.message || err)
        }
      })
    },
    toggleReveal(id) {
      if (this.revealed.has(id)) this.revealed.delete(id)
      else this.revealed.add(id)
    },
    exportTxt() {
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
      const blob = new Blob([lines.join('\n')], { type: 'text/plain;charset=utf-8' })
      const url = URL.createObjectURL(blob)
      const a = document.createElement('a')
      a.href = url
      a.download = this.$t('app.title') + '.txt'
      a.click()
      URL.revokeObjectURL(url)
      this.msg = this.$t('msg.exported', { n: this.entries.length })
    },
    onIO() {
      const v = this.ioFormat
      if (v === 'import-csv' || v === 'import-txt') {
        this.importFormat = v === 'import-txt' ? 'txt' : 'csv'
        this.$refs.fileInput.click()
      } else if (v === 'export-csv') {
        this.exportCsv()
      } else if (v === 'export-txt') {
        this.exportTxt()
      }
      this.ioFormat = ''
    },
    async importFile(e) {
      const file = e.target.files && e.target.files[0]
      const f = this.importFormat
      if (!file) return
      try {
        const text = await file.text()
        const r = f === 'txt' ? await api.importText(text, this.token) : await api.importEntries(text, this.token)
        this.msg = this.$t('msg.imported', { n: r.count })
        await this.loadEntries()
      } catch (err) {
        this.error = String(err.message || err)
      } finally {
        e.target.value = ''
        this.importFormat = ''
      }
    },
    exportCsv() {
      const keys = ['title', 'username', 'password', 'url', 'category', 'notes']
      const header = keys.map((k) => this.fieldName('modal.' + k))
      const rows = [header.map(csvEscape).join(',')]
      for (const e of this.entries) {
        rows.push([e.title, e.username, e.password, e.url, e.category, e.notes].map(csvEscape).join(','))
      }
      const csv = '\ufeff' + rows.join('\r\n')
      const blob = new Blob([csv], { type: 'text/csv;charset=utf-8' })
      const url = URL.createObjectURL(blob)
      const a = document.createElement('a')
      a.href = url
      a.download = this.$t('app.title') + '.csv'
      a.click()
      URL.revokeObjectURL(url)
      this.msg = this.$t('msg.exportedCsv', { n: this.entries.length })
    },
    downloadTemplate() {
      const keys = ['title', 'username', 'password', 'url', 'category', 'notes']
      const header = keys.map((k) => this.fieldName('modal.' + k)).join(',')
      const csv = '\ufeff' + header + '\r\n'
      const blob = new Blob([csv], { type: 'text/csv;charset=utf-8' })
      const url = URL.createObjectURL(blob)
      const a = document.createElement('a')
      a.href = url
      a.download = this.$t('app.title') + ' - ' + this.$t('file.template') + '.csv'
      a.click()
      URL.revokeObjectURL(url)
      this.msg = this.$t('msg.saved')
    },
    async createUser() {
      this.userError = ''
      try {
        await api.createUser(this.newUser, this.token)
        this.newUser = { username: '', email: '', password: '', role: 'user' }
        this.msg = this.$t('msg.userAdded')
        await this.loadUsers()
      } catch (e) {
        this.userError = String(e.message || e)
      }
    },
    downloadUserTemplate() {
      const keys = ['username', 'password', 'email', 'role']
      const header = keys.map((k) => this.$t('users.' + k)).join(',')
      const csv = '\ufeff' + header + '\r\n'
      const blob = new Blob([csv], { type: 'text/csv;charset=utf-8' })
      const url = URL.createObjectURL(blob)
      const a = document.createElement('a')
      a.href = url
      a.download = this.$t('app.title') + ' - ' + this.$t('users.template') + '.csv'
      a.click()
      URL.revokeObjectURL(url)
    },
    async importUsersFile(e) {
      const file = e.target.files && e.target.files[0]
      if (!file) return
      try {
        const text = await file.text()
        const r = await api.importUsers(text, this.token)
        this.msg = this.$t('msg.usersImported', { n: r.count, m: r.skipped })
        await this.loadUsers()
      } catch (err) {
        this.error = String(err.message || err)
      } finally {
        e.target.value = ''
      }
    },
    async removeUser(u) {
      this.askConfirm(this.$t('msg.confirmDeleteUser', { username: u.username }), async () => {
        try {
          await api.deleteUser(u.id, this.token)
          this.msg = this.$t('msg.userDeleted')
          await this.loadUsers()
        } catch (err) {
          this.userError = String(err.message || err)
        }
      })
    },
    async toggleStatus(u) {
      const next = u.status === 'active' ? 'disabled' : 'active'
      try {
        await api.updateUserStatus(u.id, next, this.token)
        this.msg = next === 'active' ? this.$t('msg.enabled') : this.$t('msg.disabled')
        await this.loadUsers()
      } catch (e) {
        this.userError = String(e.message || e)
      }
    },
    roleLabel(role) {
      if (role === 'superadmin') return this.$t('role.superadmin')
      if (role === 'admin') return this.$t('role.admin')
      return this.$t('role.user')
    },
    avatarUrl(id) {
      // 相对路径，兼容统一网关前缀
      return 'api/avatar/' + id + '?v=' + this.avatarVersion
    },
    canManageUser(u) {
      if (u.id === this.myId) return false
      if (this.role === 'superadmin') return true
      if (this.role === 'admin') return u.role !== 'superadmin'
      return false
    },
    openMe() {
      this.meForm = { username: this.username, email: this.meEmail, current_password: '', new_password: '' }
      this.showMe = true
    },
    triggerAvatar() {
      this.$refs.avatarInput.click()
    },
    async uploadAvatar(e) {
      const file = e.target.files && e.target.files[0]
      if (!file) return
      try {
        const dataUrl = await new Promise((resolve, reject) => {
          const fr = new FileReader()
          fr.onload = () => resolve(fr.result)
          fr.onerror = reject
          fr.readAsDataURL(file)
        })
        const base64 = String(dataUrl).split(',')[1] || ''
        const ext = (file.name.split('.').pop() || 'png').toLowerCase()
        const r = await api.uploadAvatar({ data: base64, ext }, this.token)
        this.myAvatar = r.avatar
        this.avatarVersion++
        this.msg = this.$t('msg.avatarUpdated')
      } catch (err) {
        this.error = String(err.message || err)
      } finally {
        e.target.value = ''
      }
    },
    async saveMe() {
      try {
        await api.updateMe(this.meForm, this.token)
        this.showMe = false
        this.msg = this.$t('msg.meUpdated')
        const me = await api.me(this.token)
        this.username = me.username
        this.meEmail = me.email || ''
        localStorage.setItem('username', me.username)
      } catch (e) {
        this.error = String(e.message || e)
      }
    },
    startEditUser(u) {
      this.userForm = { username: u.username, email: u.email || '', password: '', role: u.role, status: u.status }
      this.editingUser = u
    },
    async saveUser() {
      try {
        await api.updateUser(this.editingUser.id, this.userForm, this.token)
        this.editingUser = null
        this.msg = this.$t('msg.userUpdated')
        await this.loadUsers()
      } catch (e) {
        this.userError = String(e.message || e)
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
  background: #fff;
  padding: 32px;
  border-radius: 12px;
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.08);
  display: flex;
  flex-direction: column;
  gap: 12px;
}
.gate-card h1 {
  font-size: 20px;
  text-align: center;
  margin-bottom: 8px;
}
.gate-links {
  display: flex;
  justify-content: space-between;
  gap: 8px;
}
.link {
  background: transparent;
  color: var(--fnos-primary);
  padding: 4px;
  font-size: 13px;
}
.code-row {
  display: flex;
  gap: 8px;
}
.code-row input {
  flex: 1;
}
.code-row button {
  flex-shrink: 0;
  width: auto;
}
.inline {
  margin: 8px 0;
}
.lang-select {
  background: #f9fafb;
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
  padding: 16px 24px;
  max-width: 1100px;
  margin: 0 auto;
}
.topbar {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 12px;
  padding-right: 120px;
}
.user-info {
  display: flex;
  align-items: center;
  gap: 12px;
  color: #6b7280;
}
.user-menu {
  display: flex;
  align-items: center;
  gap: 8px;
  cursor: pointer;
  position: relative;
  padding: 4px 8px;
  border-radius: 6px;
}
.user-menu:hover {
  background: #f3f4f6;
}
.user-name {
  font-weight: 500;
  max-width: 140px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.caret {
  font-size: 10px;
  color: #9ca3af;
}
.user-dropdown {
  position: absolute;
  top: calc(100% + 6px);
  right: 0;
  background: #fff;
  border: 1px solid #e5e7eb;
  border-radius: 8px;
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.12);
  min-width: 160px;
  z-index: 10001;
  overflow: hidden;
}
.dropdown-item {
  display: block;
  width: 100%;
  text-align: left;
  background: transparent;
  border: none;
  border-radius: 0;
  padding: 10px 14px;
  font-size: 14px;
  color: #1f2937;
}
.dropdown-item:hover {
  background: #f3f4f6;
}
.menu-backdrop {
  position: fixed;
  inset: 0;
  z-index: 10000;
}
.avatar {
  width: 32px;
  height: 32px;
  border-radius: 50%;
  object-fit: cover;
  flex-shrink: 0;
}
.avatar-lg {
  width: 56px;
  height: 56px;
}
.avatar-fallback {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  background: var(--fnos-primary);
  color: #fff;
  font-weight: 600;
}
.avatar-row {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 8px;
}
.layout {
  flex: 1;
  display: flex;
  min-height: 0;
  gap: 16px;
}
.sidebar {
  width: 180px;
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 8px 0;
  border-right: 1px solid #e5e7eb;
}
.sidebar button {
  background: transparent;
  color: #6b7280;
  border-radius: 6px;
  padding: 10px 14px;
  text-align: left;
  width: 100%;
}
.sidebar button:hover {
  background: #f3f4f6;
}
.sidebar button.active {
  color: var(--fnos-primary);
  background: var(--fnos-primary-light);
  font-weight: 600;
}
.content {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  overflow-y: auto;
}
.panel {
  flex: 1;
  min-height: 0;
}
.toolbar {
  display: flex;
  gap: 8px;
  margin-bottom: 14px;
  align-items: center;
  flex-wrap: wrap;
}
.toolbar-main,
.toolbar-sub {
  display: contents;
}
.toolbar input {
  flex: 1;
}
.add-user {
  display: flex;
  gap: 8px;
  width: 100%;
}
.add-user select {
  width: 140px;
}
.list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.row {
  display: flex;
  align-items: center;
  gap: 16px;
  background: #fff;
  padding: 12px 16px;
  border-radius: 8px;
  border: 1px solid #e5e7eb;
}
.main {
  flex: 1;
  min-width: 0;
}
.entry {
  display: flex;
  flex-direction: column;
  gap: 8px;
  background: #fff;
  padding: 12px 16px;
  border-radius: 8px;
  border: 1px solid #e5e7eb;
}
.entry-head {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 8px;
}
.entry-meta {
  display: flex;
  flex-direction: column;
  gap: 4px;
  color: #6b7280;
  font-size: 12px;
}
.meta-line {
  display: flex;
  align-items: center;
  gap: 6px;
  flex-wrap: wrap;
}
.label {
  color: #9ca3af;
  flex-shrink: 0;
}
.title {
  font-weight: 600;
}
.meta {
  display: flex;
  gap: 12px;
  color: #6b7280;
  font-size: 12px;
  margin-top: 4px;
  align-items: center;
}
.tag {
  background: #eef2ff;
  color: #4f46e5;
  padding: 1px 8px;
  border-radius: 10px;
}
.ok {
  color: #16a34a;
}
.off {
  color: #dc2626;
}
.pw {
  display: flex;
  align-items: center;
  gap: 8px;
}
.mono {
  font-family: 'Consolas', monospace;
}
.field {
  cursor: pointer;
}
.field:hover {
  color: var(--fnos-primary);
}
.copy-btn {
  background: transparent;
  color: var(--fnos-primary);
  border: 1px solid var(--fnos-primary);
  padding: 2px 8px;
  font-size: 12px;
  border-radius: 4px;
  flex-shrink: 0;
  cursor: pointer;
}
.copy-btn:hover {
  background: var(--fnos-primary-light);
}
.ops {
  display: flex;
  gap: 6px;
}
.empty {
  text-align: center;
  color: #9ca3af;
  margin-top: 40px;
}
.mask {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.4);
  display: flex;
  align-items: center;
  justify-content: center;
}
.modal {
  width: 420px;
  max-height: 90%;
  overflow-y: auto;
  background: #fff;
  border-radius: 12px;
  padding: 24px;
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.modal h2 {
  margin-bottom: 8px;
}
.modal-confirm {
  width: 360px;
}
.confirm-text {
  color: #374151;
  line-height: 1.6;
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
/* Settings Card */
.settings-card {
  background: #fff;
  border-radius: 12px;
  border: 1px solid #e5e7eb;
  overflow: hidden;
}
.card-header {
  padding: 10px 16px;
  background: #f9fafb;
  border-bottom: 1px solid #e5e7eb;
}
.card-header h3 {
  font-size: 13px;
  font-weight: 600;
  color: #111827;
}
.card-body {
  padding: 12px 16px;
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.card-footer {
  padding: 10px 16px;
  border-top: 1px solid #e5e7eb;
  display: flex;
  justify-content: flex-end;
}
.card-divider {
  height: 1px;
  background: #e5e7eb;
}
.form-label {
  color: #374151;
  font-size: 12px;
  font-weight: 500;
  margin-bottom: -2px;
}
.form-label .link-inline {
  color: var(--fnos-primary);
  font-weight: 400;
  margin-left: 8px;
  text-decoration: none;
  cursor: pointer;
}
.form-label .link-inline:hover {
  text-decoration: underline;
}
.select-block {
  width: 100%;
}
.settings-card input,
.settings-card select {
  padding: 6px 10px;
  font-size: 13px;
}
.form-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 10px;
}
.form-col {
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.form-col .form-label {
  margin-bottom: 0;
}
.tip {
  background: #eff6ff;
  color: var(--fnos-primary-active);
  font-size: 11px;
  padding: 6px 10px;
  border-radius: 6px;
  border: 1px solid #bfdbfe;
}
/* Site Footer */
.site-footer {
  flex-shrink: 0;
  margin-top: 12px;
  padding-top: 12px;
  border-top: 1px solid #e5e7eb;
  text-align: center;
  color: #9ca3af;
  font-size: 12px;
}
.footer-line {
  margin-bottom: 6px;
}
.footer-links {
  display: flex;
  flex-wrap: wrap;
  gap: 6px 16px;
  justify-content: center;
  align-items: center;
}
.footer-link {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  color: #6b7280;
  text-decoration: none;
  white-space: nowrap;
}
a.footer-link:hover {
  color: var(--fnos-primary);
}
.beian-icon {
  width: 14px;
  height: 14px;
  flex-shrink: 0;
}

/* 手机端布局适配 */
@media (max-width: 640px) {
  .gate-card {
    width: calc(100% - 32px);
    padding: 24px;
  }
  .app {
    padding: 12px 14px;
  }
  .topbar {
    flex-direction: column;
    align-items: flex-start;
    gap: 10px;
    padding-right: 0;
  }
  .user-info {
    flex-wrap: wrap;
    gap: 8px;
  }
  .layout {
    flex-direction: column;
  }
  .sidebar {
    width: 100%;
    flex-direction: row;
    overflow-x: auto;
    -webkit-overflow-scrolling: touch;
    border-right: none;
    border-bottom: 1px solid #e5e7eb;
    padding: 4px 0;
  }
  .sidebar button {
    padding: 8px 14px;
    white-space: nowrap;
    flex-shrink: 0;
    width: auto;
  }
  .toolbar {
    flex-direction: column;
    align-items: stretch;
  }
  .toolbar-main,
  .toolbar-sub {
    display: flex;
    gap: 8px;
    align-items: center;
  }
  .toolbar-main input {
    flex: 1;
    min-width: 0;
  }
  .toolbar-sub {
    flex-wrap: nowrap;
  }
  .entry-head {
    flex-wrap: wrap;
  }
  .add-user {
    flex-wrap: wrap;
  }
  .add-user input,
  .add-user select {
    flex: 1 1 100%;
    width: 100%;
  }
  .row {
    flex-direction: column;
    align-items: stretch;
    gap: 10px;
  }
  .row .pw,
  .row .ops {
    justify-content: space-between;
  }
  .modal {
    width: calc(100% - 32px);
    max-height: 92%;
  }
  .form-grid {
    grid-template-columns: 1fr;
  }
}
</style>
