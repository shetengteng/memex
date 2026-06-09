import { describe, expect, it, vi, beforeEach } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import { nextTick } from 'vue'
import LibrarySessionDrawer from './LibrarySessionDrawer.vue'
import type { SessionDetail } from '@/types'

// Mock useMemex 暴露 getSession：测试自行控制每次返回的 detail
const ipcMocks = vi.hoisted(() => ({
  getSession: vi.fn(),
}))

vi.mock('@/composables/useMemex', () => ({
  useMemex: () => ipcMocks,
}))

// 简单的中文 i18n stub，避免依赖完整 i18n 设置
vi.mock('@/i18n', () => ({
  useI18n: () => ({
    t: (key: string, params?: Record<string, unknown>) => {
      const dict: Record<string, string> = {
        'session.role.user': '用户',
        'session.role.assistant': '助手',
        'session.messages': '消息',
      }
      if (key === 'session.load_more') {
        return `加载更多（还剩 ${params?.count} 条）`
      }
      return dict[key] ?? key
    },
  }),
}))

// MessageContent → MarkdownContent + ToolCallBlock。在测试里只需要把 content 直接吐出来
// （避免 markdown-it 的 DOM 副作用），所以两个组件都用同一个 stub。
vi.mock('@/components/MessageContent.vue', () => ({
  default: {
    name: 'MessageContent',
    props: ['content'],
    template: '<div class="md-stub">{{ content }}</div>',
  },
}))

import type { Session } from '@/stores/memex'

const baseSession: Session = {
  id: 'sess-x',
  adapter: 'cursor',
  workspace: '/tmp/demo',
  project: 'demo',
  startedAt: '2026-06-01T10:00:00Z',
  durationMin: 12,
  messages: 29,
  title: 'Demo Session',
  topics: [],
  l2Done: true,
}

function makeDetail(messageCount: number): SessionDetail {
  return {
    id: 'sess-x',
    source: 'cursor',
    project_path: '/tmp/demo',
    title: 'Demo Session',
    summary: 'a summary',
    topics: [],
    decisions: [],
    file_path: '/tmp/demo.json',
    message_count: messageCount,
    created_at: '2026-06-01T10:00:00Z',
    updated_at: '2026-06-01T11:00:00Z',
    messages: Array.from({ length: messageCount }, (_, i) => ({
      id: `m-${i}`,
      role: i % 2 === 0 ? 'user' : 'assistant',
      content: `msg-${i} body`,
      timestamp: '2026-06-01T10:00:00Z',
    })),
    intent: null,
  }
}

const stubs = {
  Dialog: { template: '<div><slot/></div>', props: ['open'], emits: ['update:open'] },
  DialogContent: { template: '<div><slot/></div>' },
  DialogTitle: { template: '<div class="dialog-title"><slot/></div>' },
  DialogDescription: { template: '<div class="dialog-desc"><slot/></div>' },
  VisuallyHidden: { template: '<div class="sr-only"><slot/></div>' },
  Badge: { template: '<span><slot/></span>' },
  Button: {
    template:
      '<button :disabled="disabled" @click="$emit(\'click\')"><slot/></button>',
    props: ['disabled', 'variant', 'size'],
    emits: ['click'],
  },
  IdeChip: { template: '<span class="ide-chip">{{ adapter }}</span>', props: ['adapter'] },
  // lucide-vue-next icons：避免 SVG 渲染开销
  User: { template: '<i class="i-user" />' },
  Bot: { template: '<i class="i-bot" />' },
}

describe('LibrarySessionDrawer', () => {
  beforeEach(() => {
    ipcMocks.getSession.mockReset()
  })

  it('打开后渲染 detail 摘要与所有消息（默认前 50 条）', async () => {
    ipcMocks.getSession.mockResolvedValue(makeDetail(29))
    const wrapper = mount(LibrarySessionDrawer, {
      props: { session: baseSession, open: true },
      global: { stubs },
    })
    await flushPromises()
    await nextTick()

    expect(ipcMocks.getSession).toHaveBeenCalledWith('sess-x')
    const html = wrapper.html()
    expect(html).toContain('a summary')
    expect(html).toContain('msg-0 body')
    expect(html).toContain('msg-28 body')
    expect(html).not.toContain('加载更多')
  })

  it('消息超过 50 条时显示"加载更多"且初始只渲染前 50', async () => {
    ipcMocks.getSession.mockResolvedValue(makeDetail(120))
    const wrapper = mount(LibrarySessionDrawer, {
      props: { session: baseSession, open: true },
      global: { stubs },
    })
    await flushPromises()
    await nextTick()

    const html = wrapper.html()
    expect(html).toContain('msg-0 body')
    expect(html).toContain('msg-49 body')
    expect(html).not.toContain('msg-50 body')
    expect(html).toContain('加载更多（还剩 70 条）')
  })

  it('点击"加载更多"按钮会增量渲染下一批 50 条', async () => {
    ipcMocks.getSession.mockResolvedValue(makeDetail(120))
    const wrapper = mount(LibrarySessionDrawer, {
      props: { session: baseSession, open: true },
      global: { stubs },
    })
    await flushPromises()
    await nextTick()

    const moreBtn = wrapper.findAll('button').find((b) => b.text().includes('加载更多'))
    expect(moreBtn).toBeTruthy()
    await moreBtn!.trigger('click')

    const html = wrapper.html()
    expect(html).toContain('msg-50 body')
    expect(html).toContain('msg-99 body')
    expect(html).not.toContain('msg-100 body')
    expect(html).toContain('加载更多（还剩 20 条）')
  })

  it('Drawer 不再渲染"打开完整会话/发送到 IDE/归档/删除"等底部动作按钮', async () => {
    ipcMocks.getSession.mockResolvedValue(makeDetail(2))
    const wrapper = mount(LibrarySessionDrawer, {
      props: { session: baseSession, open: true },
      global: { stubs },
    })
    await flushPromises()
    await nextTick()

    const text = wrapper.text()
    expect(text).not.toContain('打开完整会话')
    expect(text).not.toContain('发送到 IDE')
    // 归档/删除按钮的 lucide 图标也不该再出现
    expect(wrapper.find('[data-action="archive"]').exists()).toBe(false)
    expect(wrapper.find('[data-action="delete"]').exists()).toBe(false)
  })

  it('所有消息共享同一时间戳（session-level fallback）时给时间加 ~ 前缀提示', async () => {
    // 模拟 cursor / continue_dev：messages.timestamp 为 NULL，后端 COALESCE 后
    // 全部退化为 session.updated_at，这里 detail 把每条消息时间都填成同一个值
    // → UI 必须把时间显示为 "~ ..." 表示是会话级估算而非真实 message 时间。
    const detail = makeDetail(3) // makeDetail 默认每条都是同一个 timestamp
    ipcMocks.getSession.mockResolvedValue(detail)
    const wrapper = mount(LibrarySessionDrawer, {
      props: { session: baseSession, open: true },
      global: { stubs },
    })
    await flushPromises()
    await nextTick()

    const text = wrapper.text()
    expect(text).toMatch(/~ /) // 至少出现一次 fallback 标记
    // 同时确认 3 条 user/assistant 消息都被渲染（有时间戳，没有被 v-if 吃掉）
    expect(text).toContain('msg-0 body')
    expect(text).toContain('msg-1 body')
    expect(text).toContain('msg-2 body')
  })

  it('每条消息的真实 timestamp 不同时不加 ~ 前缀', async () => {
    const detail = makeDetail(3)
    detail.messages = detail.messages.map((m, i) => ({
      ...m,
      timestamp: `2026-06-01T10:0${i}:00Z`, // 真实时间互不相同
    }))
    ipcMocks.getSession.mockResolvedValue(detail)
    const wrapper = mount(LibrarySessionDrawer, {
      props: { session: baseSession, open: true },
      global: { stubs },
    })
    await flushPromises()
    await nextTick()

    const text = wrapper.text()
    expect(text).not.toMatch(/~ /)
  })

  it('open 关闭后清空 detail，再次打开重新拉取', async () => {
    ipcMocks.getSession.mockResolvedValueOnce(makeDetail(3))
    const wrapper = mount(LibrarySessionDrawer, {
      props: { session: baseSession, open: true },
      global: { stubs },
    })
    await flushPromises()
    await nextTick()
    expect(wrapper.html()).toContain('msg-0 body')

    await wrapper.setProps({ open: false })
    await flushPromises()
    expect(wrapper.html()).not.toContain('msg-0 body')

    ipcMocks.getSession.mockResolvedValueOnce(makeDetail(2))
    await wrapper.setProps({ open: true })
    await flushPromises()
    await nextTick()
    expect(ipcMocks.getSession).toHaveBeenCalledTimes(2)
  })
})
