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
  computeDurationMin,
  daemon,
  daemonStatus,
  deleteThread,
  fetchThreadDetail,
  initMemexStore,
  loadMoreSessions,
  projects,
  refreshBreakdown,
  refreshProjects,
  refreshSessions,
  refreshThreads,
  regenerateThreads,
  searchThreadByQuery,
  sessions,
  stats,
  threads,
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
        llm_model: 'qwen2.5:7b',
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
              projectPath: '/me/repo',
              title: 'hello',
              messageCount: 5,
              createdAt: '2026-06-06T10:00:00Z',
              updatedAt: '2026-06-06T10:30:00Z',
              summaryTitle: null,
              firstUserMessage: 'first msg',
              intent: null,
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
              projectPath: '/me/repo',
              title: '',
              messageCount: 1,
              createdAt: '',
              updatedAt: '',
              summaryTitle: null,
              firstUserMessage: 'how do I',
              intent: null,
            },
          ]
        }
        return []
      })
      await initMemexStore(true)
      expect(sessions[0].title).toBe('how do I')
    })

    // claude_code workflow agent 把 `=== Role === ...` 这种 system prompt 模板
    // 写到第一条 user message。store 在做 title / intent fallback 时必须把它
    // 当作"没值"，否则 UI 上会出现一堆挂着 === Role === 的会话卡。
    it('rowToSession strips noise (=== Role ===) from title and intent fallback', async () => {
      mockedInvoke.mockImplementation(async (cmd: string) => {
        if (cmd === 'list_recent') {
          return [
            {
              id: 's-noise',
              source: 'claude_code',
              projectPath: '/me/repo',
              title: '',
              messageCount: 1,
              createdAt: '',
              updatedAt: '',
              summaryTitle: null,
              firstUserMessage: '=== Role ===\n你是 agent。\n=== Task ===\n做事。',
              intent: null,
            },
          ]
        }
        return []
      })
      await initMemexStore(true)
      // title 不能被设置为 === Role === 片段
      expect(sessions[0].title.includes('=== Role')).toBe(false)
      // 既然 noise 被过滤，title fallback 链最终拿到 "(无标题)"
      expect(sessions[0].title).toBe('(无标题)')
      // intent 也不应被 noise 污染
      expect(sessions[0].intent).toBeUndefined()
    })

    it('rowToSession maps summary intent into session.intent', async () => {
      mockedInvoke.mockImplementation(async (cmd: string) => {
        if (cmd === 'list_recent') {
          return [
            {
              id: 's-intent',
              source: 'cursor',
              projectPath: '/me/repo',
              title: 'with summary',
              messageCount: 4,
              createdAt: '',
              updatedAt: '',
              summaryTitle: 'with summary',
              firstUserMessage: 'first',
              intent: '修复登录失败的问题',
            },
          ]
        }
        if (cmd === 'list_projects') return []
        return null
      })
      await initMemexStore(true)
      expect(sessions[0].intent).toBe('修复登录失败的问题')
    })

    it('rowToSession falls back to first_user_message when intent is null', async () => {
      mockedInvoke.mockImplementation(async (cmd: string) => {
        if (cmd === 'list_recent') {
          return [
            {
              id: 's-fallback',
              source: 'cursor',
              projectPath: '/me/repo',
              title: 'no summary yet',
              messageCount: 1,
              createdAt: '',
              updatedAt: '',
              summaryTitle: null,
              firstUserMessage: 'how do I read this thing',
              intent: null,
            },
          ]
        }
        if (cmd === 'list_projects') return []
        return null
      })
      await initMemexStore(true)
      expect(sessions[0].intent).toBe('how do I read this thing')
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
        projectPath: '/me/repo',
        title: id,
        messageCount: 1,
        createdAt: '',
        updatedAt: '',
        summaryTitle: null,
        firstUserMessage: '',
        intent: null,
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

  describe('threads (L5)', () => {
    beforeEach(() => {
      threads.splice(0, threads.length)
    })

    it('refreshThreads populates the reactive list', async () => {
      mockedInvoke.mockImplementation(async (cmd: string) => {
        if (cmd === 'list_threads') {
          return [
            {
              id: 1,
              name: '桌面化',
              summary: 'memex 桌面化迁移',
              sessionCount: 3,
              createdAt: '2026-06-08T10:00:00+00:00',
              updatedAt: '2026-06-08T11:00:00+00:00',
            },
          ]
        }
        return null
      })
      await refreshThreads()
      expect(threads.length).toBe(1)
      expect(threads[0].name).toBe('桌面化')
    })

    it('refreshThreads swallows backend errors and leaves list untouched', async () => {
      threads.push({
        id: 99,
        name: 'preexisting',
        summary: '',
        sessionCount: 0,
        createdAt: '',
        updatedAt: '',
      })
      mockedInvoke.mockImplementation(async () => {
        throw new Error('db gone')
      })
      await refreshThreads()
      expect(threads.length).toBe(1)
      expect(threads[0].name).toBe('preexisting')
    })

    it('fetchThreadDetail forwards id and returns detail', async () => {
      mockedInvoke.mockImplementation(async (cmd: string, args?: { threadId?: number }) => {
        if (cmd === 'get_thread_detail') {
          expect(args?.threadId).toBe(42)
          return {
            thread: {
              id: 42,
              name: 'foo',
              summary: 'bar',
              sessionCount: 0,
              createdAt: '',
              updatedAt: '',
            },
            sessions: [],
          }
        }
        return null
      })
      const d = await fetchThreadDetail(42)
      expect(d?.thread.name).toBe('foo')
    })

    it('fetchThreadDetail returns null on error', async () => {
      mockedInvoke.mockImplementation(async () => {
        throw new Error('boom')
      })
      const d = await fetchThreadDetail(1)
      expect(d).toBeNull()
    })

    it('regenerateThreads invokes backend then refreshes list', async () => {
      let calls = 0
      mockedInvoke.mockImplementation(async (cmd: string) => {
        calls += 1
        if (cmd === 'regenerate_threads') return 5
        if (cmd === 'list_threads') {
          return [
            {
              id: 7,
              name: 'new',
              summary: '',
              sessionCount: 2,
              createdAt: '',
              updatedAt: '',
            },
          ]
        }
        return null
      })
      const n = await regenerateThreads()
      expect(n).toBe(5)
      expect(calls).toBe(2) // regenerate + list
      expect(threads.length).toBe(1)
      expect(threads[0].id).toBe(7)
    })

    it('regenerateThreads rethrows backend errors', async () => {
      mockedInvoke.mockImplementation(async () => {
        throw new Error('LLM not configured')
      })
      await expect(regenerateThreads()).rejects.toThrow('LLM not configured')
    })

    // 回归：Tauri 把 CmdError 反序列化成 plain object {kind, message}。
    // 早期 store 用 `new Error(String(e))` 包了一层，导致 humanizeBackendError 拿到
    // "[object Object]" 而不是真正的 message。现在 store 应当原样把 plain object 抛出，
    // humanizeBackendError 才能根据 kind 生成友好文案。
    it('regenerateThreads preserves Tauri plain-object errors instead of stringifying', async () => {
      const backendErr = { kind: 'backend', message: '未配置 LLM 提供方' }
      mockedInvoke.mockImplementation(async () => {
        throw backendErr
      })
      try {
        await regenerateThreads()
        throw new Error('expected regenerateThreads to reject')
      } catch (e) {
        expect(e).toBe(backendErr)
        expect(e).toMatchObject({ kind: 'backend', message: '未配置 LLM 提供方' })
      }
    })

    it('deleteThread removes the thread from the reactive list', async () => {
      threads.push(
        {
          id: 1,
          name: 'keep',
          summary: '',
          sessionCount: 0,
          createdAt: '',
          updatedAt: '',
        },
        {
          id: 2,
          name: 'remove',
          summary: '',
          sessionCount: 0,
          createdAt: '',
          updatedAt: '',
        },
      )
      mockedInvoke.mockImplementation(
        async (cmd: string, args?: { threadId?: number }) => {
          if (cmd === 'delete_thread') {
            expect(args?.threadId).toBe(2)
            return undefined
          }
          return undefined
        },
      )
      await deleteThread(2)
      expect(threads.length).toBe(1)
      expect(threads[0].id).toBe(1)
    })

    it('deleteThread rethrows backend errors', async () => {
      mockedInvoke.mockImplementation(async () => {
        throw new Error('not found')
      })
      await expect(deleteThread(99)).rejects.toThrow('not found')
    })

    it('searchThreadByQuery invokes backend with query and refreshes list', async () => {
      let listCalls = 0
      mockedInvoke.mockImplementation(
        async (cmd: string, args?: { query?: string }) => {
          if (cmd === 'search_thread_by_query') {
            expect(args?.query).toBe('Tauri 多窗口')
            return 42
          }
          if (cmd === 'list_threads') {
            listCalls += 1
            return [
              {
                id: 42,
                name: 'Tauri 多窗口',
                summary: '',
                sessionCount: 3,
                createdAt: '',
                updatedAt: '',
              },
            ]
          }
          return null
        },
      )
      const id = await searchThreadByQuery('Tauri 多窗口')
      expect(id).toBe(42)
      expect(listCalls).toBe(1)
      expect(threads[0].name).toBe('Tauri 多窗口')
    })

    it('searchThreadByQuery rethrows backend errors', async () => {
      mockedInvoke.mockImplementation(async () => {
        throw new Error('未找到与「foo」相关的会话')
      })
      await expect(searchThreadByQuery('foo')).rejects.toThrow('未找到')
    })
  })

  describe('computeDurationMin', () => {
    it('returns minutes for a normal positive interval', () => {
      const out = computeDurationMin(
        '2026-06-08T10:00:00+00:00',
        '2026-06-08T10:42:00+00:00',
      )
      expect(out).toBe(42)
    })

    it('rounds sub-minute intervals up to 1m so列表里"有数据但不到 1 分钟"也能看到', () => {
      const out = computeDurationMin(
        '2026-06-08T10:00:00+00:00',
        '2026-06-08T10:00:30+00:00',
      )
      expect(out).toBe(1)
    })

    it('returns 0 when timestamps are equal', () => {
      const t = '2026-06-08T10:00:00+00:00'
      expect(computeDurationMin(t, t)).toBe(0)
    })

    it('returns 0 when updated_at is earlier than created_at (会话数据脏，不能给负值)', () => {
      const out = computeDurationMin(
        '2026-06-08T11:00:00+00:00',
        '2026-06-08T10:00:00+00:00',
      )
      expect(out).toBe(0)
    })

    it('returns 0 for empty or null inputs', () => {
      expect(computeDurationMin('', '')).toBe(0)
      expect(computeDurationMin(null, '2026-06-08T10:00:00+00:00')).toBe(0)
      expect(computeDurationMin('2026-06-08T10:00:00+00:00', undefined)).toBe(0)
    })

    it('returns 0 for unparsable strings', () => {
      expect(computeDurationMin('not-a-date', 'also-bad')).toBe(0)
    })

    it('handles multi-hour sessions correctly', () => {
      const out = computeDurationMin(
        '2026-06-08T08:00:00+00:00',
        '2026-06-08T11:30:00+00:00',
      )
      expect(out).toBe(210)
    })
  })
})
