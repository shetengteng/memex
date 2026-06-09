import { describe, expect, it } from 'vitest'
import {
  buildDisambiguatedNames,
  type DisambiguableProject,
} from './projectDisambiguation'

const mk = (id: string, path: string): DisambiguableProject => ({
  id,
  name: path.split('/').filter(Boolean).pop() ?? path,
  path,
})

describe('buildDisambiguatedNames', () => {
  it('末段唯一时显示末段名', () => {
    const ps = [
      mk('a', '/Users/me/repo/memex'),
      mk('b', '/Users/me/repo/yuan-notes'),
    ]
    const m = buildDisambiguatedNames(ps)
    expect(m.get('a')).toBe('memex')
    expect(m.get('b')).toBe('yuan-notes')
  })

  it('末段冲突时自动加一段父目录（最短消歧）', () => {
    const ps = [
      mk('a', '/Users/me/tt-demo/src'),
      mk('b', '/Users/me/repo/metadata-server/src'),
      mk('c', '/Users/me/repo/yuan-notes/frontend/src'),
    ]
    const m = buildDisambiguatedNames(ps)
    expect(m.get('a')).toBe('tt-demo/src')
    expect(m.get('b')).toBe('metadata-server/src')
    expect(m.get('c')).toBe('frontend/src')
  })

  it('父目录同名时继续加深直到唯一', () => {
    const ps = [
      mk('a', '/repo/foo/src'),
      mk('b', '/other/foo/src'),
    ]
    const m = buildDisambiguatedNames(ps)
    expect(m.get('a')).toBe('repo/foo/src')
    expect(m.get('b')).toBe('other/foo/src')
  })

  it('完全相同 path 也能输出（退化到完整 path）', () => {
    const ps = [mk('a', '/repo/x'), mk('b', '/repo/x')]
    const m = buildDisambiguatedNames(ps)
    expect(m.get('a')).toBe('/repo/x')
    expect(m.get('b')).toBe('/repo/x')
  })

  it('空 path 退化到 name', () => {
    const ps: DisambiguableProject[] = [
      { id: 'a', name: 'orphan', path: '' },
    ]
    const m = buildDisambiguatedNames(ps)
    expect(m.get('a')).toBe('orphan')
  })

  it('单一项目时直接显示末段', () => {
    const m = buildDisambiguatedNames([mk('a', '/repo/foo/bar')])
    expect(m.get('a')).toBe('bar')
  })
})
