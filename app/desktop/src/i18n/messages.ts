// i18n 字典入口。
//
// **真正的文案在 ./zh.ts 与 ./en.ts**，本文件只组装并对外暴露 MESSAGES 表 +
// 一些枚举。新增字符串只需在两个语言文件里同时加同名 key，组件用 useI18n() 即可访问。
// 拆分原因：之前 messages.ts 单文件 2500+ 行，CR / 冲突 / 翻译协作都不友好。

import { zh } from './zh'
import { en } from './en'
import type { Locale, Messages } from './types'

export type { Locale, Messages } from './types'

export const LOCALE_OPTIONS: { value: Locale; label: string }[] = [
  { value: 'zh', label: '简体中文' },
  { value: 'en', label: 'English' },
]

export const DEFAULT_LOCALE: Locale = 'zh'

export const MESSAGES: Record<Locale, Messages> = { zh, en }
