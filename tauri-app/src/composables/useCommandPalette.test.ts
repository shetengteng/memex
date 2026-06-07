import { describe, expect, it } from 'vitest'
import { useCommandPalette } from './useCommandPalette'

describe('useCommandPalette', () => {
  it('starts closed', () => {
    const { isOpen } = useCommandPalette()
    expect(isOpen.value).toBe(false)
  })

  it('open() sets isOpen=true', () => {
    const palette = useCommandPalette()
    palette.close()
    palette.open()
    expect(palette.isOpen.value).toBe(true)
  })

  it('close() sets isOpen=false', () => {
    const palette = useCommandPalette()
    palette.open()
    palette.close()
    expect(palette.isOpen.value).toBe(false)
  })

  it('toggle() flips the value', () => {
    const palette = useCommandPalette()
    palette.close()
    palette.toggle()
    expect(palette.isOpen.value).toBe(true)
    palette.toggle()
    expect(palette.isOpen.value).toBe(false)
  })

  it('state is shared across calls (module-singleton)', () => {
    const a = useCommandPalette()
    const b = useCommandPalette()
    a.close()
    expect(b.isOpen.value).toBe(false)
    a.open()
    expect(b.isOpen.value).toBe(true)
    a.close()
  })
})
