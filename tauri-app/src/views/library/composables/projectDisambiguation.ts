/**
 * 项目展示名去歧义：对同末段路径自动加最短父目录前缀，让 facet 里
 * 多行 `src` / `frontend` 等同名末段能被用户区分开。
 *
 * 设计：
 *   * 输入是项目全集（reactive 数组也可），输出是 `id -> 展示名` Map
 *   * 单个项目从倒数 1 段开始尝试，逐步加深，直到当前末段路径在
 *     全集里**唯一**，最坏情况退化到完整 path
 *   * 复杂度 O(n²·d)（n=项目数，d=path 段数）；几百项目内不必引 Trie
 *   * 纯函数 + 不修改输入；调用方可在 computed 里复用
 *
 * 为什么不能简单按 `path.split('/').pop()`：
 *   用户机器上同时有 `/Users/me/tt-demo/src`、
 *   `/Users/me/repo/metadata-server/src` 等多个 src 时，
 *   facet 会出现 N 行外观一样的 "src"——计数能对、过滤也能选，
 *   但用户认不出哪一行对应哪一个真实项目。
 */
export interface DisambiguableProject {
  id: string
  name: string
  path: string
}

export function buildDisambiguatedNames(
  all: readonly DisambiguableProject[],
): Map<string, string> {
  const segCache = new Map<string, string[]>()
  for (const p of all) segCache.set(p.id, p.path.split('/').filter(Boolean))

  const result = new Map<string, string>()
  for (const target of all) {
    const targetSegs = segCache.get(target.id) ?? []
    if (!targetSegs.length) {
      result.set(target.id, target.name)
      continue
    }
    let assigned = false
    for (let depth = 1; depth <= targetSegs.length; depth += 1) {
      const candidate = targetSegs.slice(-depth).join('/')
      const conflict = all.some((p) => {
        if (p.id === target.id) return false
        const segs = segCache.get(p.id) ?? []
        return segs.slice(-depth).join('/') === candidate
      })
      if (conflict) continue
      result.set(target.id, candidate)
      assigned = true
      break
    }
    if (!assigned) result.set(target.id, target.path)
  }
  return result
}
