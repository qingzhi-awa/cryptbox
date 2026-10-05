import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'

export default defineConfig({
  // __INTLIFY_JIT_COMPILATION__ = true（R9-06）：让 vue-i18n 使用 AST 解释器而非
  // new Function 编译路径，从而可以移除 CSP 中的 'unsafe-eval'。
  define: {
    __INTLIFY_JIT_COMPILATION__: true
  },
  plugins: [vue()],
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true
  }
})
