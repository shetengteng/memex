import { ref, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'

/**
 * Surface（窗口表面质感）= 「实色」or「毛玻璃」。和颜色基调（light/dark/system）
 * 是正交维度：用户既可以选 Dark + Glass 也可以 Light + Glass。
 *
 * 真值在后端 KV (`ui.surface`)。前端 ref 只是一份本地缓存，启动时通过
 * `initSurface()` 拉一次后端值；用户切换时调 `set_window_surface` IPC，
 * 后端立刻调 `apply_vibrancy`/`clear_vibrancy` + 写 KV。
 *
 * CSS 端：在 `<html>` 上挂 `surface-glass` class，所有需要让 vibrancy 透出
 * 的容器（body / sidebar / 卡片）按这个 class 切到半透明 + backdrop-blur。
 */
export type SurfaceMode = 'solid' | 'glass'

const STORAGE_KEY = 'memex-surface'

const initial: SurfaceMode =
  (typeof localStorage !== 'undefined'
    ? (localStorage.getItem(STORAGE_KEY) as SurfaceMode | null)
    : null) ?? 'solid'

export const surfaceMode = ref<SurfaceMode>(initial)

function applyClass(mode: SurfaceMode): void {
  if (typeof document === 'undefined') return
  document.documentElement.classList.toggle('surface-glass', mode === 'glass')
}

watch(surfaceMode, (v) => {
  if (typeof localStorage !== 'undefined') localStorage.setItem(STORAGE_KEY, v)
  applyClass(v)
  void invoke('set_window_surface', { mode: v }).catch(() => {})
})

/**
 * 启动时调一次：先按本地 cache 立刻把 class 挂上（避免首屏闪烁），
 * 再异步拉后端权威值。后端值与本地不一致时以后端为准（这是为了在
 * 多窗口 / kv 被外部改动场景下保持一致）。
 */
export async function initSurface(): Promise<void> {
  applyClass(surfaceMode.value)
  try {
    const remote = (await invoke<string>('get_window_surface')) as SurfaceMode
    if (remote && remote !== surfaceMode.value) {
      surfaceMode.value = remote
    }
  } catch {}
}
