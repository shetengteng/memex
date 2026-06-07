import { ref, watch } from 'vue'

export type ThemeMode = 'light' | 'dark' | 'system'

const STORAGE_KEY = 'memex-theme'

const stored = (typeof localStorage !== 'undefined' ? localStorage.getItem(STORAGE_KEY) : null) as
  | ThemeMode
  | null

export const themeMode = ref<ThemeMode>(stored ?? 'system')

const mql = typeof window !== 'undefined' ? window.matchMedia('(prefers-color-scheme: dark)') : null

function apply(mode: ThemeMode) {
  if (typeof document === 'undefined') return
  const useDark = mode === 'dark' || (mode === 'system' && !!mql?.matches)
  document.documentElement.classList.toggle('dark', useDark)
}

watch(themeMode, (v) => {
  if (typeof localStorage !== 'undefined') localStorage.setItem(STORAGE_KEY, v)
  apply(v)
})

if (mql) {
  mql.addEventListener('change', () => {
    if (themeMode.value === 'system') apply('system')
  })
}

export function initTheme() {
  apply(themeMode.value)
}
