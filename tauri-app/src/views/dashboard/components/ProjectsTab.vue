<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { Loader2, FolderOpen } from 'lucide-vue-next'
import type { ProjectSummary } from '@/types'
import { useMemex } from '@/composables/useMemex'
import { timeAgo, adapterColor, adapterBg, adapterLabel } from '@/lib/utils'
import { Input } from '@/components/ui/input'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'

const emit = defineEmits<{
  openSession: [sessionId: string]
  filterSessions: [projectName: string]
}>()

const { listProjects } = useMemex()

const projects = ref<ProjectSummary[]>([])
const loading = ref(true)
const search = ref('')
const filterAdapter = ref<string>('all')

const allAdapters = computed(() => {
  const set = new Set<string>()
  for (const p of projects.value) {
    for (const name of Object.keys(p.by_adapter)) set.add(name)
  }
  return Array.from(set).sort()
})

const filteredProjects = computed(() => {
  let list = projects.value

  if (filterAdapter.value !== 'all') {
    list = list.filter(p => p.by_adapter[filterAdapter.value])
  }

  if (search.value) {
    const q = search.value.toLowerCase()
    list = list.filter(p =>
      p.name.toLowerCase().includes(q) || p.project_path.toLowerCase().includes(q),
    )
  }

  return list
})

async function loadProjects() {
  loading.value = true
  try { projects.value = await listProjects() } catch { /* ignore */ }
  loading.value = false
}

function truncate(s: string | null, max: number): string {
  if (!s) return ''
  return s.length > max ? s.slice(0, max) + '...' : s
}

onMounted(loadProjects)
</script>

<template>
  <h1 class="mb-5 text-xl font-bold tracking-tight">Projects</h1>
  <div class="mb-4 flex flex-wrap items-center gap-2">
    <Input
      v-model="search"
      placeholder="Search projects..."
      class="min-w-[200px] flex-1 text-xs"
    />
    <Select v-model="filterAdapter">
      <SelectTrigger class="w-[140px] text-xs">
        <SelectValue placeholder="All Tools" />
      </SelectTrigger>
      <SelectContent>
        <SelectItem value="all">All Tools</SelectItem>
        <SelectItem v-for="a in allAdapters" :key="a" :value="a">{{ adapterLabel(a) }}</SelectItem>
      </SelectContent>
    </Select>
  </div>

  <div v-if="loading" class="flex items-center justify-center py-16">
    <Loader2 class="h-5 w-5 animate-spin text-muted-foreground" />
  </div>
  <div v-else-if="filteredProjects.length === 0" class="flex flex-col items-center justify-center gap-2 py-16 text-muted-foreground">
    <FolderOpen class="h-10 w-10 opacity-40" />
    <span class="text-sm">No projects found</span>
  </div>
  <div v-else class="grid gap-4" style="grid-template-columns: repeat(auto-fill, minmax(340px, 1fr));">
    <div
      v-for="p in filteredProjects"
      :key="p.project_path"
      class="cursor-pointer rounded-lg border border-border bg-card p-4 transition-all hover:border-primary/40 hover:bg-accent/30"
      @click="emit('filterSessions', p.name)"
    >
      <h3 class="text-[15px] font-semibold leading-tight">{{ p.name }}</h3>
      <div class="mb-2 mt-1 text-xs text-muted-foreground">
        {{ p.project_path }} &middot; {{ p.session_count }} sessions &middot; {{ timeAgo(p.last_updated) }}
      </div>
      <div v-if="p.last_title" class="mb-2 text-xs leading-relaxed text-muted-foreground">
        {{ truncate(p.last_title, 120) }}
      </div>
      <div class="flex flex-wrap items-center gap-1.5">
        <span
          v-for="[name, count] in Object.entries(p.by_adapter).sort((a, b) => b[1] - a[1])"
          :key="name"
          class="inline-flex items-center rounded-full px-2 py-0.5 text-[10px] font-semibold"
          :class="[adapterBg(name), adapterColor(name)]"
        >
          {{ adapterLabel(name) }} {{ count }}
        </span>
      </div>
    </div>
  </div>
  <div class="mt-3 text-center text-xs text-muted-foreground">
    {{ filteredProjects.length }} projects{{ filterAdapter !== 'all' || search ? ' (filtered)' : '' }}
  </div>
</template>
