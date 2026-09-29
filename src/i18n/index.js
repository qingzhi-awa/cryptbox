import { createI18n } from 'vue-i18n'
import zhCN from './zh-CN'
import zhTW from './zh-TW'
import en from './en'
import ja from './ja'
import ko from './ko'
import fr from './fr'
import de from './de'
import es from './es'
import ru from './ru'
import pt from './pt'

const saved = localStorage.getItem('locale') || 'zh-CN'

const i18n = createI18n({
  legacy: true,
  locale: saved,
  fallbackLocale: 'zh-CN',
  messages: {
    'zh-CN': zhCN,
    'zh-TW': zhTW,
    en: en,
    ja: ja,
    ko: ko,
    fr: fr,
    de: de,
    es: es,
    ru: ru,
    pt: pt
  }
})

export default i18n
