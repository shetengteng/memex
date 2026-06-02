// 集中管理 UI 文案的中英文字典。
// 新增字符串时只在这里加 key，不在组件里写裸字符串。
// key 命名约定：<area>.<sub>.<noun>，如 settings.adapters.title。

export type Locale = 'zh' | 'en'

export const LOCALE_OPTIONS: { value: Locale; label: string }[] = [
  { value: 'zh', label: '简体中文' },
  { value: 'en', label: 'English' },
]

export const DEFAULT_LOCALE: Locale = 'zh'

type Messages = Record<string, string>

const zh: Messages = {
  // 通用
  'common.refresh': '刷新',
  'common.refreshing': '刷新中',
  'common.loading': '加载中…',
  'common.copy': '复制',
  'common.copied': '已复制',
  'common.back': '返回',
  'common.cancel': '取消',
  'common.save': '保存',
  'common.enabled': '已启用',
  'common.disabled': '已禁用',
  'common.running': '运行中',
  'common.offline': '未运行',
  'common.starting': '启动中',
  'common.ready': '就绪',

  // popup 底部导航
  'nav.home': '主页',
  'nav.search': '搜索',
  'nav.settings': '设置',
  'nav.status': '状态',
  'nav.dashboard': '打开 Dashboard',

  // 主页 / 搜索
  'search.placeholder': '在所有会话中搜索…',
  'search.empty': '暂无最近会话',
  'search.no_results': '没有命中结果',

  // 设置
  'settings.title': '设置',
  'settings.section.adapters': '适配器',
  'settings.section.llm': 'LLM',
  'settings.section.privacy': '隐私',
  'settings.section.general': '通用',
  'settings.adapters.local': '本地',
  'settings.adapters.offline': '离线',
  'settings.llm.ollama_label': 'Ollama（{model}）',
  'settings.llm.claude_fallback': 'Claude 兜底',
  'settings.privacy.auto_redact': '自动脱敏',
  'settings.privacy.hide_private': 'Private session 隐藏',
  'settings.general.language': '界面语言',
  'settings.general.language_hint': '切换 popup 与 dashboard 内的语言',

  // 状态页
  'status.title': '系统状态',
  'status.system': '系统',
  'status.adapters': '适配器',
  'status.llm': 'LLM',
  'status.daemon.label': '后台服务',
  'status.daemon.checking': '检查中…',
  'status.daemon.starting_hint': 'PID {pid}，等待 HTTP 响应…',
  'status.daemon.offline_hint': '点击右侧"重启"启动 memex-daemon',
  'status.db.label': '数据库',
  'status.db.not_initialized': '未初始化',
  'status.db.hint_summary': '{sessions} 个会话 · {messages} 条消息',
  'status.index.label': '索引',
  'status.index.value': '{count} 个 chunk',
  'status.index.fts_ready': 'FTS5 已就绪',
  'status.index.empty': '尚未生成索引',
  'status.restart.button': '重启',
  'status.restart.in_progress': '启动中',
  'status.restart.fail': '重启失败',
  'status.llm.active': '当前提供方',
  'status.llm.active_none': '未启用',
  'status.llm.active_hint': '{sessions} 个会话 · {chunks} 个 chunk 摘要',
  'status.llm.paused': '摘要功能已暂停',
  'status.llm.cloud': '云端兜底',
  'status.llm.cloud_on': '已开启',
  'status.llm.cloud_off': '关闭',
  'status.llm.cloud_hint': '发送前会做脱敏',

  // Dashboard 通用
  'dashboard.title': 'Dashboard',
  'dashboard.subtitle': '跨 {count} 个项目的本地 AI 会话记忆',
  'dashboard.nav.overview': '总览',
  'dashboard.nav.sessions': '会话',
  'dashboard.nav.projects': '项目',
  'dashboard.nav.reports': '报告',
  'dashboard.nav.search': '搜索',

  // Reports
  'reports.title': '报告',
  'reports.subtitle': '基于 L2 会话摘要自动生成的日报和周报。',
  'reports.tab.daily': '日报',
  'reports.tab.weekly': '周报',
  'reports.regenerate.daily': '重新生成日报',
  'reports.regenerate.weekly': '重新生成周报',
  'reports.regenerate.in_progress': '生成中…',
  'reports.empty.daily': '还没有日报',
  'reports.empty.weekly': '还没有周报',
  'reports.empty.hint': '当 LLM 服务可用且 {scope}内至少有 {min} 个会话时，会在每次 ingest 时自动生成。可在<em>设置</em>里启用 Ollama 或配置 Claude API Key，然后运行 <code>memex ingest</code>，或点击右上角"重新生成"按钮立即触发。',
  'reports.section.topics': '主题',
  'reports.section.decisions': '关键决策',
  'reports.session_count': '涵盖 {count} 个会话',
  'reports.generated_at': '生成于 {time}',

  // Session 详情
  'session.back': '返回会话列表',
  'session.not_found': '会话未找到',
  'session.summary': '摘要',
  'session.summary.empty': '尚未生成摘要。',
  'session.summary.regenerate': '重新生成',
  'session.summary.generate': '生成摘要',
  'session.summary.generating': '生成中…',
  'session.summary.success': '摘要生成成功',
  'session.summary.fail_short': '至少需要 2 条消息才能生成摘要',
  'session.summary.calling': '正在调用 LLM，可能需要几秒…',
  'session.kpi.messages': '消息',
  'session.kpi.messages_unit': '{count} 条',
  'session.kpi.updated': '最近更新',
  'session.kpi.path': '路径',
  'session.role.user': '用户',
  'session.role.assistant': '助手',
  'session.messages': '消息',
  'session.load_more': '加载更多（还剩 {count} 条）',
}

const en: Messages = {
  // common
  'common.refresh': 'Refresh',
  'common.refreshing': 'Refreshing',
  'common.loading': 'Loading…',
  'common.copy': 'Copy',
  'common.copied': 'Copied',
  'common.back': 'Back',
  'common.cancel': 'Cancel',
  'common.save': 'Save',
  'common.enabled': 'Enabled',
  'common.disabled': 'Disabled',
  'common.running': 'Running',
  'common.offline': 'Offline',
  'common.starting': 'Starting',
  'common.ready': 'Ready',

  // popup nav
  'nav.home': 'Home',
  'nav.search': 'Search',
  'nav.settings': 'Settings',
  'nav.status': 'Status',
  'nav.dashboard': 'Open Dashboard',

  // home / search
  'search.placeholder': 'Search across all sessions…',
  'search.empty': 'No recent sessions yet',
  'search.no_results': 'No results',

  // settings
  'settings.title': 'Settings',
  'settings.section.adapters': 'Adapters',
  'settings.section.llm': 'LLM',
  'settings.section.privacy': 'Privacy',
  'settings.section.general': 'General',
  'settings.adapters.local': 'local',
  'settings.adapters.offline': 'offline',
  'settings.llm.ollama_label': 'Ollama ({model})',
  'settings.llm.claude_fallback': 'Claude fallback',
  'settings.privacy.auto_redact': 'Auto redact',
  'settings.privacy.hide_private': 'Hide private sessions',
  'settings.general.language': 'Language',
  'settings.general.language_hint': 'Switch the popup and dashboard language',

  // status
  'status.title': 'Health',
  'status.system': 'System',
  'status.adapters': 'Adapters',
  'status.llm': 'LLM',
  'status.daemon.label': 'Daemon',
  'status.daemon.checking': 'checking…',
  'status.daemon.starting_hint': 'PID {pid}, waiting for HTTP…',
  'status.daemon.offline_hint': 'Click Restart to start memex-daemon',
  'status.db.label': 'Database',
  'status.db.not_initialized': 'not initialized',
  'status.db.hint_summary': '{sessions} sessions · {messages} messages',
  'status.index.label': 'Index',
  'status.index.value': '{count} chunks',
  'status.index.fts_ready': 'FTS5 ready',
  'status.index.empty': 'no chunks yet',
  'status.restart.button': 'Restart',
  'status.restart.in_progress': 'Starting',
  'status.restart.fail': 'Restart failed',
  'status.llm.active': 'Active provider',
  'status.llm.active_none': 'none',
  'status.llm.active_hint': '{sessions} sessions · {chunks} chunk summaries',
  'status.llm.paused': 'summaries paused',
  'status.llm.cloud': 'Cloud fallback',
  'status.llm.cloud_on': 'opt-in',
  'status.llm.cloud_off': 'off',
  'status.llm.cloud_hint': 'redacted before send',

  // dashboard nav
  'dashboard.title': 'Dashboard',
  'dashboard.subtitle': 'Local AI session memory across {count} projects',
  'dashboard.nav.overview': 'Overview',
  'dashboard.nav.sessions': 'Sessions',
  'dashboard.nav.projects': 'Projects',
  'dashboard.nav.reports': 'Reports',
  'dashboard.nav.search': 'Search',

  // reports
  'reports.title': 'Reports',
  'reports.subtitle': 'Automatic daily and weekly digests built from L2 session summaries.',
  'reports.tab.daily': 'Daily',
  'reports.tab.weekly': 'Weekly',
  'reports.regenerate.daily': 'Regenerate daily',
  'reports.regenerate.weekly': 'Regenerate weekly',
  'reports.regenerate.in_progress': 'Generating…',
  'reports.empty.daily': 'No daily reports yet',
  'reports.empty.weekly': 'No weekly reports yet',
  'reports.empty.hint': 'Reports are generated when an LLM provider is active and there are at least {min} sessions in {scope}. Enable Ollama in <em>Settings</em> or set a Claude API key, then run <code>memex ingest</code>, or click the regenerate button above.',
  'reports.section.topics': 'Topics',
  'reports.section.decisions': 'Decisions',
  'reports.session_count': '{count} sessions',
  'reports.generated_at': 'generated {time}',

  // session detail
  'session.back': 'Back to sessions',
  'session.not_found': 'Session not found',
  'session.summary': 'Summary',
  'session.summary.empty': 'No summary yet.',
  'session.summary.regenerate': 'Regenerate',
  'session.summary.generate': 'Generate',
  'session.summary.generating': 'Generating…',
  'session.summary.success': 'Summary generated',
  'session.summary.fail_short': 'Session needs at least 2 messages',
  'session.summary.calling': 'Calling LLM, this may take a few seconds…',
  'session.kpi.messages': 'Messages',
  'session.kpi.messages_unit': '{count}',
  'session.kpi.updated': 'Updated',
  'session.kpi.path': 'Path',
  'session.role.user': 'User',
  'session.role.assistant': 'Assistant',
  'session.messages': 'Messages',
  'session.load_more': 'Load more ({count} remaining)',
}

export const MESSAGES: Record<Locale, Messages> = { zh, en }
