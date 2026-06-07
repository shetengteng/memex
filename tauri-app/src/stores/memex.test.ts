import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

// Mock IPC 层：让 store 不真的调 Tauri
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}))

import { invoke } from '@tauri-apps/api/core'
import {
  adapters,
  ADAPTER_MAP,
  breakdownByAdapter,
  daemon,
  daemonStatus,
  initMemexStore,
  loadMoreSessions,
  projects,
  refreshBreakdown,
  refreshProjects,
  refreshSessions,
  sessions,
  stats,
  totals,
} from './memex'

const mockedInvoke = invoke as unknown as ReturnType<typeof vi.fn>

describe('stores/memex', () => {
  beforeEach(() => {
    mockedInvoke.mockReset()
    sessions.splice(0, sessions.length)
    projects.splice(0, projects.length)
    for (const k of Object.keys(breakdownByAdapter)) delete breakdownByAdapter[k]
    stats.value = null
    daemon.value = null
  })

  afterEach(() => {
    sessions.splice(0, sessions.length)
    projects.splice(0, projects.length)
    for (const k of Object.keys(breakdownByAdapter)) delete breakdownByAdapter[k]
    stats.value = null
    daemon.value = null
  })

  describe('adapter meta', () => {
    it('exposes 7 adapters', () => {
      expect(adapters.length).toBe(7)
    })

    it('ADAPTER_MAP indexes by id', () => {
      expect(ADAPTER_MAP.cursor?.label).toBe('Cursor')
      expect(ADAPTER_MAP.claude_code?.label).toBe('Claude Code')
    })
  })

  describe('totals reactivity', () => {
    it('reads zeros until stats arrives', () => {
      expect(totals.sessions).toBe(0)
      expect(totals.messages).toBe(0)
    })

    it('syncs from stats ref via watch', async () => {
      stats.value = {
        sessions: 100,
        messages: 5000,
        chunks: 0,
        db_exists: true,
        summaries: 0,
        sessions_eligible_for_summary: 0,
        chunks_summarized: 0,
        llm_provider: 'ollama',
      }
      // watch 是同步 flush 'pre' 默认是 lazy，等一个 tick
      await new Promise((r) => setTimeout(r, 0))
      expect(totals.sessions).toBe(100)
      expect(totals.messages).toBe(5000)
    })
  })

  describe('daemonStatus reactivity', () => {
    it('default running=false when daemon ref null', () => {
      expect(daemonStatus.running).toBe(false)
    })

    it('syncs from daemon ref', async () => {
      daemon.value = {
        running: true,
        pid: 1,
        port: 45291,
        http_ok: true,
        started_at: '2026-06-07T05:00:00+00:00',
      }
      await new Promise((r) => setTimeout(r, 0))
      expect(daemonStatus.running).toBe(true)
      expect(daemonStatus.startedAt).toBe('2026-06-07T05:00:00+00:00')
    })
  })

  describe('initMemexStore', () => {
    it('populates sessions from list_recent', async () => {
      mockedInvoke.mockImplementation(async (cmd: string) => {
        if (cmd === 'list_recent') {
          return [
            {
              id: 's1',
              source: 'cursor',
              project_path: '/me/repo',
              title: 'hello',
              message_count: 5,
              created_at: '2026-06-06T10:00:00Z',
              updated_at: '2026-06-06T10:30:00Z',
              summary_title: null,
              first_user_message: 'first msg',
            },
          ]
        }
        if (cmd === 'list_projects') return []
        return null
      })

      await initMemexStore(true) // force=true 绕开幂等
      expect(sessions.length).toBe(1)
      expect(sessions[0].id).toBe('s1')
      expect(sessions[0].adapter).toBe('cursor')
      expect(sessions[0].title).toBe('hello')
    })

    it('falls back to first_user_message when title is empty', async () => {
      mockedInvoke.mockImplementation(async (cmd: string) => {
        if (cmd === 'list_recent') {
          return [
            {
              id: 's1',
              source: 'cursor',
              project_path: '/me/repo',
              title: '',
              message_count: 1,
              created_at: '',
              updated_at: '',
              summary_title: null,
              first_user_message: 'how do I',
            },
          ]
        }
        return []
      })
      await initMemexStore(true)
      expect(sessions[0].title).toBe('how do I')
    })

    it('refreshSessions reuses the same array reference', async () => {
      mockedInvoke.mockResolvedValue([])
      const ref1 = sessions
      await refreshSessions()
      expect(sessions).toBe(ref1) // 同一个 reactive proxy
    })

    it('refreshProjects keeps existing reference', async () => {
      mockedInvoke.mockResolvedValue([])
      const ref1 = projects
      await refreshProjects()
      expect(projects).toBe(ref1)
    })
  })

  describe('breakdownByAdapter', () => {
    it('starts empty', () => {
      expect(Object.keys(breakdownByAdapter).length).toBe(0)
    })

    it('refreshBreakdown writes by_adapter into reactive object', async () => {
      mockedInvoke.mockImplementation(async (cmd: string) => {
        if (cmd === 'get_breakdown') {
          return {
            by_adapter: { cursor: 12, claude_code: 7 },
            by_project: {},
            recent_7d_sessions: 0,
            recent_7d_messages: 0,
            recent_30d_sessions: 0,
            recent_30d_messages: 0,
          }
        }
        return null
      })
      await refreshBreakdown()
      expect(breakdownByAdapter.cursor).toBe(12)
      expect(breakdownByAdapter.claude_code).toBe(7)
    })

    it('initMemexStore also pulls breakdown', async () => {
      mockedInvoke.mockImplementation(async (cmd: string) => {
        if (cmd === 'list_recent') return []
        if (cmd === 'list_projects') return []
        if (cmd === 'get_breakdown') {
          return {
            by_adapter: { codex: 3 },
            by_project: {},
            recent_7d_sessions: 0,
            recent_7d_messages: 0,
            recent_30d_sessions: 0,
            recent_30d_messages: 0,
          }
        }
        return null
      })
      await initMemexStore(true)
      expect(breakdownByAdapter.codex).toBe(3)
    })

    it('refreshBreakdown swallows errors', async () => {
      mockedInvoke.mockImplementation(async () => {
        throw new Error('boom')
      })
      await expect(refreshBreakdown()).resolves.toBeUndefined()
    })
  })

  describe('loadMoreSessions', () => {
    function buildRow(id: string) {
      return {
        id,
        source: 'cursor' as const,
        project_path: '/me/repo',
        title: id,
        message_count: 1,
        created_at: '',
        updated_at: '',
        summary_title: null,
        first_user_message: '',
      }
    }

    it('appends rows and returns hasMore=true when full page returned', async () => {
      sessions.splice(0, sessions.length, {
        id: 'old',
      } as never)
      // pageSize=2, 后端返回 2 条 → hasMore=true
      mockedInvoke.mockImplementation(async (cmd: string, args?: { offset?: number }) => {
        if (cmd === 'list_recent') {
          expect(args?.offset).toBe(1) // sessions.length === 1
          return [buildRow('a'), buildRow('b')]
        }
        return null
      })
      const r = await loadMoreSessions(2)
      expect(r).toEqual({ loaded: 2, hasMore: true })
      expect(sessions.length).toBe(3)
      expect(sessions[1].id).toBe('a')
      expect(sessions[2].id).toBe('b')
    })

    it('returns hasMore=false when partial page returned', async () => {
      mockedInvoke.mockImplementation(async (cmd: string) => {
        if (cmd === 'list_recent') return [buildRow('only-one')]
        return null
      })
      const r = await loadMoreSessions(5)
      expect(r).toEqual({ loaded: 1, hasMore: false })
    })

    it('returns hasMore=false when no rows returned', async () => {
      mockedInvoke.mockImplementation(async (cmd: string) => {
        if (cmd === 'list_recent') return []
        return null
      })
      const r = await loadMoreSessions(10)
      expect(r).toEqual({ loaded: 0, hasMore: false })
      expect(sessions.length).toBe(0)
    })

    it('swallows backend errors', async () => {
      mockedInvoke.mockImplementation(async () => {
        throw new Error('db gone')
      })
      const r = await loadMoreSessions()
      expect(r).toEqual({ loaded: 0, hasMore: false })
    })
  })
})
