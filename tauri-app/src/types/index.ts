export interface Stats {
  sessions: number
  messages: number
  chunks: number
  db_exists: boolean
}

export interface SessionRow {
  id: string
  source: string
  project_path: string | null
  message_count: number
  updated_at: string
}

export interface SearchResult {
  chunk_id: number
  session_id: string
  message_id: string
  chunk_type: string
  content: string
  snippet: string
  rank: number
  match_reason: string
  adapter?: string
  project?: string
  timestamp?: string
}

export interface SessionDetail {
  id: string
  source: string
  project_path: string | null
  file_path: string
  message_count: number
  created_at: string
  updated_at: string
  messages: MessageRow[]
}

export interface MessageRow {
  id: string
  role: string
  content: string
  timestamp: string | null
}

export type ViewName = 'home' | 'search' | 'session' | 'settings'
