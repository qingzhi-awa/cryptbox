import { createApp } from 'vue'
import App from './App.vue'
import i18n from './i18n'
import './style.css'

// 捕获脚本加载失败与未处理异常
const diag = document.getElementById('diag')
function addDiag(msg) {
  if (diag) diag.textContent += '\n' + msg
}
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

const app = createApp(App)
app.use(i18n)
app.config.errorHandler = (err) => {
  addDiag('VUE-ERR: ' + (err && err.message ? err.message : String(err)))
}
app.mount('#app')
