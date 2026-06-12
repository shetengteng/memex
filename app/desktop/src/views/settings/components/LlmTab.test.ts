import { describe, it, expect, vi, beforeEach } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import LlmTab from './LlmTab.vue'

const ipcMocks = vi.hoisted(() => ({
  llmProviderList: vi.fn(),
  llmProviderUpsert: vi.fn(),
  llmProviderDelete: vi.fn(),
  llmProviderTest: vi.fn(),
  getConfig: vi.fn().mockResolvedValue(null),
  setConfig: vi.fn().mockResolvedValue(undefined),
}))

vi.mock('@/composables/useMemex', () => ({
  useMemex: () => ipcMocks,
}))

vi.mock('vue-sonner', () => ({
  toast: { success: vi.fn(), error: vi.fn(), message: vi.fn() },
}))

// 简化的 dialog 直接返回不渲染
vi.mock('./ProviderEditDialog.vue', () => ({
  default: { template: '<div></div>', props: ['open', 'editing'] },
}))

describe('LlmTab', () => {
  beforeEach(() => {
    Object.values(ipcMocks).forEach((fn) => fn.mockClear?.())
    ipcMocks.llmProviderList.mockResolvedValue([
      {
        id: 'p-1',
        name: 'Ollama Local',
        kind: 'ollama',
        baseUrl: 'http://127.0.0.1:11434',
        model: 'qwen2.5:7b',
        apiKey: '',
        enabled: true,
        isDefault: true,
        status: 'local',
        latencyMs: 320,
        updatedAt: '2026-06-07T03:00:00Z',
      },
    ])
    ipcMocks.llmProviderUpsert.mockImplementation(async (p: any) => ({
      id: p.id,
      name: p.name,
      kind: p.kind,
      baseUrl: p.baseUrl,
      model: p.model,
      apiKey: p.apiKey,
      enabled: p.enabled,
      isDefault: p.isDefault,
      status: 'untested',
      latencyMs: null,
      updatedAt: '2026-06-07T03:00:00Z',
    }))
    ipcMocks.llmProviderTest.mockResolvedValue({
      ok: true,
      latencyMs: 420,
      error: null,
      responseText: 'pong',
    })
  })

  const stubs = {
    Card: { template: '<div><slot/></div>' },
    CardContent: { template: '<div><slot/></div>' },
    CardDescription: { template: '<span><slot/></span>' },
    CardFooter: { template: '<div><slot/></div>' },
    CardHeader: { template: '<div><slot/></div>' },
    CardTitle: { template: '<div><slot/></div>' },
    Badge: { template: '<span><slot/></span>' },
    Switch: {
      template: '<input type="checkbox" />',
      props: ['modelValue', 'disabled'],
      emits: ['update:modelValue'],
    },
    Textarea: {
      template: '<textarea :value="modelValue" @input="$emit(\'update:modelValue\', $event.target.value)" />',
      props: ['modelValue'],
      emits: ['update:modelValue'],
    },
    Tooltip: { template: '<span><slot/></span>' },
    TooltipContent: { template: '<span><slot/></span>' },
    TooltipTrigger: { template: '<span><slot/></span>' },
    Button: {
      template: '<button :disabled="disabled" @click="$emit(\'click\')"><slot/></button>',
      props: ['disabled'],
      emits: ['click'],
    },
  }

  it('loads providers and prompt template on mount', async () => {
    const wrapper = mount(LlmTab, { global: { stubs } })
    await flushPromises()
    expect(ipcMocks.llmProviderList).toHaveBeenCalledOnce()
    expect(ipcMocks.getConfig).toHaveBeenCalledWith('llm.prompt_template')
    expect(wrapper.text()).toContain('Ollama Local')
    expect(wrapper.text()).toContain('共 1 个，已启用 1')
  })

  it('saves prompt template via setConfig', async () => {
    const wrapper = mount(LlmTab, { global: { stubs } })
    await flushPromises()
    const buttons = wrapper.findAll('button')
    // 「保存模板」按钮是最后一个
    const save = buttons[buttons.length - 1]
    await save.trigger('click')
    await flushPromises()
    expect(ipcMocks.setConfig).toHaveBeenCalledWith('llm.prompt_template', expect.any(String))
  })
})
