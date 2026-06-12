/**
 * 「工具使用」卡片的纯计算逻辑 —— 把后端 `WorkloadReport.by_adapter`
 * 转成模板渲染需要的两类百分比：
 *
 * 1. `widthPct` —— **相对最大那一项**的百分比（0..100）。bar 视觉宽度用，
 *    最大那条铺满 100%，其它按比例缩短，便于一眼看出"哪个 IDE 明显多"。
 * 2. `sharePct` —— **相对所有 IDE 会话总和**的百分比（0..100，rounding 后
 *    总和约等于 100）。文本数字用，符合用户对"工具使用占比"的直觉
 *    （旧逻辑把 widthPct 当数字打成 `100%` 让人误以为"占了全部"）。
 *
 * 之所以拆成 pure function：组件里写 inline computed 没法单测，且双语义
 * 百分比的边界（空数组 / 单 IDE / count=0 全部为 0）容易回归。
 */
export interface AdapterUsageRow {
  id: string
  label: string
  count: number
  /** bar 宽度 0..100：相对最大项 */
  widthPct: number
  /** 文本百分比 0..100：相对全部 IDE 会话总和 */
  sharePct: number
}

export interface AdapterUsageInput {
  key: string
  sessions: number
}

const DEFAULT_TOP_N = 8

export function buildAdapterUsage(
  rows: readonly AdapterUsageInput[],
  labelOf: (id: string) => string,
  topN: number = DEFAULT_TOP_N,
): AdapterUsageRow[] {
  const positive = rows.filter((x) => x.sessions > 0)
  if (positive.length === 0) return []

  const max = Math.max(1, ...positive.map((x) => x.sessions))
  const total = positive.reduce((acc, x) => acc + x.sessions, 0)
  // total === 0 已被前一条过滤掉（positive 里全 > 0），但保留兜底避免 NaN
  const safeTotal = total > 0 ? total : 1

  return positive
    .map((x) => ({
      id: x.key,
      label: labelOf(x.key),
      count: x.sessions,
      widthPct: Math.round((x.sessions / max) * 100),
      sharePct: Math.round((x.sessions / safeTotal) * 100),
    }))
    .sort((a, b) => b.count - a.count)
    .slice(0, topN)
}
