import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import tailwindcss from '@tailwindcss/vite'
import { resolve } from 'path'
import { readFileSync } from 'fs'

const host = process.env.TAURI_DEV_HOST

const pkg = JSON.parse(readFileSync(resolve(__dirname, 'package.json'), 'utf8')) as { version: string }

export default defineConfig({
  /*
   * base: './' 让 dist/index.html 生成相对路径 (`./assets/...`)，而不是绝对路径 (`/assets/...`)。
   * Tauri 2 在 macOS 上用自定义协议 `tauri://localhost` 加载 production dist；
   * macOS 26 (Tahoe) 起 WebKit 对自定义协议 + 绝对路径的解析变严格，绝对路径
   * 会被判为"协议外资源"导致 `code=-1004 NSURLErrorCannotConnectToHost`，
   * 主 frame navigation 直接停在 `about:blank`，整窗空白且无任何 console 错误线索。
   * 相对路径不触发协议外解析，所有版本都安全。
   * 参考：tauri-apps/tauri#13262；DEV "Fixing the Tauri v2 white screen in production"。
   */
  base: './',
  plugins: [vue(), tailwindcss()],
  resolve: {
    alias: {
      '@': resolve(__dirname, 'src'),
    },
  },
  define: {
    __APP_VERSION__: JSON.stringify(pkg.version),
  },
  clearScreen: false,
  server: {
    port: 1520,
    strictPort: true,
    host: host || false,
    hmr: host ? { protocol: 'ws', host, port: 1521 } : undefined,
    watch: { ignored: ['**/src-tauri/**'] },
  },
})
