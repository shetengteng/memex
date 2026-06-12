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
import { useI18n } from '@/i18n'

const palette = useCommandPalette()
const router = useRouter()
const { t, locale } = useI18n()

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
  const tag = locale.value === 'en' ? 'en-US' : 'zh-CN'
  return d.toLocaleString(tag, { dateStyle: 'short', timeStyle: 'short' })
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
    <CommandInput :placeholder="t('cmd.placeholder')" />
    <!--
      Scrollbar: CommandList 默认 `no-scrollbar`（彻底隐藏 webkit + firefox 滚动条），
      列表项很多时用户感知不到能滚。用 cmd-palette-scroll class 显式恢复滚动条
      （样式在 style.css 里定义为 6px 细滚动条，与全站一致）。
    -->
    <CommandList class="cmd-palette-scroll !max-h-[60vh]">
      <CommandEmpty>{{ t('cmd.empty') }}</CommandEmpty>

      <CommandGroup :heading="t('cmd.group.search_results')">
        <CommandItem
          v-for="s in sessions.slice(0, 2)"
          :key="`search-${s.id}`"
          :value="`session ${s.title}`"
          @select="go(`/library?session=${s.id}`)"
        >
          <Search class="mr-2 size-4 shrink-0" />
          <span class="truncate">"{{ s.title }}"</span>
          <!-- 同样：flex-1 + text-right + truncate，避免与 CommandItem 内置的 CheckIcon(ml-auto) 争抢 -->
          <span class="flex-1 truncate text-right text-xs text-muted-foreground">
            · {{ s.project }} · {{ ADAPTER_MAP[s.adapter].label }} · {{ tFmt(s.startedAt) }}
          </span>
        </CommandItem>
      </CommandGroup>

      <CommandSeparator />

      <CommandGroup :heading="t('cmd.group.quick_actions')">
        <CommandItem value="goto today" @select="go('/today')">
          <Sparkles class="mr-2 size-4" />
          {{ t('cmd.action.goto_today') }}
          <CommandShortcut>⌘1</CommandShortcut>
        </CommandItem>
        <CommandItem value="goto library" @select="go('/library')">
          <Layers class="mr-2 size-4" />
          {{ t('cmd.action.goto_library') }}
          <CommandShortcut>⌘2</CommandShortcut>
        </CommandItem>
        <CommandItem value="goto insights" @select="go('/insights')">
          <TrendingUp class="mr-2 size-4" />
          {{ t('cmd.action.goto_insights') }}
          <CommandShortcut>⌘3</CommandShortcut>
        </CommandItem>
        <CommandItem value="goto connect" @select="go('/connect')">
          <Plug class="mr-2 size-4" />
          {{ t('cmd.action.goto_connect') }}
          <CommandShortcut>⌘4</CommandShortcut>
        </CommandItem>
        <CommandItem value="ingest now" @select="palette.close()">
          <RefreshCw class="mr-2 size-4" />
          {{ t('cmd.action.ingest_now') }}
          <CommandShortcut>⌘R</CommandShortcut>
        </CommandItem>
        <CommandItem value="reflect weekly" @select="go('/insights')">
          <Bookmark class="mr-2 size-4" />
          {{ t('cmd.action.reflect_weekly') }}
        </CommandItem>
        <CommandItem value="copy mcp cursor" @select="palette.close()">
          <Copy class="mr-2 size-4" />
          {{ t('cmd.action.copy_mcp') }}
        </CommandItem>
        <CommandItem value="open settings" @select="go('/settings')">
          <Settings class="mr-2 size-4" />
          {{ t('cmd.action.open_settings') }}
          <CommandShortcut>⌘,</CommandShortcut>
        </CommandItem>
      </CommandGroup>

      <CommandSeparator />

      <CommandGroup :heading="t('cmd.group.recent')">
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

      <CommandGroup :heading="t('cmd.group.projects')">
        <CommandItem
          v-for="p in projects"
          :key="`proj-${p.id}`"
          :value="`project ${p.name}`"
          @select="go(`/library?project=${encodeURIComponent(p.path)}`)"
        >
          <FolderGit2 class="mr-2 size-4 shrink-0" />
          <span class="truncate">{{ p.name }}</span>
          <!--
            用 flex-1 + text-right 而不是 ml-auto。
            CommandItem 自带一个隐藏的 CheckIcon(ml-auto) 在末尾，与 sessions span 的 ml-auto
            会争抢右侧空间，导致数字看起来没有靠右、整行凌乱。
            flex-1 + text-right 把 sessions span 撑满中间剩余空间，数字稳定贴右。
          -->
          <span class="flex-1 text-right text-xs text-muted-foreground tabular-nums">
            {{ t('cmd.proj.sessions', { count: p.sessions }) }}
          </span>
        </CommandItem>
      </CommandGroup>
    </CommandList>
  </CommandDialog>
</template>
