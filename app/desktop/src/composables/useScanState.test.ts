import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { resetScanState, scanState, startScanning, stopScanning } from './useScanState'

describe('useScanState', () => {
  beforeEach(() => {
    vi.useFakeTimers()
    resetScanState()
  })
  afterEach(() => {
    vi.useRealTimers()
    resetScanState()
  })

  it('startScanning flips state and resets msgs', () => {
    scanState.msgs = 42
    startScanning()
    expect(scanState.scanning).toBe(true)
    expect(scanState.msgs).toBe(0)
  })

  it('stopScanning preserves min display of 2s', () => {
    startScanning()
    // 立即停止 → 还需要至少 2s 才真正 hide
    stopScanning()
    expect(scanState.scanning).toBe(true)
    vi.advanceTimersByTime(1999)
    expect(scanState.scanning).toBe(true)
    vi.advanceTimersByTime(2)
    expect(scanState.scanning).toBe(false)
  })

  it('stopScanning hides immediately after min display reached', () => {
    startScanning()
    vi.advanceTimersByTime(3000)
    stopScanning()
    expect(scanState.scanning).toBe(false)
  })

  it('resetScanState clears scanning + msgs + pending timer', () => {
    startScanning()
    scanState.msgs = 7
    stopScanning() // 计时器开始
    resetScanState()
    expect(scanState.scanning).toBe(false)
    expect(scanState.msgs).toBe(0)
    // 确保定时器被清掉，再走 2s 不会突然翻回 true
    vi.advanceTimersByTime(5000)
    expect(scanState.scanning).toBe(false)
  })
})
