import { reactive } from 'vue'

export const scanState = reactive({
  scanning: false,
  msgs: 0,
})

let scanStartTime = 0
let delayTimer: ReturnType<typeof setTimeout> | null = null
const MIN_DISPLAY_MS = 2000

export function startScanning() {
  if (delayTimer) { clearTimeout(delayTimer); delayTimer = null }
  scanState.scanning = true
  scanState.msgs = 0
  scanStartTime = Date.now()
}

export function stopScanning() {
  const elapsed = Date.now() - scanStartTime
  const remaining = Math.max(0, MIN_DISPLAY_MS - elapsed)
  if (remaining > 0) {
    delayTimer = setTimeout(() => { scanState.scanning = false; delayTimer = null }, remaining)
  } else {
    scanState.scanning = false
  }
}

export function resetScanState() {
  if (delayTimer) { clearTimeout(delayTimer); delayTimer = null }
  scanState.scanning = false
  scanState.msgs = 0
}
