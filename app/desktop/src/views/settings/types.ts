export type ProviderKind = 'openai_compat' | 'anthropic' | 'ollama'
export type ProviderStatus = 'ok' | 'error' | 'local' | 'untested'

export type Provider = {
  id: string
  name: string
  kind: ProviderKind | string
  baseUrl: string
  model: string
  apiKey: string
  enabled: boolean
  isDefault: boolean
  status: ProviderStatus | string
  latencyMs: number | null
  updatedAt: number
}
