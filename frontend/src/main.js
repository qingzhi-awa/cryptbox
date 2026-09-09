import { createApp } from 'vue'
import App from './App.vue'
import i18n from './i18n'
import './style.css'

// —— 调试诊断框：仅在通过 `passbook.exe --debug` 启动时显示 ——
const diag = document.getElementById('diag')
function addDiag(msg) {
  if (diag) diag.textContent += '\n' + msg
}
// 捕获脚本加载失败（模块脚本 404/跨域/MIME）与未处理异常
window.addEventListener(
  'error',
  (e) => {
    const t = e.target
    const src = t && (t.src || t.href) ? t.src || t.href : ''
    addDiag('CAP-ERR: ' + (e.message || e.type) + (src ? ' src=' + src : ''))
  },
  true
)
window.addEventListener('unhandledrejection', (e) => {
  addDiag('REJECT: ' + (e.reason && e.reason.message ? e.reason.message : e.reason))
})
;(async () => {
  try {
    const isDebug = await window.go.main.App.IsDebug()
    if (isDebug && diag) {
      diag.style.display = 'block'
      diag.textContent = 'DEBUG 模式'
    }
  } catch (e) {
    /* 忽略 */
  }
})()

const app = createApp(App)
app.use(i18n)
app.config.errorHandler = (err) => {
  addDiag('VUE-ERR: ' + (err && err.message ? err.message : String(err)))
}
app.mount('#app')
