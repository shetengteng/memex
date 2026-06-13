// i18n 共享类型定义。
//
// 拆分 zh.ts / en.ts 之后，两个 dict 都需要同一个 Messages 形状；放在
// 单独文件里避免循环依赖（messages.ts <-> zh.ts <-> messages.ts）。

export type Locale = 'zh' | 'en'

export type Messages = Record<string, string>
