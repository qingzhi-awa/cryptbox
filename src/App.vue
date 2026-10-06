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
        <button v-if="!loggedIn" class="btn-primary" @click="openSync">{{ $t('sync.login') }}</button>
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

    <div v-if="msg" class="toast toast-msg">{{ msg }}</div>
    <div v-if="error" class="toast toast-error">{{ error }}</div>

    <div class="layout">
      <nav class="sidebar">
        <button :class="{ active: tab === 'passwords' }" @click="switchTab('passwords')">{{ $t('tabs.passwords') }}</button>
        <button :class="{ active: tab === 'trash' }" @click="switchTab('trash')">{{ $t('tabs.trash') }}</button>
        <button :class="{ active: tab === 'logs' }" @click="switchTab('logs')">{{ $t('tabs.logs') }}</button>
        <button :class="{ active: tab === 'settings' }" @click="switchTab('settings')">{{ $t('tabs.settings') }}</button>
        <button :class="{ active: tab === 'about' }" @click="switchTab('about')">{{ $t('tabs.about') }}</button>
        <div class="sidebar-footer">
          <button class="theme-toggle" @click="cycleTheme" :title="themeLabel">
            <span class="theme-icon">{{ themeIcon }}</span>
          </button>
          <a
            class="gh-link"
            href="https://github.com/qingzhi-awa/cryptbox"
            target="_blank"
            rel="noopener"
            :title="$t('about.github')"
          >
            <svg viewBox="0 0 16 16" width="18" height="18" fill="currentColor" aria-hidden="true">
              <path d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27s1.36.09 2 .27c1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.01 8.01 0 0 0 16 8c0-4.42-3.58-8-8-8z" />
            </svg>
          </a>
        </div>
      </nav>

      <div class="content">
        <!-- 密码管理 -->
        <template v-if="tab === 'passwords'">
          <div class="toolbar">
            <input v-model="search" :placeholder="$t('toolbar.search')" />
            <button class="btn-primary" @click="startAdd">{{ $t('toolbar.add') }}</button>
            <select v-model="ioFormat" class="btn-ghost menu-select" @change="onIO">
              <option value="" selected>{{ $t('toolbar.io') }}</option>
              <option value="import-csv">{{ $t('toolbar.import') }} CSV</option>
              <option value="import-txt">{{ $t('toolbar.import') }} TXT</option>
              <option value="export-csv">{{ $t('toolbar.export') }} CSV</option>
              <option value="export-txt">{{ $t('toolbar.export') }} TXT</option>
            </select>
            <button class="btn-ghost" @click="downloadTemplate">{{ $t('toolbar.template') }}</button>
            <template v-if="loggedIn">
              <button class="btn-primary" @click="doPush">{{ $t('sync.upload') }}</button>
              <button class="btn-ghost" @click="doPull">{{ $t('sync.download') }}</button>
            </template>
          </div>

    <div class="list grid-list">
      <div
        v-for="e in filtered"
        :key="e.id"
        class="entry"
        :class="{ 'entry-pinned': e.pinned, 'entry-dragging': dragId === e.id, 'entry-drop-target': dragOverId === e.id && dragId !== e.id }"
        :draggable="canDrag"
        :title="canDrag ? $t('list.dragHint') : ''"
        @dragstart="onDragStart(e)"
        @dragover.prevent="onDragOver(e)"
        @dragleave="onDragLeave(e)"
        @drop.prevent="onDrop(e)"
        @dragend="onDragEnd"
      >
        <div class="entry-head">
          <div class="title">
            <span v-if="e.pinned" class="pin-mark" aria-hidden="true">
              <svg viewBox="0 0 24 24" width="12" height="12">
                <path d="M12 2l2.4 6.2 6.6.5-5 4.3 1.5 6.4L12 16l-5.5 3.4 1.5-6.4-5-4.3 6.6-.5z" fill="currentColor"></path>
              </svg>
            </span>
            <span>{{ e.title }}</span>
          </div>
          <div class="ops">
            <button class="btn-ghost pin-btn" :class="{ 'pin-on': e.pinned }" @click="togglePin(e)">
              {{ e.pinned ? $t('list.unpin') : $t('list.pin') }}
            </button>
            <button class="btn-ghost" @click="startEdit(e)">{{ $t('list.edit') }}</button>
            <button class="btn-danger" @click="remove(e)">{{ $t('list.delete') }}</button>
          </div>
        </div>
        <div class="entry-meta">
          <div class="meta-line">
            <span class="label">{{ $t('modal.url') }}:</span>
            <span class="field" @click="copyText(e.url)">{{ e.url ? (revealed.has(e.id + ':url') ? e.url : '••••••••') : $t('list.none') }}</span>
            <button v-if="e.url" class="mini" @click="toggleReveal(e.id, 'url')">
              {{ revealed.has(e.id + ':url') ? $t('list.hide') : $t('list.show') }}
            </button>
            <button v-if="e.url" class="copy-btn" @click="copyText(e.url)">{{ $t('list.copy') }}</button>
          </div>
          <div class="meta-line">
            <span class="label">{{ $t('modal.username') }}:</span>
            <span class="field" @click="copyText(e.username)">{{ e.username ? (revealed.has(e.id + ':user') ? e.username : '••••••••') : $t('list.none') }}</span>
            <button v-if="e.username" class="mini" @click="toggleReveal(e.id, 'user')">
              {{ revealed.has(e.id + ':user') ? $t('list.hide') : $t('list.show') }}
            </button>
            <button v-if="e.username" class="copy-btn" @click="copyText(e.username)">{{ $t('list.copy') }}</button>
          </div>
          <div class="meta-line">
            <span class="label">{{ $t('modal.password') }}:</span>
            <span class="mono field" @click="copyText(e.password)">{{ revealed.has(e.id + ':pwd') ? e.password : '••••••••' }}</span>
            <button class="mini" @click="toggleReveal(e.id, 'pwd')">
              {{ revealed.has(e.id + ':pwd') ? $t('list.hide') : $t('list.show') }}
            </button>
            <button class="copy-btn" @click="copyText(e.password)">{{ $t('list.copy') }}</button>
          </div>
          <div class="meta-line">
            <span class="label">{{ $t('modal.category') }}:</span>
            <span class="field" @click="copyText(e.category)">{{ e.category || $t('list.none') }}</span>
          </div>
          <div class="meta-line">
            <span class="label">{{ $t('modal.notes') }}:</span>
            <span class="field" @click="copyText(e.notes)">{{ e.notes || $t('list.none') }}</span>
          </div>
        </div>
      </div>
      <div v-if="filtered.length === 0" class="empty">{{ $t('list.empty') }}</div>
    </div>
        </template>

        <!-- 回收站 -->
        <div v-else-if="tab === 'trash'" class="panel">
          <h2 class="panel-title">{{ $t('trash.title') }}</h2>
          <div class="trash-list">
            <div v-for="e in trash" :key="e.id" class="trash-item">
              <div class="trash-main">
                <div class="title">{{ e.title }}</div>
                <div class="meta">{{ e.username || '—' }}</div>
              </div>
              <div class="ops">
                <button class="btn-ghost" @click="doRestore(e.id)">{{ $t('trash.restore') }}</button>
                <button class="btn-danger" @click="doPurge(e.id)">{{ $t('trash.purge') }}</button>
              </div>
            </div>
            <div v-if="trash.length === 0" class="empty">{{ $t('trash.empty') }}</div>
          </div>
          <div class="modal-actions">
            <button class="btn-danger" :disabled="trash.length === 0" @click="doEmptyTrash">{{ $t('trash.emptyTrash') }}</button>
          </div>
        </div>

        <!-- 本机日志 -->
        <div v-else-if="tab === 'logs'" class="panel">
          <div class="panel-head">
            <h2 class="panel-title">{{ $t('logs.title') }}</h2>
            <div class="panel-ops">
              <button class="btn-ghost" @click="loadLogs">{{ $t('logs.refresh') }}</button>
              <button class="btn-danger" :disabled="logs.length === 0" @click="doClearLogs">{{ $t('logs.clear') }}</button>
            </div>
          </div>
          <p class="panel-hint">{{ $t('logs.hint') }}</p>
          <div class="log-list">
            <div v-for="(l, i) in logs" :key="i" class="log-line"><code>{{ l }}</code></div>
            <div v-if="logs.length === 0" class="empty">{{ $t('logs.empty') }}</div>
          </div>
        </div>

        <!-- 设置 -->
        <div v-else-if="tab === 'settings'" class="panel">
          <h2 class="panel-title">{{ $t('settings.title') }}</h2>
          <label class="checkbox-row">
            <input type="checkbox" v-model="settingsForm.autostart" />
            <span>{{ $t('settings.autostart') }}</span>
          </label>
          <label class="checkbox-row">
            <input type="checkbox" v-model="settingsForm.autosync" />
            <span>{{ $t('settings.autosync') }}</span>
          </label>
          <label class="checkbox-row" v-if="loggedIn">
            <input type="checkbox" v-model="pinSync" @change="changePinSync" />
            <span>{{ $t('settings.pinSync') }}</span>
          </label>
          <div class="settings-group">
            <div class="settings-label">{{ $t('settings.priority') }}</div>
            <label class="radio-row">
              <input type="radio" value="local" v-model="settingsForm.priority" />
              <span>{{ $t('settings.local') }}</span>
            </label>
            <label class="radio-row">
              <input type="radio" value="server" v-model="settingsForm.priority" />
              <span>{{ $t('settings.server') }}</span>
            </label>
            <label class="radio-row">
              <input type="radio" value="merge" v-model="settingsForm.priority" />
              <span>{{ $t('settings.merge') }}</span>
            </label>
          </div>
          <div class="settings-group">
            <div class="settings-label">{{ $t('settings.recycle') }}</div>
            <label class="checkbox-row">
              <input type="checkbox" v-model="settingsForm.recycle" />
              <span>{{ $t('settings.recycleOn') }}</span>
            </label>
            <label class="form-row">
              <span>{{ $t('settings.recycleDays') }}</span>
              <input v-model.number="settingsForm.recycleDays" type="number" min="1" max="3650" />
            </label>
          </div>
          <p class="settings-hint">{{ $t('settings.hint') }}</p>
          <div class="modal-actions">
            <button class="btn-primary" @click="saveSettings">{{ $t('modal.save') }}</button>
          </div>
        </div>

        <!-- 关于 -->
        <div v-else-if="tab === 'about'" class="panel">
          <h2 class="panel-title">{{ $t('about.title') }}</h2>
          <p class="about-line">{{ $t('app.title') }} {{ $t('about.version') }}：{{ clientVersion || '—' }}</p>
          <p class="about-line">
            {{ $t('about.github') }}：<a class="about-link" href="https://github.com/qingzhi-awa/cryptbox" target="_blank" rel="noopener">github.com/qingzhi-awa/cryptbox</a>
          </p>
          <p class="about-line about-desc">{{ $t('about.desc') }}</p>
        </div>
      </div>
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
          <input v-model="server" :placeholder="$t('sync.serverPlaceholder')" @blur="loadTrustInfo" />
          <button class="btn-ghost scan-btn" @click="scanLan">{{ $t('sync.scan') }}</button>
        </div>
        <!-- 已信任的证书指纹：便于用户与服务器端展示值核对 -->
        <div v-if="trustedFingerprint" class="fp-box inline-fp">
          <div class="fp-label">{{ $t('sync.fpLabel') }}</div>
          <div class="fp-value">{{ trustedFingerprint }}</div>
        </div>
        <!-- 已确认的服务器地址（可在原生确认框之外撤销，PT-11） -->
        <div v-if="allowedServers.length" class="allowed-list">
          <div class="allowed-title">{{ $t('sync.allowedTitle') }}</div>
          <div v-for="s in allowedServers" :key="s" class="allowed-item">
            <span class="allowed-host">{{ s }}</span>
            <button class="link-btn" @click="removeAllowed(s)">{{ $t('sync.allowedRemove') }}</button>
          </div>
        </div>
        <div v-if="scanning" class="scan-status">{{ $t('sync.scanning') }}</div>
        <div v-if="lanServers && lanServers.length" class="lan-list">
          <button v-for="s in lanServers" :key="s.url" class="lan-item" @click="pickLan(s)">
            <span class="lan-badge" :class="s.secure ? 'lan-secure' : 'lan-plain'">
              {{ s.secure ? 'HTTPS' : 'HTTP' }}
            </span>
            <span class="lan-url">{{ s.url }}</span>
          </button>
        </div>
        <!-- 明文模式风险提示（用户显式输入 http:// 时） -->
        <p v-if="isPlaintext" class="plaintext-warn">
          {{ $t('sync.plaintextWarn') }}
          <button class="link-btn" @click="useHttps">{{ $t('sync.useHttps') }}</button>
        </p>
        <p v-if="secretDegraded" class="secret-warn">{{ $t('sync.secretFileWarn') }}</p>
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
        <!-- 密码曾被重置时的恢复路径：旧密码恢复（数据无损）或清空重建 -->
        <template v-if="vaultRecovery">
          <p class="recovery-hint">{{ $t('sync.recoverPrompt') }}</p>
          <label>{{ $t('sync.recoverOldPassword') }}</label>
          <input v-model="oldPassword" type="password" />
          <div class="modal-actions" style="margin-top: 8px">
            <button class="btn-ghost" @click="resetVaultRemote">{{ $t('sync.resetVault') }}</button>
            <button class="btn-primary" @click="recoverVault">{{ $t('sync.recoverSubmit') }}</button>
          </div>
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
import { listen } from '@tauri-apps/api/event'
import { getVersion } from '@tauri-apps/api/app'

function csvEscape(v) {
  v = String(v == null ? '' : v)
  // 公式注入防护：= + - @ 等开头的字段会被 Excel/WPS 当作公式执行，前置单引号强制按文本处理。
  // R12-01：必须覆盖**前导空白**——Excel 会忽略字段开头的空格/Tab，`" =1+1"` 同样会触发公式。
  if (/^[\s]*[=+\-@\t\r]/.test(v)) v = "'" + v
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
      // 拖拽排序状态：dragId 为被拖动条目，dragOverId 为当前悬停目标
      dragId: null,
      dragOverId: null,
      editing: null,
      form: this.emptyForm(),
      formShow: false,
      serverLoginOpen: false,
      userMenuOpen: false,
      serverLoginMode: 'login',
      // 密码重置后的恢复模式：用旧密码恢复原密码库，或清空后重新开始。
      vaultRecovery: false,
      oldPassword: '',
      server: '',
      serverUsername: '',
      serverPassword: '',
      serverEmail: '',
      serverCode: '',
      rememberMe: false,
      lanServers: [],
      scanning: false,
      // 已确认的服务器地址（PT-11 白名单，可在同步弹窗中撤销）
      allowedServers: [],
      // 当前已信任的证书指纹（展示用，便于用户与服务器端核对）
      trustedFingerprint: '',
      // 凭据库后端："keyring" | "file"（file 表示已降级，需提示用户）
      secretBackend: '',
      // 登录态（令牌本体由 Rust 侧保管，前端不持有原始 JWT，见 PT-06）
      loggedIn: false,
      // R11-11：服务端下发的头像文件名字符串不再写入 localStorage——它对不可信
      // 对端数据而言是多余的持久化面，本字段仅用于 parseInt 取 id 与 v-if，内存态足够。
      myAvatar: '',
      avatarDataUrl: '',
      ioFormat: '',
      toastTimer: null,
      idleTimer: null,
      idleTimeout: 5 * 60 * 1000, // 5 分钟无操作自动锁定
      // 左侧边栏当前页签：passwords / trash / logs / settings / about（无用户管理）
      tab: 'passwords',
      // 本机操作日志（client.log，新在前）
      logs: [],
      // 客户端版本号（关于页展示）
      clientVersion: '',
      // 颜色模式：auto（跟随系统）/ light / dark，循环切换
      themeMode: localStorage.getItem('theme') || 'auto',
      settingsForm: { autostart: false, autosync: false, priority: 'local', recycle: true, recycleDays: 30 },
      pinSync: false,
      recycleEnabled: true,
      trash: [],
      confirmBox: null,
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
    // 拖拽排序：仅在未搜索（列表即完整顺序）且条目多于一条时启用，
    // 否则拖动的只是筛选结果，落库顺序会与所见不符。
    canDrag() {
      return !this.search.trim() && this.entries.length > 1
    },
    avatarUrl() {
      // 头像经 Rust 侧带认证拉取（data URL）；头像端点已要求登录，<img> 直连无法携带凭据。
      return this.avatarDataUrl
    },
    // 是否使用明文 HTTP（提示风险并提供一键改用 HTTPS）
    isPlaintext() {
      return /^http:\/\//i.test((this.server || '').trim())
    },
    // 凭据库是否已降级为本地文件（Linux 无桌面密钥环时）
    secretDegraded() {
      return this.secretBackend === 'file'
    },
    // 颜色模式图标与提示文案（自动 / 浅色 / 深色 循环）
    themeIcon() {
      return this.themeMode === 'light' ? '☀️' : this.themeMode === 'dark' ? '🌙' : '🌗'
    },
    themeLabel() {
      const key = this.themeMode === 'light' ? 'theme.light' : this.themeMode === 'dark' ? 'theme.dark' : 'theme.auto'
      return this.$t(key)
    }
  },
  watch: {
    msg(v) { if (v) this.autoClearToast('msg') },
    error(v) { if (v) this.autoClearToast('error') }
  },
  async mounted() {
    this.setupIdleLock()
    this.applyTheme()
    // 关于页展示客户端版本（读 tauri.conf.json 的 version）。
    getVersion().then((v) => { this.clientVersion = v }).catch(() => {})
    // 从托盘恢复窗口时，同步锁定状态（隐藏到托盘时后端已清除主密钥）
    await listen('window-shown', async () => {
      try {
        const unlocked = await api.IsUnlocked()
        if (!unlocked) {
          this.vaultUnlocked = false
          this.entries = []
          this.vaultPassword = ''
        }
      } catch (e) {
        /* ignore */
      }
    })
    try {
      this.vaultInitialized = await api.Init()
      const cfg = await api.GetServerConfig()
      try {
        const s = await api.GetSettings()
        this.recycleEnabled = s.recycle !== '0'
      } catch (e) {
        /* 忽略，保持默认走回收站 */
      }
      // 会话状态由 Rust 侧给出（令牌经系统凭据库持久化，前端不接触）。
      const sess = await api.SessionInfo()
      this.server = localStorage.getItem('syncServer') || cfg.server || sess.server || ''
      this.serverUsername = localStorage.getItem('syncUsername') || cfg.username || sess.username || ''
      this.loggedIn = sess.loggedIn === '1'
      // 清理历史版本可能遗留的明文口令与令牌：同步口令是主密钥派生源，绝不落盘。
      localStorage.removeItem('syncPassword')
      localStorage.removeItem('serverToken')
      this.rememberMe = !!localStorage.getItem('syncUsername')
      if (this.loggedIn && this.server) {
        try {
          await api.SyncCheck(this.server)
          await this.loadAvatar()
        } catch (e) {
          // 证书信任问题不代表会话失效（用户确认信任后即可恢复）；
          // 其余错误视为会话已失效，清除本地会话状态。
          const msg = String(e)
          if (!/NEED_TRUST|FINGERPRINT_CHANGED/.test(msg)) {
            await api.ClearSession()
            this.loggedIn = false
            localStorage.removeItem('myAvatar')
          }
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
    // 统一把错误转成用户可读文案：
    //   · LOCAL_LOCKED → 引导先解锁本地密码库（破坏性同步命令的前置要求）
    //   · NEED_TRUST / FINGERPRINT_CHANGED → 服务器证书信任问题（正常会被弹窗拦截）
    //   · 其余错误已由 Rust 侧翻译为可读提示，直接展示
    errText(e) {
      const msg = String(e)
      if (msg.includes('LOCAL_LOCKED')) return this.$t('sync.needUnlock')
      if (msg.includes('NOT_LOGGED_IN')) return this.$t('sync.notLoggedIn')
      // 用户在原生确认框中拒绝连接（地址白名单 / 证书指纹）。
      if (msg.includes('TRUST_DENIED')) return this.$t('sync.trustDenied')
      if (msg.includes('FINGERPRINT_CHANGED')) return this.$t('sync.fpChanged')
      return msg
    },
    // 加载已确认的服务器与已信任的指纹（展示用）。
    async loadTrustInfo() {
      try {
        this.allowedServers = (await api.ListAllowedServers()) || []
      } catch (e) {
        this.allowedServers = []
      }
      try {
        this.trustedFingerprint = this.server ? await api.GetTrustedFingerprint(this.server) : ''
      } catch (e) {
        this.trustedFingerprint = ''
      }
    },
    // 撤销某服务器的确认：下次连接会重新弹出原生确认框。
    async removeAllowed(host) {
      this.error = ''
      try {
        await api.RemoveAllowedServer(host)
        await this.loadTrustInfo()
      } catch (e) {
        this.error = this.errText(e)
      }
    },
    // 选择局域网发现结果（https 项优先展示）
    pickLan(s) {
      this.server = s.url
    },
    // 把当前明文地址改为 https（服务端同端口双协议，通常可直接切换）
    useHttps() {
      this.server = (this.server || '').replace(/^http:\/\//i, 'https://')
    },
    changeLang() {
      this.$i18n.locale = this.lang
      localStorage.setItem('locale', this.lang)
    },
    // ---- 边栏页签与颜色模式 ----
    switchTab(t) {
      this.tab = t
      if (t === 'trash') this.loadTrash()
      if (t === 'logs') this.loadLogs()
      if (t === 'settings') this.loadSettings()
    },
    // 颜色模式循环切换：自动（跟随系统）→ 浅色 → 深色 → 自动。
    cycleTheme() {
      const order = ['auto', 'light', 'dark']
      const next = order[(order.indexOf(this.themeMode) + 1) % order.length]
      this.themeMode = next
      try {
        localStorage.setItem('theme', next)
      } catch (e) {
        /* ignore */
      }
      this.applyTheme()
    },
    applyTheme() {
      let resolved = this.themeMode
      if (resolved === 'auto') {
        resolved = window.matchMedia && window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light'
      }
      document.documentElement.setAttribute('data-theme', resolved)
    },
    async loadLogs() {
      this.error = ''
      try {
        this.logs = (await api.ReadLogs(500)) || []
      } catch (e) {
        this.error = String(e)
      }
    },
    doClearLogs() {
      this.askConfirm(this.$t('logs.confirmClear'), async () => {
        try {
          await api.ClearLogs()
          this.logs = []
        } catch (e) {
          this.error = String(e)
        }
      })
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
          this.maybeAutoSync()
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
    setupIdleLock() {
      const events = ['mousemove', 'mousedown', 'keydown', 'wheel', 'touchstart', 'scroll']
      const reset = () => {
        if (!this.vaultUnlocked) return
        clearTimeout(this.idleTimer)
        this.idleTimer = setTimeout(() => { this.lock() }, this.idleTimeout)
      }
      events.forEach((ev) => window.addEventListener(ev, reset, { passive: true }))
      reset()
      // 窗口失焦 / 页面隐藏（切换应用、最小化、锁屏）时立即锁定，避免密钥长期驻留内存。
      window.addEventListener('blur', () => {
        if (this.vaultUnlocked) this.lock()
      })
      document.addEventListener('visibilitychange', () => {
        if (document.hidden && this.vaultUnlocked) this.lock()
      })
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
      const soft = this.recycleEnabled
      this.askConfirm(this.$t('msg.confirmDelete', { title: e.title }), async () => {
        try {
          await api.DeleteEntry(e.id, soft)
          this.msg = soft ? this.$t('msg.deletedToTrash') : this.$t('msg.deleted')
          await this.loadEntries()
          await this.autoSync()
        } catch (err) {
          this.error = String(err)
        }
      })
    },
    async autoSync() {
      if (!this.loggedIn) return
      try {
        await api.PushVault(this.server)
      } catch (e) {
        /* 自动同步失败不阻断本地操作 */
      }
    },
    // 显示/隐藏切换：按「条目 id + 字段」记录，可在本条目的各字段之间独立开关。
    toggleReveal(id, field) {
      const k = id + ':' + field
      if (this.revealed.has(k)) this.revealed.delete(k)
      else this.revealed.add(k)
    },
    async togglePin(e) {
      try {
        await api.PinEntry(e.id, !e.pinned)
        await this.loadEntries()
        this.msg = e.pinned ? this.$t('msg.unpinned') : this.$t('msg.pinned')
      } catch (err) {
        this.error = String(err)
      }
    },
    onDragStart(e) {
      if (!this.canDrag) return
      this.dragId = e.id
    },
    onDragOver(e) {
      if (this.dragId != null && this.dragId !== e.id) this.dragOverId = e.id
    },
    onDragLeave(e) {
      if (this.dragOverId === e.id) this.dragOverId = null
    },
    onDragEnd() {
      this.dragId = null
      this.dragOverId = null
    },
    async onDrop(target) {
      const from = this.dragId
      this.dragId = null
      this.dragOverId = null
      if (from == null || from === target.id) return
      const list = this.entries.slice()
      const fi = list.findIndex((x) => x.id === from)
      const ti = list.findIndex((x) => x.id === target.id)
      if (fi < 0 || ti < 0) return
      // 置顶与普通条目分属两组，不允许互相拖入（否则会被排序规则弹回原位）。
      if (list[fi].pinned !== list[ti].pinned) {
        this.msg = this.$t('msg.pinGroupOnly')
        return
      }
      const [moved] = list.splice(fi, 1)
      list.splice(ti, 0, moved)
      this.entries = list
      try {
        await api.ReorderEntries(list.map((x) => x.id))
        await this.loadEntries()
        // 顺序属于同步内容：落库后推送一次，使其他设备保持一致。
        await this.autoSync()
      } catch (e) {
        this.error = String(e)
        await this.loadEntries()
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
    onIO() {
      const v = this.ioFormat
      if (v === 'import-csv') this.importCsv()
      else if (v === 'import-txt') this.importTxt()
      else if (v === 'export-csv') this.exportCsv()
      else if (v === 'export-txt') this.exportTxt()
      this.ioFormat = ''
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
        // 附带一条示例数据，导入后可直接看到各列含义（密码建议导入前自行替换）。
        const example = ['示例网站', 'myuser', 'MyPass123', 'https://example.com', '常用', '这是一条示例备注，可删除']
        const csv = '\ufeff' + header + '\r\n' + example.map(csvEscape).join(',') + '\r\n'
        const path = await api.SaveTextFile(csv, this.$t('app.title') + ' - ' + this.$t('file.template') + '.csv')
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
      this.serverPassword = ''
      this.rememberMe = !!localStorage.getItem('syncUsername')
      this.serverLoginOpen = true
      this.checkSecretBackend()
      this.loadTrustInfo()
    },
    // 检测同步密钥的存储后端，降级时提示用户（Linux 无桌面密钥环时）。
    async checkSecretBackend() {
      try {
        this.secretBackend = await api.GetSecretBackend()
      } catch (e) {
        this.secretBackend = ''
      }
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
    async loadAvatar() {
      // 头像端点要求登录：由 Rust 侧带 JWT（并校验证书指纹）拉取后转 data URL。
      // 拉取失败仅回退为字母头像，不打扰用户。
      this.avatarDataUrl = ''
      if (!this.myAvatar || !this.server || !this.loggedIn) return
      const id = parseInt(this.myAvatar.replace(/\.[^.]+$/, ''), 10)
      if (!id) return
      try {
        this.avatarDataUrl = await api.FetchAvatar(this.server, id)
      } catch (e) {
        /* ignore */
      }
    },
    async doLogout() {
      // 令牌保存在 Rust 侧 + 系统凭据库，退出需通知后端清除（前端无令牌可清）。
      try {
        await api.ClearSession()
      } catch (e) {
        /* 清理失败不阻断退出流程 */
      }
      this.loggedIn = false
      this.serverPassword = ''
      this.myAvatar = ''
      this.avatarDataUrl = ''
      this.userMenuOpen = false
      localStorage.removeItem('serverToken')
      localStorage.removeItem('myAvatar')
      localStorage.removeItem('syncPassword')
      localStorage.removeItem('syncServer')
      localStorage.removeItem('syncUsername')
      this.msg = this.$t('sync.loggedOut')
    },
    async auth(isRegister) {
      this.error = ''
      try {
        const r = isRegister
          ? await api.SyncRegister(this.server, this.serverUsername, this.serverPassword, this.serverEmail, this.serverCode)
          : await api.SyncLogin(this.server, this.serverUsername, this.serverPassword)
        // 令牌已由 Rust 侧保存（内存 + 系统凭据库），前端只置登录态；
        // 服务端不再把原始 JWT 下发给 WebView（PT-06）。
        this.loggedIn = true
        this.myAvatar = r.avatar || ''
        await this.loadAvatar()
        // 同步口令不持久化：它同时是端到端加密主密钥的派生源，明文落盘等于交出密码库。
        // 「记住我」仅用于记忆同步服务器地址与账号名。
        localStorage.removeItem('syncPassword')
        localStorage.removeItem('serverToken')
        if (this.rememberMe) {
          localStorage.setItem('syncServer', this.server)
          localStorage.setItem('syncUsername', this.serverUsername)
        } else {
          localStorage.removeItem('syncServer')
          localStorage.removeItem('syncUsername')
        }
        this.msg = isRegister ? this.$t('msg.registerSuccess') : this.$t('msg.loginSuccess')
        this.serverLoginOpen = false
        this.vaultRecovery = false
        this.oldPassword = ''
        this.serverPassword = ''
        this.serverEmail = ''
        this.serverCode = ''
      } catch (e) {
        const msg = String(e)
        if (msg.includes('VAULT_KEY_MISMATCH')) {
          // 账号密码曾被重置：旧 vault key 无法用新密码解开，展示恢复入口。
          this.vaultRecovery = true
          this.error = this.$t('sync.recoverPrompt')
        } else {
          this.error = this.errText(e)
        }
      }
    },
    // 用旧密码恢复原密码库（数据无损）：本地解开旧 vault key 后用新密码重新包裹上传。
    async recoverVault() {
      this.error = ''
      try {
        await api.RecoverVault(this.server, this.serverUsername, this.serverPassword, this.oldPassword)
        this.vaultRecovery = false
        this.oldPassword = ''
        // 本地 vault key 已重新持久化，重新登录完成同步状态建立。
        await this.auth(false)
      } catch (e) {
        const msg = String(e)
        this.error = msg.includes('OLD_PASSWORD_WRONG') ? this.$t('sync.recoverFailed') : this.errText(e)
      }
    },
    // 放弃旧密码库（不可恢复）：清空服务端旧密文与本地缓存，之后自动生成新 vault key。
    async resetVaultRemote() {
      this.error = ''
      try {
        await api.ResetVaultRemote(this.server, this.serverUsername, this.serverPassword)
        this.vaultRecovery = false
        this.oldPassword = ''
        await this.auth(false)
      } catch (e) {
        this.error = this.errText(e)
      }
    },
    async sendRegCode() {
      this.error = ''
      try {
        await api.SendRegisterCode(this.server, this.serverEmail)
        this.msg = this.$t('sync.codeSent')
      } catch (e) {
        this.error = this.errText(e)
      }
    },
    async doPush() {
      this.error = ''
      try {
        await api.PushVault(this.server)
        this.msg = this.$t('msg.uploaded')
      } catch (e) {
        this.error = this.errText(e)
      }
    },
    async doPull() {
      this.error = ''
      try {
        const n = await api.PullVault(this.server)
        this.msg = this.$t('msg.downloaded', { n })
        await this.loadEntries()
      } catch (e) {
        this.error = this.errText(e)
      }
    },
    async loadSettings() {
      try {
        const s = await api.GetSettings()
        this.settingsForm = {
          autostart: s.autostart === '1',
          autosync: s.autosync === '1',
          priority: ['local', 'server', 'merge'].includes(s.priority) ? s.priority : 'local',
          recycle: s.recycle !== '0',
          recycleDays: parseInt(s.recycleDays, 10) || 30
        }
      } catch (e) {
        this.settingsForm = { autostart: false, autosync: false, priority: 'local', recycle: true, recycleDays: 30 }
      }
      // 置顶同步为账号级设置（保存在服务端），仅在已登录时可读改。
      this.pinSync = false
      if (this.loggedIn && this.server) {
        try {
          this.pinSync = await api.GetPinSync(this.server)
        } catch (e) {
          /* 读取失败按关闭展示，用户改动时会再次尝试 */
        }
      }
    },
    async changePinSync() {
      try {
        await api.SetPinSync(this.server, this.pinSync)
        this.msg = this.$t('settings.saved')
        this.autoClearToast('msg')
      } catch (e) {
        this.error = this.errText(e)
        // 失败时回读真实状态，避免勾选框与服务端不一致。
        try {
          this.pinSync = await api.GetPinSync(this.server)
        } catch (e2) {
          /* ignore */
        }
      }
    },
    async saveSettings() {
      try {
        await api.SaveSettings(
          this.settingsForm.autostart,
          this.settingsForm.autosync,
          this.settingsForm.priority,
          this.settingsForm.recycle,
          this.settingsForm.recycleDays
        )
        this.recycleEnabled = this.settingsForm.recycle
        this.msg = this.$t('settings.saved')
        this.autoClearToast('msg')
      } catch (e) {
        this.error = String(e)
      }
    },
    askConfirm(text, onOk) {
      this.confirmBox = { text, onOk }
    },
    async loadTrash() {
      this.error = ''
      try {
        this.trash = (await api.ListTrash()) || []
      } catch (e) {
        this.error = String(e)
      }
    },
    async doRestore(id) {
      try {
        await api.RestoreEntry(id)
        this.msg = this.$t('trash.restored')
        this.trash = (await api.ListTrash()) || []
        await this.loadEntries()
        await this.autoSync()
      } catch (e) {
        this.error = String(e)
      }
    },
    async doPurge(id) {
      try {
        await api.PurgeEntry(id)
        this.msg = this.$t('trash.purged')
        this.trash = (await api.ListTrash()) || []
      } catch (e) {
        this.error = String(e)
      }
    },
    async doEmptyTrash() {
      this.askConfirm(this.$t('trash.confirmEmpty'), async () => {
        try {
          const n = await api.EmptyTrash()
          this.msg = this.$t('trash.emptied', { n })
          this.trash = []
        } catch (e) {
          this.error = String(e)
        }
      })
    },
    async maybeAutoSync() {
      try {
        const s = await api.GetSettings()
        if (s.autosync !== '1' || !this.loggedIn || !this.server) return
        let n = 0
        if (s.priority === 'server') {
          n = await api.PullVault(this.server)
          await this.loadEntries()
          this.msg = this.$t('settings.autoPulled', { n })
        } else if (s.priority === 'merge') {
          n = await api.MergeVault(this.server)
          await this.loadEntries()
          this.msg = this.$t('settings.autoMerged', { n })
        } else {
          await api.PushVault(this.server)
          this.msg = this.$t('settings.autoPushed')
        }
        this.autoClearToast('msg')
      } catch (e) {
        // 自动同步失败不打断使用，仅在控制台可见
        console.warn('auto sync failed:', e)
      }
    }
  }
}
</script>

<style scoped>
/* 禁用态按钮（如回收站为空时的「清空回收站」）：灰显且不可点击 */
button:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}
.gate {
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
}
.gate-card {
  width: 340px;
  background: var(--card);
  border: 1px solid var(--border);
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
  color: var(--muted);
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
  background: var(--chip);
  color: var(--text);
  border: 1px solid var(--input-border);
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
  color: var(--muted);
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
  color: var(--muted);
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
.grid-list {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(360px, 1fr));
  gap: 8px;
  align-items: start;
}
.entry {
  display: flex;
  flex-direction: column;
  gap: 8px;
  background: var(--card);
  padding: 12px 16px;
  border-radius: 8px;
  border: 1px solid var(--border);
  transition: border-color 0.15s, background 0.15s;
}
/* 仅当可拖拽时给出抓取光标（搜索状态下不启用拖拽） */
.entry[draggable='true'] {
  cursor: grab;
}
.entry.entry-dragging {
  opacity: 0.45;
}
.entry.entry-drop-target {
  border-color: var(--fnos-primary);
  background: #f5f9ff;
}
.entry-pinned {
  border-left: 3px solid var(--fnos-primary);
}
.entry-head .title {
  display: flex;
  align-items: center;
  gap: 4px;
  min-width: 0;
}
.pin-mark {
  display: inline-flex;
  align-items: center;
  color: var(--fnos-primary);
  flex-shrink: 0;
}
.pin-btn.pin-on {
  color: var(--fnos-primary);
  border-color: var(--fnos-primary);
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
  color: var(--muted);
  font-size: 12px;
}
.meta-line {
  display: flex;
  align-items: center;
  gap: 6px;
  flex-wrap: wrap;
}
.label {
  color: var(--faint);
  flex-shrink: 0;
}
.item {
  display: flex;
  align-items: center;
  gap: 16px;
  background: var(--card);
  border: 1px solid var(--border);
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
  color: var(--muted);
  font-size: 12px;
  margin-top: 4px;
  align-items: center;
  flex-wrap: wrap;
}
.tag {
  background: var(--chip);
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
  background: var(--chip);
  color: var(--text);
  padding: 4px 8px;
  font-size: 12px;
}
.empty {
  text-align: center;
  color: var(--muted);
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
  background: var(--card);
  border: 1px solid var(--input-border);
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
  color: var(--muted);
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
  border: 1px solid var(--input-border);
  padding: 4px 10px;
  border-radius: 6px;
}
.user-trigger:hover {
  opacity: 1;
  background: var(--chip);
}
.caret {
  color: var(--muted);
  font-size: 12px;
}
.user-dropdown {
  position: absolute;
  top: calc(100% + 6px);
  right: 0;
  min-width: 200px;
  background: var(--card);
  border: 1px solid var(--border);
  border-radius: 8px;
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.12);
  z-index: 5001;
  overflow: hidden;
}
.dd-server {
  padding: 8px 12px;
  font-size: 12px;
  color: var(--muted);
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
.recovery-hint {
  margin: 6px 0 0;
  padding: 8px 10px;
  background: #fffbeb;
  border: 1px solid #fde68a;
  border-radius: 8px;
  font-size: 12px;
  color: #92400e;
  line-height: 1.6;
}
.settings-group {
  margin-top: 10px;
  padding: 8px 10px;
  background: #f9fafb;
  border: 1px solid var(--border);
  border-radius: 8px;
}
.settings-label {
  font-size: 13px;
  color: #374151;
  margin-bottom: 4px;
}
.radio-row {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-top: 4px;
  cursor: pointer;
  color: #374151;
  font-size: 13px;
}
.radio-row input,
.checkbox-row input {
  width: auto;
  cursor: pointer;
}
.settings-hint {
  margin-top: 8px;
  font-size: 12px;
  color: var(--faint);
  line-height: 1.5;
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
  color: var(--muted);
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
  display: flex;
  align-items: center;
  gap: 8px;
  text-align: left;
  background: var(--chip);
  color: var(--text);
  padding: 6px 10px;
  border-radius: 6px;
  font-size: 13px;
}
.lan-item:hover {
  background: var(--fnos-primary-light);
  opacity: 1;
}
.lan-badge {
  flex-shrink: 0;
  font-size: 11px;
  padding: 1px 6px;
  border-radius: 4px;
  font-weight: 600;
}
.lan-secure {
  background: #dcfce7;
  color: #166534;
}
.lan-plain {
  background: #fee2e2;
  color: #991b1b;
}
.lan-url {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.plaintext-warn {
  margin: 6px 0 0;
  padding: 8px 10px;
  background: #fef2f2;
  border: 1px solid #fecaca;
  border-radius: 6px;
  color: #991b1b;
  font-size: 12px;
  line-height: 1.5;
}
.link-btn {
  background: none;
  border: none;
  color: var(--fnos-primary, #2563eb);
  cursor: pointer;
  font-size: 12px;
  padding: 0;
  text-decoration: underline;
}
.inline-fp {
  margin-top: 6px;
}
.allowed-list {
  margin-top: 6px;
  border: 1px solid var(--border);
  border-radius: 6px;
  padding: 6px 10px;
}
.allowed-title {
  color: var(--muted);
  font-size: 12px;
  margin-bottom: 4px;
}
.allowed-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  font-size: 12px;
  padding: 2px 0;
}
.allowed-host {
  font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
  color: #374151;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.fp-box {
  background: var(--hover);
  border: 1px solid var(--border);
  border-radius: 6px;
  padding: 10px;
}
.fp-label {
  color: var(--muted);
  font-size: 12px;
  margin-bottom: 4px;
}
.fp-value {
  font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
  font-size: 12px;
  color: var(--text);
  word-break: break-all;
  line-height: 1.5;
}
.secret-warn {
  margin: 6px 0 0;
  color: #92400e;
  background: #fffbeb;
  border: 1px solid #fde68a;
  border-radius: 6px;
  padding: 6px 10px;
  font-size: 12px;
}
.form-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  margin-top: 8px;
  color: #374151;
  font-size: 13px;
}
.form-row input {
  width: 90px;
  text-align: right;
}
.trash-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
  max-height: 320px;
  overflow-y: auto;
  margin: 8px 0;
}
.trash-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  background: var(--card);
  border: 1px solid var(--border);
  padding: 10px 12px;
  border-radius: 8px;
}
.trash-main {
  min-width: 0;
}
.modal-confirm {
  width: 380px;
}
.confirm-text {
  color: #374151;
  font-size: 14px;
  line-height: 1.5;
  word-break: break-all;
}

/* ---- 左侧边栏布局（对齐服务端后台：152px 宽、页签 + 主题切换/GitHub 页脚） ---- */
.layout {
  flex: 1;
  min-height: 0;
  display: flex;
  gap: 16px;
}
.sidebar {
  width: 152px;
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 8px 0;
  border-right: 1px solid var(--border);
}
.sidebar button {
  background: transparent;
  color: var(--muted);
  border-radius: 6px;
  padding: 10px 14px;
  text-align: left;
  width: 100%;
}
.sidebar button:hover {
  background: var(--hover);
}
.sidebar button.active {
  color: var(--fnos-primary);
  background: var(--fnos-primary-light);
  font-weight: 600;
}
.sidebar-footer {
  margin-top: auto;
  padding-top: 8px;
  border-top: 1px solid var(--border);
  display: flex;
  align-items: center;
  gap: 16px;
  padding-left: 12px;
}
/* 用 .sidebar button.theme-toggle 提高优先级，避免被 .sidebar button 的 width:100% 覆盖 */
.sidebar button.theme-toggle {
  width: 18px;
  height: 36px;
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  background: transparent;
  color: var(--muted);
  border-radius: 4px;
  padding: 0;
  font-size: 16px;
  line-height: 1;
}
.sidebar button.theme-toggle:hover {
  background: var(--hover);
  opacity: 1;
}
.theme-icon {
  font-size: 16px;
}
.gh-link {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 18px;
  height: 36px;
  border-radius: 4px;
  color: var(--muted);
  flex-shrink: 0;
}
.gh-link:hover {
  background: var(--hover);
  color: var(--text);
}
.content {
  flex: 1;
  min-width: 0;
  min-height: 0;
  display: flex;
  flex-direction: column;
}
/* 非密码页的通用面板：独立滚动，标题行与操作按钮 */
.panel {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 12px;
}
.panel-title {
  font-size: 18px;
}
.panel-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}
.panel-ops {
  display: flex;
  gap: 8px;
}
.panel-hint {
  color: var(--muted);
  font-size: 12px;
}
.about-line {
  color: var(--muted);
  font-size: 13px;
}
.about-desc {
  line-height: 1.7;
}
.about-link {
  color: var(--fnos-primary);
  text-decoration: none;
  word-break: break-all;
}
.about-link:hover {
  text-decoration: underline;
}
/* 本机日志：等宽字体逐行展示（新在前） */
.log-list {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.log-line {
  background: var(--card);
  border: 1px solid var(--border);
  border-radius: 6px;
  padding: 6px 10px;
}
.log-line code {
  font-family: 'Consolas', monospace;
  font-size: 12px;
  color: var(--text);
  word-break: break-all;
  white-space: pre-wrap;
}

/* ---- 深色模式补丁：覆盖未变量化的零散颜色 ---- */
html[data-theme='dark'] .gate-card {
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.5);
}
html[data-theme='dark'] .entry-drop-target {
  background: var(--fnos-primary-light);
}
html[data-theme='dark'] .confirm-text {
  color: var(--text);
}
html[data-theme='dark'] .toast-msg {
  background: rgba(5, 150, 105, 0.95);
}
html[data-theme='dark'] .lang-select {
  background: var(--chip);
  color: var(--text);
  border-color: var(--input-border);
}
</style>
