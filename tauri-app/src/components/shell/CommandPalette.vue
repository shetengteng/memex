<script setup lang="ts">
import { onMounted, onUnmounted } from 'vue'
import { useRouter } from 'vue-router'
import {
  CommandDialog,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
  CommandSeparator,
  CommandShortcut,
} from '@/components/ui/command'
import {
  Sparkles,
  Layers,
  TrendingUp,
  Plug,
  Settings,
  Clock,
  FolderGit2,
  Hash,
  RefreshCw,
  Bookmark,
  Copy,
  Zap,
  Search,
} from 'lucide-vue-next'
import { useCommandPalette } from '@/composables/useCommandPalette'
import { sessions, projects, ADAPTER_MAP } from '@/stores/memex'

const palette = useCommandPalette()
const router = useRouter()

const onKey = (e: KeyboardEvent) => {
  if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'k') {
    e.preventDefault()
    palette.toggle()
  }
  if (palette.isOpen.value && (e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'r') {
    e.preventDefault()
    palette.close()
  }
}

onMounted(() => window.addEventListener('keydown', onKey))
onUnmounted(() => window.removeEventListener('keydown', onKey))

const go = (to: string) => {
  palette.close()
  router.push(to)
}

const tFmt = (iso: string) => {
  const d = new Date(iso)
  return d.toLocaleString('zh-CN', { dateStyle: 'short', timeStyle: 'short' })
}
</script>

<template>
  <!--
    Width: 默认 shadcn-vue DialogContent 是 sm:max-w-sm（~384px），命令面板里
    "项目名 · 适配器 · 时间"一行经常被截，所以同 Library/Reflection dialog 一样
    用 `!max-w-2xl` 拉到 ~672px，移动端用 w-[92vw] 兜底。
    Height: CommandList 默认 max-h-72（288px）也太矮，几条记录就要滚动；
    用 max-h-[60vh] 让它随窗口高度自适应。
    Position: CommandDialog 默认 `top-1/3 translate-y-0`（spotlight 风格），
    被用户反映"没有居中"。用 `!top-1/2 !-translate-y-1/2` 还原回 DialogContent
    的纵向居中行为，与 Library / Reflection dialog 保持一致。
  -->
  <CommandDialog
    v-model:open="palette.isOpen.value"
    class="w-[92vw] !max-w-2xl !top-1/2 !-translate-y-1/2"
  >
    <CommandInput placeholder="搜索任何东西…" />
    <CommandList class="!max-h-[60vh]">
      <CommandEmpty>没有匹配的结果</CommandEmpty>

      <CommandGroup heading="搜索结果">
        <CommandItem
          v-for="s in sessions.slice(0, 2)"
          :key="`search-${s.id}`"
          :value="`session ${s.title}`"
          @select="go(`/library?session=${s.id}`)"
        >
          <Search class="mr-2 size-4" />
          <span class="truncate">"{{ s.title }}"</span>
          <span class="ml-auto shrink-0 text-xs text-muted-foreground">
            · {{ s.project }} · {{ ADAPTER_MAP[s.adapter].label }} · {{ tFmt(s.startedAt) }}
          </span>
        </CommandItem>
      </CommandGroup>

      <CommandSeparator />

      <CommandGroup heading="快速操作">
        <CommandItem value="goto today" @select="go('/today')">
          <Sparkles class="mr-2 size-4" />
          跳到「今天」
          <CommandShortcut>⌘1</CommandShortcut>
        </CommandItem>
        <CommandItem value="goto library" @select="go('/library')">
          <Layers class="mr-2 size-4" />
          跳到「资料库」
          <CommandShortcut>⌘2</CommandShortcut>
        </CommandItem>
        <CommandItem value="goto insights" @select="go('/insights')">
          <TrendingUp class="mr-2 size-4" />
          跳到「洞察」
          <CommandShortcut>⌘3</CommandShortcut>
        </CommandItem>
        <CommandItem value="goto connect" @select="go('/connect')">
          <Plug class="mr-2 size-4" />
          跳到「连接」
          <CommandShortcut>⌘4</CommandShortcut>
        </CommandItem>
        <CommandItem value="ingest now" @select="palette.close()">
          <RefreshCw class="mr-2 size-4" />
          立即采集一次
          <CommandShortcut>⌘R</CommandShortcut>
        </CommandItem>
        <CommandItem value="reflect weekly" @select="go('/insights')">
          <Bookmark class="mr-2 size-4" />
          生成本周反思
        </CommandItem>
        <CommandItem value="copy mcp cursor" @select="palette.close()">
          <Copy class="mr-2 size-4" />
          复制 MCP 配置（Cursor）
        </CommandItem>
        <CommandItem value="open settings" @select="go('/settings')">
          <Settings class="mr-2 size-4" />
          打开「设置」
          <CommandShortcut>⌘,</CommandShortcut>
        </CommandItem>
      </CommandGroup>

      <CommandSeparator />

      <CommandGroup heading="最近会话">
        <CommandItem
          v-for="s in sessions.slice(0, 5)"
          :key="`recent-${s.id}`"
          :value="`recent ${s.title}`"
          @select="go(`/library?session=${s.id}`)"
        >
          <Clock class="mr-2 size-4" />
          <span class="truncate">{{ s.project }} · {{ ADAPTER_MAP[s.adapter].label }} · {{ tFmt(s.startedAt) }}</span>
        </CommandItem>
      </CommandGroup>

      <CommandSeparator />

      <CommandGroup heading="项目">
        <CommandItem
          v-for="p in projects"
          :key="`proj-${p.id}`"
          :value="`project ${p.name}`"
          @select="go(`/library?project=${p.name}`)"
        >
          <FolderGit2 class="mr-2 size-4" />
          {{ p.name }}
          <span class="ml-auto text-xs text-muted-foreground tabular-nums">{{ p.sessions }} 个会话</span>
        </CommandItem>
      </CommandGroup>
    </CommandList>
  </CommandDialog>
</template>
