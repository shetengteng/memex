import { describe, expect, it, vi } from 'vitest'
import { mount } from '@vue/test-utils'
import MessageContent from './MessageContent.vue'

// Stub markdown 渲染，避免 markdown-it 副作用，让我们能直接检查 segment 分割。
vi.mock('./MarkdownContent.vue', () => ({
  default: {
    name: 'MarkdownContent',
    props: ['content'],
    template: '<div class="md-stub">{{ content }}</div>',
  },
}))

// 同样 stub 工具卡片，只透传 name / args / result。
vi.mock('./ToolCallBlock.vue', () => ({
  default: {
    name: 'ToolCallBlock',
    props: ['name', 'args', 'result'],
    template:
      '<div class="tool-stub" :data-tool="name" :data-args="args" :data-result="result" />',
  },
}))

describe('MessageContent segmenting', () => {
  it('renders plain markdown as a single md-stub when content has no tool block', () => {
    const wrapper = mount(MessageContent, { props: { content: '这只是一段普通文字\n带换行' } })
    const stubs = wrapper.findAll('.md-stub')
    expect(stubs.length).toBe(1)
    expect(stubs[0]!.text()).toContain('这只是一段普通文字')
    expect(wrapper.findAll('.tool-stub').length).toBe(0)
  })

  it('splits a tool block out of surrounding markdown', () => {
    const content = [
      '我帮你读一下 HomePage.vue 里的 hero 段：',
      '',
      '[tool: read_file_v2]',
      'args: {"path":"/tmp/HomePage.vue","offset":160,"limit":40}',
      'result: {"contents":" <main></main>","totalLinesInFile":282}',
      '',
      '看上去问题在 hero-eyebrow 这块。',
    ].join('\n')

    const wrapper = mount(MessageContent, { props: { content } })

    // 应该有两段 markdown + 一段 tool
    const mds = wrapper.findAll('.md-stub')
    const tools = wrapper.findAll('.tool-stub')
    expect(tools.length).toBe(1)
    expect(mds.length).toBe(2)
    expect(mds[0]!.text()).toContain('我帮你读一下')
    expect(mds[1]!.text()).toContain('hero-eyebrow')

    const tool = tools[0]!
    expect(tool.attributes('data-tool')).toBe('read_file_v2')
    expect(tool.attributes('data-args')).toContain('"path":"/tmp/HomePage.vue"')
    expect(tool.attributes('data-result')).toContain('"totalLinesInFile":282')
  })

  it('handles a tool block with no args / no result without throwing', () => {
    const content = '[tool: bash]\n'
    const wrapper = mount(MessageContent, { props: { content } })
    expect(wrapper.findAll('.tool-stub').length).toBe(1)
    const tool = wrapper.find('.tool-stub')
    expect(tool.attributes('data-tool')).toBe('bash')
    // 空字符串属性会在 DOM 上变成 undefined / 空，因此不强校验 args/result
  })
})
