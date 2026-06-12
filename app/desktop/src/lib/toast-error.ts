import { toast } from 'vue-sonner'

import { humanizeBackendError, type FriendlyBackendError } from './utils'

/**
 * Toast a structured backend error in user-readable form.
 *
 * Tauri IPC rejects come back as plain objects (`{ kind, message }`), so naive
 * `String(e)` interpolation stringifies them as "[object Object]". This helper
 * routes the error through `humanizeBackendError` (kind-aware + known-message
 * regex matching) and emits a Sonner toast with the friendly text as the
 * description, then returns the parsed result so the caller can chain an
 * action button (e.g. "去设置") when needed.
 */
export function toastBackendError(
  title: string,
  e: unknown,
  duration = 8000,
): FriendlyBackendError {
  const fe = humanizeBackendError(e)
  toast.error(title, { description: fe.friendly, duration })
  return fe
}
