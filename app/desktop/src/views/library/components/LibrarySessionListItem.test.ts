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
        'library.list.tooltip.l2_done': '已生成会话摘要（L2）',
        'library.list.tooltip.l2_pending': '尚未生成会话摘要（L2）',
        'library.list.badge.l2_done': '已摘要',
        'library.list.badge.l2_pending': '未摘要',
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
  /// 锁按钮总是渲染（不能 v-if 掉），未私有时通过 opacity-0 在视觉上隐藏，
  /// hover 行才淡显——这样用户能在「想标记某条会话为私有」的瞬间立即看到
  /// 入口，而不需要先打开 Drawer 再找 toggle。
  it('未私有时锁按钮存在但默认 opacity-0', () => {
    const wrapper = mount(LibrarySessionListItem, {
      props: { session: baseSession, groupKey: 'today', active: false },
      global: { stubs },
    })
    const lock = wrapper.find('[role="button"][aria-label="标记为私有"]')
    expect(lock.exists()).toBe(true)
    expect(lock.attributes('aria-pressed')).toBe('false')
    expect(lock.classes().join(' ')).toContain('opacity-0')
  })

  /// 私有时锁按钮 aria-pressed=true，颜色 amber 高亮，无 opacity-0 把它压住，
  /// 让"已私有"在列表上一目了然。
  it('已私有时锁按钮 aria-pressed=true 且 amber 高亮常显', () => {
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
    expect(cls).toContain('text-amber-600')
    expect(cls).not.toContain('opacity-0')
  })

  /// 关键交互：点击锁按钮只能触发 toggle-private，**不能**冒泡到外层 button
  /// 触发 open 事件——否则用户每点一次锁就同时开 Drawer，违反「外部入口、
  /// 简单一点」的诉求。这个测试是该改动的核心断言。
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

  /// 键盘 a11y：因为外层是 button，内层 lock 用 span role=button + tabindex=0
  /// 才能避开 W3C 不允许 button 嵌 button 的限制。Enter / Space 必须仍然能触发
  /// toggle，且通过 .stop.prevent 阻止外层 button 的默认行为。
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
    // 直接点外层 button：用 root 触发更稳，jsdom 里点击 title 文本节点不会
    // 一定向上冒泡。
    await wrapper.trigger('click')
    expect(wrapper.emitted('open')).toBeTruthy()
  })
})
