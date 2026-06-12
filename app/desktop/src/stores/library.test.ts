import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}))

import { invoke } from '@tauri-apps/api/core'
import {
  libraryHasMore,
  librarySessions,
  loadMoreLibrarySessions,
  reloadLibrarySessions,
} from './library'

const mockedInvoke = invoke as unknown as ReturnType<typeof vi.fn>

function buildRow(id: string, source: string = 'cursor') {
  return {
    id,
    source,
    project_path: '/me/repo',
    title: id,
    message_count: 1,
    created_at: '',
    updated_at: '',
    summary_title: null,
    first_user_message: '',
    intent: null,
  }
}

describe('stores/library (filter pushdown)', () => {
  beforeEach(() => {
    mockedInvoke.mockReset()
    librarySessions.splice(0, librarySessions.length)
    libraryHasMore.value = false
  })

  afterEach(() => {
    librarySessions.splice(0, librarySessions.length)
    libraryHasMore.value = false
  })

  it('reloadLibrarySessions replaces the whole list and forwards filter to IPC', async () => {
    librarySessions.push({ id: 'stale' } as never)

    let observedArgs: Record<string, unknown> | undefined
    mockedInvoke.mockImplementation(async (cmd: string, args?: Record<string, unknown>) => {
      if (cmd === 'list_sessions_filtered') {
        observedArgs = args
        return [buildRow('a', 'codex'), buildRow('b', 'codex')]
      }
      return null
    })

    await reloadLibrarySessions({ adapters: ['codex'], time: '7d' })

    expect(librarySessions.length).toBe(2)
    expect(librarySessions[0].id).toBe('a')
    expect(librarySessions[0].adapter).toBe('codex')
    expect(observedArgs?.filter).toEqual({ adapters: ['codex'], time: '7d' })
    expect(observedArgs?.limit).toBe(200)
    expect(observedArgs?.offset).toBe(0)
  })

  it('reloadLibrarySessions marks hasMore=true when page is full', async () => {
    // 200 = LIBRARY_PAGE_SIZE
    const rows = Array.from({ length: 200 }, (_, i) => buildRow(`s${i}`))
    mockedInvoke.mockResolvedValue(rows)
    await reloadLibrarySessions({})
    expect(librarySessions.length).toBe(200)
    expect(libraryHasMore.value).toBe(true)
  })

  it('reloadLibrarySessions marks hasMore=false when page is partial', async () => {
    mockedInvoke.mockResolvedValue([buildRow('only')])
    await reloadLibrarySessions({})
    expect(librarySessions.length).toBe(1)
    expect(libraryHasMore.value).toBe(false)
  })

  it('reloadLibrarySessions swallows backend errors and keeps the list intact', async () => {
    librarySessions.push({ id: 'keep' } as never)
    mockedInvoke.mockImplementation(async () => {
      throw new Error('db gone')
    })
    await reloadLibrarySessions({ adapters: ['cursor'] })
    // 失败后不改变现状，让 UI 不会因为短暂错误闪空
    expect(librarySessions.length).toBe(1)
    expect((librarySessions[0] as { id: string }).id).toBe('keep')
  })

  it('loadMoreLibrarySessions appends with offset = librarySessions.length and reuses the same filter', async () => {
    librarySessions.push({ id: 'first' } as never)
    let observedArgs: Record<string, unknown> | undefined
    mockedInvoke.mockImplementation(async (cmd: string, args?: Record<string, unknown>) => {
      if (cmd === 'list_sessions_filtered') {
        observedArgs = args
        return [buildRow('p1'), buildRow('p2')]
      }
      return null
    })

    const r = await loadMoreLibrarySessions({ adapters: ['cursor'] }, 2)

    expect(r).toEqual({ loaded: 2, hasMore: true })
    expect(librarySessions.length).toBe(3)
    expect(observedArgs?.offset).toBe(1)
    expect(observedArgs?.limit).toBe(2)
    expect(observedArgs?.filter).toEqual({ adapters: ['cursor'] })
    expect(libraryHasMore.value).toBe(true)
  })

  it('loadMoreLibrarySessions returns hasMore=false when next page is partial', async () => {
    mockedInvoke.mockResolvedValue([buildRow('only-one')])
    const r = await loadMoreLibrarySessions({}, 5)
    expect(r).toEqual({ loaded: 1, hasMore: false })
    expect(libraryHasMore.value).toBe(false)
  })

  it('loadMoreLibrarySessions returns hasMore=false when next page is empty', async () => {
    mockedInvoke.mockResolvedValue([])
    const r = await loadMoreLibrarySessions({})
    expect(r).toEqual({ loaded: 0, hasMore: false })
    expect(libraryHasMore.value).toBe(false)
  })

  it('loadMoreLibrarySessions swallows backend errors', async () => {
    mockedInvoke.mockImplementation(async () => {
      throw new Error('boom')
    })
    const r = await loadMoreLibrarySessions({})
    expect(r).toEqual({ loaded: 0, hasMore: false })
  })
})
