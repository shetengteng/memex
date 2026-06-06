export type Provider = {
  id: string
  name: string
  kind: 'openai_compat' | 'anthropic' | 'ollama'
  baseUrl: string
  model: string
  apiKey: string
  enabled: boolean
  isDefault: boolean
  status: 'ok' | 'error' | 'local' | 'untested'
  latencyMs: number | null
  updatedAt: number
}
