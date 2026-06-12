// 轻量 i18n：单一全局 reactive locale + `{var}` 风格的插值。
// 故意不引入 vue-i18n，避免给 menubar 主进程多挂一个运行时依赖。

import { ref, computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { MESSAGES, DEFAULT_LOCALE, type Locale } from './messages'

export type { Locale } from './messages'
export { LOCALE_OPTIONS } from './messages'

const STORAGE_KEY = 'memex.ui.locale'
const CONFIG_KEY = 'ui.locale'

// 全应用共享一份 locale，所有 useI18n() 实例都看到同一个值。
const current = ref<Locale>(readInitialLocale())

function readInitialLocale(): Locale {
  if (typeof window !== 'undefined') {
    try {
      const v = window.localStorage?.getItem(STORAGE_KEY)
      if (v === 'zh' || v === 'en') return v
    } catch {
      // 沙箱里 localStorage 不可用就走默认值
    }
  }
  return DEFAULT_LOCALE
}

function interpolate(tpl: string, vars?: Record<string, string | number>): string {
  if (!vars) return tpl
  return tpl.replace(/\{(\w+)\}/g, (_, k) => {
    const v = vars[k]
    return v === undefined || v === null ? '' : String(v)
  })
}

export function useI18n() {
  const locale = current
  const t = (key: string, vars?: Record<string, string | number>): string => {
    const dict = MESSAGES[locale.value] || MESSAGES[DEFAULT_LOCALE]
    const tpl = dict[key]
    if (tpl === undefined) {
      // 缺 key 时退到默认 locale，再退到 key 本身，便于开发期发现遗漏
      const fallback = MESSAGES[DEFAULT_LOCALE][key]
      return fallback ? interpolate(fallback, vars) : key
    }
    return interpolate(tpl, vars)
  }
  const tHtml = computed(() => (key: string, vars?: Record<string, string | number>) => t(key, vars))
  return { locale, t, tHtml }
}

// 给已经 mount 的窗口一个机会在 setLocale 之后立刻重渲染（reactive 自动触发）。
export async function setLocale(next: Locale): Promise<void> {
  current.value = next
  try {
    window.localStorage?.setItem(STORAGE_KEY, next)
  } catch {
    /* ignore */
  }
  try {
    await invoke('set_config', { key: CONFIG_KEY, value: next })
  } catch {
    /* 后台落库失败也不影响 UI 切换 */
  }
}

// 启动时从后台 kv 读取 locale，跟 localStorage 一致就跳过。
// 这样不同窗口下次启动都能看到统一的语言。
export async function syncLocaleFromBackend(): Promise<void> {
  try {
    const val = await invoke<string | null>('get_config', { key: CONFIG_KEY })
    if (val === 'zh' || val === 'en') {
      current.value = val
      try {
        window.localStorage?.setItem(STORAGE_KEY, val)
      } catch {
        /* ignore */
      }
    }
  } catch {
    /* ignore */
  }
}
