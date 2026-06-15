// @vitest-environment jsdom
//
// 默认 happy-dom 在解析 mixed text + `<script>` 时会把 script 当 text 节点扁平化，
// 导致 DOMPurify 看不到真正的 script 元素、剥不彻底；jsdom 的 DOM 解析行为与
// 生产 Tauri webview（macOS WKWebKit）一致，能可靠覆盖 sanitize 路径。
import { describe, expect, it } from 'vitest'
import { mount } from '@vue/test-utils'
import MarkdownContent from './MarkdownContent.vue'

// 修复点：之前 markdown-it 配置 `html: false`，help 文档里 `<kbd>⌘K</kbd>` 这种内联
// HTML 被直接丢弃，UI 上只剩"快捷键 打开命令面板"，看着像一段缺词。这一组测试
// 把"哪些片段必须保留 / 哪些必须剥"固化下来，避免后续误改回 html:false 或漏 sanitize。
describe('MarkdownContent / inline HTML & GFM extensions', () => {
  it('preserves <kbd> tags so keyboard shortcuts render correctly', () => {
    const wrapper = mount(MarkdownContent, {
      props: { content: '按 <kbd>⌘K</kbd> 打开命令面板' },
    })
    const html = wrapper.html()
    expect(html).toContain('<kbd>⌘K</kbd>')
  })

  it('preserves <mark> highlight tags', () => {
    const wrapper = mount(MarkdownContent, {
      props: { content: '已 <mark>命中</mark> 缓存' },
    })
    expect(wrapper.html()).toContain('<mark>命中</mark>')
  })

  it('renders GFM task lists with checkboxes (- [x] / - [ ])', () => {
    const wrapper = mount(MarkdownContent, {
      props: { content: '- [x] 完成项\n- [ ] 待办项' },
    })
    const html = wrapper.html()
    // markdown-it-task-lists 把每行 li 渲染成带 class="task-list-item" 的 li，
    // 其内有 <input type="checkbox">，已勾选的多一个 checked 属性。属性顺序由
    // plugin 决定，所以分两条断言（一条已勾选 + 一条未勾选）来固定行为。
    expect(html).toContain('class="contains-task-list"')
    const inputs = html.match(/<input[^>]*type="checkbox"[^>]*>/g) ?? []
    expect(inputs.length).toBe(2)
    expect(inputs.some((tag) => tag.includes('checked'))).toBe(true)
    expect(inputs.some((tag) => !tag.includes('checked'))).toBe(true)
  })

  it('still renders standard markdown (heading / list / code) without regressions', () => {
    const wrapper = mount(MarkdownContent, {
      props: {
        content: '# 标题\n\n- 项一\n- 项二\n\n```js\nconsole.log(1)\n```',
      },
    })
    const html = wrapper.html()
    expect(html).toContain('<h1>标题</h1>')
    expect(html).toContain('<li>项一</li>')
    expect(html).toContain('console.log(1)')
  })

  it('honors maxLen by truncating long input with ellipsis', () => {
    const wrapper = mount(MarkdownContent, {
      props: { content: 'a'.repeat(50), maxLen: 10 },
    })
    expect(wrapper.text()).toContain('…')
  })

  it('strips <script> tags injected through inline HTML to prevent XSS', () => {
    const wrapper = mount(MarkdownContent, {
      props: { content: '正文 <script>window.x=1</script> 结尾' },
    })
    const html = wrapper.html()
    expect(html).not.toContain('<script')
    expect(html).not.toContain('window.x=1')
  })

  it('strips dangerous on*= event handlers from inline HTML', () => {
    const wrapper = mount(MarkdownContent, {
      props: { content: '<img src=x onerror="alert(1)">' },
    })
    const html = wrapper.html()
    expect(html).not.toContain('onerror')
    expect(html).not.toContain('alert(1)')
  })
})
