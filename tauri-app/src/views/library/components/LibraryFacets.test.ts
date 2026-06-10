import { describe, it, expect, beforeEach, vi } from 'vitest'
import { mount } from '@vue/test-utils'
import LibraryFacets from './LibraryFacets.vue'
import type { SummaryFilter, TimeFilter } from '../composables/sessionFilters'

vi.mock('@/stores/memex', () => ({
  adapters: [
    { id: 'cursor', label: 'Cursor', status: 'active', path: '~/x', sessions: 100 },
    { id: 'claude_code', label: 'Claude Code', status: 'active', path: '~/y', sessions: 50 },
  ],
  breakdownByAdapter: { cursor: 100, claude_code: 50 },
  // 12 个项目，触发 "+ N 更多" 按钮（PROJECT_DEFAULT_LIMIT = 8）。
  // path 字段各不相同 → disambiguation 后显示就是末段名 `proj-N`。
  projects: Array.from({ length: 12 }, (_, i) => ({
    id: `p${i}`,
    name: `proj-${i}`,
    path: `/workspace/proj-${i}`,
    sessions: 30 - i, // 倒序
    messages: 100,
    lastActive: '',
  })),
}))

const baseProps = {
  fAdapters: [] as string[],
  fProjects: [] as string[],
  fTime: '7d' as TimeFilter,
  fSummary: 'all' as SummaryFilter,
  activeFilterCount: 0,
  hasActiveFilters: false,
}

const stubs = {
  Badge: { template: '<span><slot/></span>' },
  Button: {
    template: '<button :disabled="disabled" @click="$emit(\'click\')"><slot/></button>',
    props: ['disabled', 'variant', 'size'],
    emits: ['click'],
  },
  Checkbox: {
    template:
      '<input type="checkbox" :checked="modelValue" @change="$emit(\'update:modelValue\', !modelValue)" />',
    props: ['modelValue'],
    emits: ['update:modelValue'],
  },
  Input: {
    template:
      '<input :type="type ?? \'text\'" :value="modelValue" :placeholder="placeholder" @input="$emit(\'update:modelValue\', $event.target.value)" />',
    props: ['modelValue', 'placeholder', 'type'],
    emits: ['update:modelValue'],
  },
  Label: { template: '<label><slot/></label>' },
  RadioGroup: {
    template: '<div><slot/></div>',
    props: ['modelValue'],
    emits: ['update:modelValue'],
  },
  RadioGroupItem: { template: '<input type="radio" />', props: ['value'] },
  Tooltip: { template: '<span><slot/></span>' },
  TooltipTrigger: { template: '<span><slot/></span>' },
  TooltipContent: { template: '<span><slot/></span>' },
  IdeDot: true,
}

describe('LibraryFacets', () => {
  beforeEach(() => {
    /* nothing */
  })

  it('默认只渲染前 8 个项目并显示展开按钮（每次 +10）', () => {
    const wrapper = mount(LibraryFacets, { props: baseProps, global: { stubs } })

    // 12 个项目；默认 limit=8 → 隐藏 4 个
    const html = wrapper.html()
    expect(html).toContain('proj-0')
    expect(html).toContain('proj-7')
    expect(html).not.toContain('proj-8')
    expect(html).toContain('+ 展开 4（剩 4）')
  })

  it('点击展开按钮后增量渲染下一批（一次 +10）', async () => {
    const wrapper = mount(LibraryFacets, { props: baseProps, global: { stubs } })
    const moreBtn = wrapper.findAll('button').find((b) => b.text().includes('展开'))
    expect(moreBtn).toBeTruthy()
    await moreBtn!.trigger('click')

    expect(wrapper.html()).toContain('proj-8')
    expect(wrapper.html()).toContain('proj-11')
    expect(wrapper.html()).toContain('收起')
  })

  it('搜索框过滤项目名（不区分大小写）', async () => {
    const wrapper = mount(LibraryFacets, { props: baseProps, global: { stubs } })
    const search = wrapper.find('input[type="search"]')
    await search.setValue('PROJ-1')

    // 命中：proj-1, proj-10, proj-11，但其他 proj-0/proj-2 等不应命中
    const html = wrapper.html()
    expect(html).toContain('proj-1')
    expect(html).toContain('proj-10')
    expect(html).toContain('proj-11')
    expect(html).not.toContain('proj-0<')
    expect(html).not.toContain('proj-2<')
  })

  it('搜索词无命中时显示空状态', async () => {
    const wrapper = mount(LibraryFacets, { props: baseProps, global: { stubs } })
    await wrapper.find('input[type="search"]').setValue('does-not-exist-xxxxx')
    expect(wrapper.text()).toContain('没有匹配的项目')
  })

  it('搜索只匹配 disambiguated 展示名，不被 path 中间段误伤', async () => {
    // 直接复现用户报的「搜索项目名搜出来不太对，会有其他的」：
    // 老实现把 p.path.toLowerCase().includes(q) 也算进 match，导致
    // 搜 "cursor"、"src" 这种 segment 把 ~/.cursor/extensions、
    // node_modules/src/* 这类"路径含关键字但项目名不含"的项目全拉进来。
    // 这里 mock 一个 path 含 "cursor" 但 displayName="alpha" 的项目，
    // 加一个 displayName="cursor-helper" 的项目，搜 "cursor" 必须只命中后者。
    vi.resetModules()
    vi.doMock('@/stores/memex', () => ({
      adapters: [],
      breakdownByAdapter: {},
      projects: [
        {
          id: 'p-alpha',
          name: 'alpha',
          // path 中间段含 "cursor"——老实现会被搜到，新实现必须屏蔽
          path: '/Users/me/.cursor/extensions/alpha',
          sessions: 5,
          messages: 0,
          lastActive: '',
        },
        {
          id: 'p-real',
          name: 'cursor-helper',
          path: '/Users/me/work/cursor-helper',
          sessions: 3,
          messages: 0,
          lastActive: '',
        },
      ],
    }))
    const { default: LibraryFacetsReloaded } = await import('./LibraryFacets.vue')

    const wrapper = mount(LibraryFacetsReloaded, { props: baseProps, global: { stubs } })
    await wrapper.find('input[type="search"]').setValue('cursor')

    const html = wrapper.html()
    expect(html).toContain('cursor-helper')
    expect(html).not.toContain('alpha')

    vi.doUnmock('@/stores/memex')
  })

  it('点击工具区"全选"emit update:fAdapters 全部 id', async () => {
    const wrapper = mount(LibraryFacets, { props: baseProps, global: { stubs } })
    const selectAllBtn = wrapper.findAll('button').find((b) => b.text() === '全选')
    expect(selectAllBtn).toBeTruthy()
    await selectAllBtn!.trigger('click')

    const events = wrapper.emitted('update:fAdapters')
    expect(events).toBeTruthy()
    expect(events![0][0]).toEqual(['cursor', 'claude_code'])
  })

  it('count=0 的 adapter 不渲染（用户只用 cursor 时不应看到 codex/aider 等空行）', async () => {
    // 复现「工具后面有个数为 0」：adapters 数组是 7 个静态条目，
    // breakdownByAdapter 只对真正有数据的 adapter 写值，老实现把所有 7 行
    // 都渲染出来 + 右侧 ?? 0。新实现只渲染 count > 0 的 adapter；
    // "全选"也只覆盖可见集合，不会把 0 计数 adapter 误塞进 fAdapters。
    vi.resetModules()
    vi.doMock('@/stores/memex', () => ({
      adapters: [
        { id: 'cursor', label: 'Cursor', status: 'active', path: '~/x', sessions: 100 },
        { id: 'codex', label: 'Codex', status: 'disabled', path: '~/y', sessions: 0 },
      ],
      breakdownByAdapter: { cursor: 100 },
      projects: [],
    }))
    const { default: Reloaded } = await import('./LibraryFacets.vue')

    const wrapper = mount(Reloaded, { props: baseProps, global: { stubs } })
    const html = wrapper.html()
    expect(html).toContain('Cursor')
    expect(html).not.toContain('Codex')

    const selectAllBtn = wrapper.findAll('button').find((b) => b.text() === '全选')
    await selectAllBtn!.trigger('click')
    const events = wrapper.emitted('update:fAdapters')
    expect(events![0][0]).toEqual(['cursor'])

    vi.doUnmock('@/stores/memex')
  })

  it('空 path / 空展示名的项目不渲染（防御后端聚合出 project_path=== "" 的幽灵行）', async () => {
    // 复现「资料库左侧很多展示的过滤条件值空」：后端如果聚合出
    // project_path === '' 的 ProjectSummary，老实现会渲染出
    // checkbox + 空白 label + 数字的无意义行，且勾选后 fProjects 落入空字符串、
    // 过滤完全沉默。前端在 sortedProjects 阶段统一拦截。
    vi.resetModules()
    vi.doMock('@/stores/memex', () => ({
      adapters: [],
      breakdownByAdapter: {},
      projects: [
        { id: 'p-real', name: 'real-project', path: '/Users/me/work/real', sessions: 5, messages: 0, lastActive: '' },
        { id: 'p-empty', name: '', path: '', sessions: 3, messages: 0, lastActive: '' },
        { id: 'p-root', name: '', path: '/', sessions: 1, messages: 0, lastActive: '' },
      ],
    }))
    const { default: Reloaded } = await import('./LibraryFacets.vue')

    const wrapper = mount(Reloaded, { props: baseProps, global: { stubs } })
    const html = wrapper.html()
    expect(html).toContain('real')
    // 空白 label 行不应出现：用项目区第一个 checkbox 来判断
    const checkboxes = wrapper.findAll('input[type="checkbox"]')
    // adapter 区为空（adapters: []），项目区只有 1 个合法项目 → 共 1 个 checkbox
    expect(checkboxes.length).toBe(1)

    vi.doUnmock('@/stores/memex')
  })

  it('已全选时点击 "全清" emit update:fAdapters 空数组', async () => {
    const wrapper = mount(LibraryFacets, {
      props: { ...baseProps, fAdapters: ['cursor', 'claude_code'] },
      global: { stubs },
    })
    const clearBtn = wrapper.findAll('button').find((b) => b.text() === '全清')
    expect(clearBtn).toBeTruthy()
    await clearBtn!.trigger('click')

    const events = wrapper.emitted('update:fAdapters')
    expect(events![0][0]).toEqual([])
  })

  it('点击项目区"全选"emit update:fProjects 当前可见项（用完整 path）', async () => {
    const wrapper = mount(LibraryFacets, { props: baseProps, global: { stubs } })
    const selectAllBtns = wrapper.findAll('button').filter((b) => b.text() === '全选')
    // 第二个"全选"是项目区的
    const projectSelectAll = selectAllBtns[1]
    expect(projectSelectAll).toBeTruthy()
    await projectSelectAll!.trigger('click')

    const events = wrapper.emitted('update:fProjects')
    expect(events).toBeTruthy()
    // 默认只 visible 8 个；emit 出来的应是 path 而非 name
    expect((events![0][0] as string[]).length).toBe(8)
    expect(events![0][0]).toContain('/workspace/proj-0')
    expect(events![0][0]).toContain('/workspace/proj-7')
    expect(events![0][0]).not.toContain('proj-0')
  })

  it('勾选单个项目 checkbox emit toggleProject 带完整 path', async () => {
    // 这条用例直接对应用户报的「点击项目名没反应」：
    // checkbox @update:model-value 必须 emit toggleProject(p.path)，
    // 父组件 library/index.vue 才能把 path 压入 fProjects → 触发 reload。
    // 任何回归（如错传 p.name / 错传 boolean）都会让查询沉默失败。
    const wrapper = mount(LibraryFacets, { props: baseProps, global: { stubs } })
    const checkboxes = wrapper.findAll('input[type="checkbox"]')
    // 前 2 个 checkbox 属于 adapter 区（cursor / claude_code），项目区从 index=2 开始
    const firstProjectCheckbox = checkboxes[2]
    expect(firstProjectCheckbox).toBeTruthy()
    await firstProjectCheckbox.trigger('change')

    const events = wrapper.emitted('toggleProject')
    expect(events).toBeTruthy()
    expect(events!.length).toBe(1)
    // payload 必须是完整 path，不能退化成 name（proj-0）或 boolean
    expect(events![0][0]).toBe('/workspace/proj-0')
  })

  it('再次勾选已选中项目 emit toggleProject 同一个 path（父组件负责反选）', async () => {
    const wrapper = mount(LibraryFacets, {
      props: { ...baseProps, fProjects: ['/workspace/proj-0'] },
      global: { stubs },
    })
    const checkboxes = wrapper.findAll('input[type="checkbox"]')
    // 已选中态下，checkbox 应该是 checked=true
    const firstProjectCheckbox = checkboxes[2]
    expect((firstProjectCheckbox.element as HTMLInputElement).checked).toBe(true)
    await firstProjectCheckbox.trigger('change')

    const events = wrapper.emitted('toggleProject')
    expect(events).toBeTruthy()
    // 反选时 emit 出来的还是 path，由父组件的 toggleProject 决定增删
    expect(events![0][0]).toBe('/workspace/proj-0')
  })
})
