import { describe, expect, it, vi } from 'vitest'
import { mount } from '@vue/test-utils'
import LibrarySessionListItem from './LibrarySessionListItem.vue'
import type { Session } from '@/stores/memex'

vi.mock('@/i18n', () => ({
  useI18n: () => ({
    locale: { value: 'zh' },
    t: (key: string) => {
      const dict: Record<string, string> = {
        'library.list.action.mark_private': '标记为私有',
        'library.list.action.unmark_private': '已私有 · 点击取消标记',
        'library.list.action.summarize_now': '点击立即生成会话摘要（L2）',
        'library.list.action.regenerate_summary': '点击重新生成会话摘要（L2）',
        'library.list.tooltip.l2_done': '已生成会话摘要（L2）',
        'library.list.tooltip.l2_pending': '尚未生成会话摘要（L2）',
        'library.list.tooltip.summarizing': '正在生成会话摘要…',
        'library.list.badge.l2_done': '已摘要',
        'library.list.badge.l2_pending': '未摘要',
        'library.list.badge.summarizing': '生成中',
      }
      return dict[key] ?? key
    },
  }),
}))

vi.mock('@/components/shell/IdeChip.vue', () => ({
  default: {
    name: 'IdeChip',
    props: ['adapter'],
    template: '<span class="ide-chip">{{ adapter }}</span>',
  },
}))

const baseSession: Session = {
  id: 'sess-1',
  adapter: 'cursor',
  workspace: '/proj/demo',
  project: 'demo',
  startedAt: '2026-06-01T10:00:00Z',
  durationMin: 12,
  messages: 5,
  title: 'Demo Session',
  topics: ['rust'],
  l2Done: true,
  isPrivate: false,
}

const stubs = {
  Badge: { template: '<span><slot/></span>' },
}

describe('LibrarySessionListItem · 私有标记锁按钮', () => {
  /// 改造后锁按钮**常驻**显示（不再 hover-only），让用户一眼分辨私有/公开。
  /// 关键断言：未私有时按钮存在、aria-pressed=false、不应再有 opacity-0 隐藏类。
  it('未私有时锁按钮常驻显示，使用 LockOpen 图标，灰色', () => {
    const wrapper = mount(LibrarySessionListItem, {
      props: { session: baseSession, groupKey: 'today', active: false },
      global: { stubs },
    })
    const lock = wrapper.find('[role="button"][aria-label="标记为私有"]')
    expect(lock.exists()).toBe(true)
    expect(lock.attributes('aria-pressed')).toBe('false')
    const cls = lock.classes().join(' ')
    expect(cls).not.toContain('opacity-0')
    expect(cls).toContain('text-muted-foreground/60')
    // 未私有图标应为 LockOpen（lucide-vue-next 渲染成 svg.lucide-lock-open）
    expect(lock.find('svg.lucide-lock-open').exists()).toBe(true)
    expect(lock.find('svg.lucide-lock').exists()).toBe(false)
  })

  /// 私有时颜色应为 red（不再用 amber），图标切换为闭合 Lock。
  it('已私有时锁按钮 aria-pressed=true，红色 + Lock 图标', () => {
    const wrapper = mount(LibrarySessionListItem, {
      props: {
        session: { ...baseSession, isPrivate: true },
        groupKey: 'today',
        active: false,
      },
      global: { stubs },
    })
    const lock = wrapper.find('[role="button"][aria-label="已私有 · 点击取消标记"]')
    expect(lock.exists()).toBe(true)
    expect(lock.attributes('aria-pressed')).toBe('true')
    const cls = lock.classes().join(' ')
    expect(cls).toContain('text-red-600')
    expect(cls).not.toContain('opacity-0')
    expect(lock.find('svg.lucide-lock').exists()).toBe(true)
  })

  /// 关键交互：点击锁按钮只能触发 toggle-private，**不能**冒泡到外层 button
  /// 触发 open 事件——否则用户每点一次锁就同时开 Drawer。
  it('点击锁按钮触发 toggle-private，不触发 open（@click.stop）', async () => {
    const wrapper = mount(LibrarySessionListItem, {
      props: { session: baseSession, groupKey: 'today', active: false },
      global: { stubs },
    })
    const lock = wrapper.find('[role="button"][aria-label="标记为私有"]')
    await lock.trigger('click')

    expect(wrapper.emitted('toggle-private')).toBeTruthy()
    expect(wrapper.emitted('toggle-private')![0][0]).toMatchObject({ id: 'sess-1' })
    expect(wrapper.emitted('open')).toBeFalsy()
  })

  /// 键盘 a11y：Enter 必须仍然能触发 toggle，且通过 .stop.prevent 阻止外层 button 默认行为。
  it('键盘 Enter 触发 toggle-private（保留 a11y）', async () => {
    const wrapper = mount(LibrarySessionListItem, {
      props: { session: baseSession, groupKey: 'today', active: false },
      global: { stubs },
    })
    const lock = wrapper.find('[role="button"][aria-label="标记为私有"]')
    await lock.trigger('keydown', { key: 'Enter' })
    expect(wrapper.emitted('toggle-private')).toBeTruthy()
  })

  /// 行体（标题区域）点击仍然要触发 open——锁按钮的 @click.stop 不能误伤
  /// 外层 button 自己的 click 行为。
  it('点击行体（非锁按钮）正常触发 open', async () => {
    const wrapper = mount(LibrarySessionListItem, {
      props: { session: baseSession, groupKey: 'today', active: false },
      global: { stubs },
    })
    await wrapper.trigger('click')
    expect(wrapper.emitted('open')).toBeTruthy()
  })
})

describe('LibrarySessionListItem · 摘要状态 badge 可点击', () => {
  /// 已摘要 → 点击触发 summarize（用于"重新生成"），不冒泡到 open。
  it('点击"已摘要" badge 触发 summarize，不触发 open', async () => {
    const wrapper = mount(LibrarySessionListItem, {
      props: { session: baseSession, groupKey: 'today', active: false },
      global: { stubs },
    })
    const badge = wrapper.find('[role="button"][aria-label="点击重新生成会话摘要（L2）"]')
    expect(badge.exists()).toBe(true)
    await badge.trigger('click')

    expect(wrapper.emitted('summarize')).toBeTruthy()
    expect(wrapper.emitted('summarize')![0][0]).toMatchObject({ id: 'sess-1' })
    expect(wrapper.emitted('open')).toBeFalsy()
  })

  /// 未摘要 → 点击触发 summarize（用于"立即生成"）。
  it('点击"未摘要" badge 触发 summarize，不触发 open', async () => {
    const wrapper = mount(LibrarySessionListItem, {
      props: {
        session: { ...baseSession, l2Done: false },
        groupKey: 'today',
        active: false,
      },
      global: { stubs },
    })
    const badge = wrapper.find('[role="button"][aria-label="点击立即生成会话摘要（L2）"]')
    expect(badge.exists()).toBe(true)
    await badge.trigger('click')

    expect(wrapper.emitted('summarize')).toBeTruthy()
    expect(wrapper.emitted('summarize')![0][0]).toMatchObject({ id: 'sess-1' })
    expect(wrapper.emitted('open')).toBeFalsy()
  })

  /// 键盘 a11y：Enter 触发 summarize。
  it('键盘 Enter 在"未摘要" badge 上触发 summarize', async () => {
    const wrapper = mount(LibrarySessionListItem, {
      props: {
        session: { ...baseSession, l2Done: false },
        groupKey: 'today',
        active: false,
      },
      global: { stubs },
    })
    const badge = wrapper.find('[role="button"][aria-label="点击立即生成会话摘要（L2）"]')
    await badge.trigger('keydown', { key: 'Enter' })
    expect(wrapper.emitted('summarize')).toBeTruthy()
  })

  /// summarizing=true 时显示"生成中" + spinner，且 badge 不再可点击（不存在 aria-label
  /// 为"立即生成 / 重新生成"的 role=button），防止用户在 LLM 跑的时候重复触发。
  it('summarizing=true 时禁用点击、显示 spinner', () => {
    const wrapper = mount(LibrarySessionListItem, {
      props: {
        session: baseSession,
        groupKey: 'today',
        active: false,
        summarizing: true,
      },
      global: { stubs },
    })
    expect(wrapper.text()).toContain('生成中')
    // lucide-vue-next 当前把 Loader2 渲染成 svg.lucide-loader-circle（在 v0.475+ 重命名）。
    expect(wrapper.find('svg.lucide-loader-circle').exists()).toBe(true)
    expect(wrapper.find('[role="button"][aria-label="点击重新生成会话摘要（L2）"]').exists()).toBe(false)
    expect(wrapper.find('[role="button"][aria-label="点击立即生成会话摘要（L2）"]').exists()).toBe(false)
  })
})
