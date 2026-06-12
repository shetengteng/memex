<script setup lang="ts">
import { computed, ref } from 'vue'
import { Badge } from '@/components/ui/badge'
import { Input } from '@/components/ui/input'
import IdeDot from '@/components/shell/IdeDot.vue'
import { ChevronRight, FolderGit2, Search } from 'lucide-vue-next'
import { ADAPTER_MAP, projects } from '@/stores/memex'

defineEmits<{ open: [string] }>()

const projectQuery = ref('')

// 先过滤掉空 path / 空 name 的项目行（后端如有 project_path === '' 的聚合
// 行漏到前端，会渲染成无 label 的卡片）。再在剩余集合上做搜索。
const validProjects = computed(() =>
  projects.filter((p) => {
    const path = p.path?.trim()
    if (!path || path === '/') return false
    return p.name.trim().length > 0
  }),
)

// 跟 LibraryFacets 的搜索框对齐：只匹配 project name + tags，不搜 path 全文。
// path 中间段（如 `~/.cursor/extensions/...`、`node_modules/src/...`）会让常见
// 关键词把无关项目全拉进来，是用户报「搜索项目名搜出来不太对」的根因。
const filteredProjects = computed(() => {
  const q = projectQuery.value.trim().toLowerCase()
  if (!q) return validProjects.value.slice()
  return validProjects.value.filter((p) =>
    `${p.name} ${p.tags.join(' ')}`.toLowerCase().includes(q),
  )
})

const totalProjectSessions = computed(() =>
  filteredProjects.value.reduce((acc, p) => acc + p.sessions, 0),
)

const tFmt = (iso: string) =>
  new Date(iso).toLocaleString('zh-CN', { dateStyle: 'short', timeStyle: 'short' })
</script>

<template>
  <div class="flex flex-1 min-h-0 flex-col overflow-hidden">
    <div class="flex shrink-0 items-center gap-2 px-5 pt-3 pb-2">
      <div class="relative flex-1">
        <Search
          class="pointer-events-none absolute left-3 top-1/2 size-3.5 -translate-y-1/2 text-muted-foreground"
        />
        <Input
          v-model="projectQuery"
          class="h-9 pl-9"
          placeholder="按项目名、路径或标签搜索…"
        />
      </div>
      <span class="hidden whitespace-nowrap text-[12px] text-muted-foreground md:inline">
        共
        <span class="font-medium text-foreground tabular-nums">{{ filteredProjects.length }}</span>
        个项目 ·
        <span class="font-medium text-foreground tabular-nums">
          {{ totalProjectSessions.toLocaleString() }}
        </span>
        个会话
      </span>
    </div>

    <div class="flex-1 min-h-0 overflow-y-auto px-5 pb-5">
      <div v-if="filteredProjects.length" class="grid gap-3 md:grid-cols-2">
        <button
          v-for="p in filteredProjects"
          :key="p.id"
          class="group rounded-xl border bg-card p-4 text-left transition hover:border-primary/40 hover:bg-accent/40 hover:shadow-sm"
          @click="$emit('open', p.path)"
        >
          <div class="mb-2 flex items-center justify-between">
            <div class="flex items-center gap-2">
              <FolderGit2 class="size-4 text-muted-foreground" />
              <span class="text-[14px] font-semibold">{{ p.name }}</span>
            </div>
            <Badge variant="secondary" class="tabular-nums">{{ p.sessions }} 个会话</Badge>
          </div>
          <div class="mb-3 truncate font-mono text-[10px] text-muted-foreground">{{ p.path }}</div>
          <div class="flex items-center gap-2 text-[11px] text-muted-foreground">
            <IdeDot :adapter="p.primaryAdapter" />
            主用 {{ ADAPTER_MAP[p.primaryAdapter]?.label ?? p.primaryAdapter }}
            <span>·</span>
            <span>最近活跃 {{ tFmt(p.lastActiveAt) }}</span>
          </div>
          <div class="mt-3 flex items-center justify-between">
            <div class="flex flex-wrap gap-1.5">
              <Badge v-for="t in p.tags" :key="t" variant="outline" class="text-[10px]">{{ t }}</Badge>
            </div>
            <span
              class="inline-flex items-center gap-1 text-[11px] text-primary opacity-0 transition group-hover:opacity-100"
            >
              查看会话
              <ChevronRight class="size-3" />
            </span>
          </div>
        </button>
      </div>
      <div v-else class="flex h-40 items-center justify-center">
        <div class="text-center">
          <FolderGit2 class="mx-auto size-8 text-muted-foreground/40" />
          <p class="mt-2 text-[12px] text-muted-foreground">没有匹配的项目</p>
        </div>
      </div>
    </div>
  </div>
</template>
