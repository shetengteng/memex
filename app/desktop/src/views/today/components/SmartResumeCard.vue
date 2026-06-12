<script setup lang="ts">
import { computed, ref } from 'vue'
import { Card } from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import {
  ArrowUpRight,
  Send,
  Settings2,
  Zap,
} from 'lucide-vue-next'
import { toast } from 'vue-sonner'
import IdeChip from '@/components/shell/IdeChip.vue'
import { sessions, type Session } from '@/stores/memex'
import LibrarySessionDrawer from '@/views/library/components/LibrarySessionDrawer.vue'
import { toastBackendError } from '@/lib/toast-error'

// 后端暂未暴露"未完成/被中断"信号，先把最近 3 条 session 当候选
const resumeCandidates = computed(() => sessions.slice(0, 3))

// 复用 LibrarySessionDrawer：点击「打开会话」直接就地弹框，
// 避免老逻辑「router.push('/library?session=...')」会先跳到 Library 页面再弹框带来的视觉切换闪烁。
const selected = ref<Session | null>(null)
const drawerOpen = ref(false)

function openSession(s: Session) {
  selected.value = s
  drawerOpen.value = true
}

const fromNow = (iso: string) => {
  if (!iso) return '—'
  const diff = (Date.now() - new Date(iso).getTime()) / 1000
  if (diff < 60) return `${Math.floor(diff)} 秒前`
  if (diff < 3600) return `${Math.floor(diff / 60)} 分钟前`
  if (diff < 86400) return `${Math.floor(diff / 3600)} 小时前`
  return `${Math.floor(diff / 86400)} 天前`
}

/**
 * 把 session 的 title / topics / decisions / intent / next 拼成「续接 prompt」并写剪贴板。
 *
 * 现阶段 IDE 跨进程注入对话 API 全面缺失（Cursor 无 deep link、Codex/OpenCode 的
 * resume 需要 IDE 内部 session id），所以「发送到 IDE」的最小可用形态就是
 * **复制结构化 prompt 到剪贴板**，由用户 paste 到目标 IDE 的对话框续接。
 *
 * 字段缺失时静默跳过对应行，避免出现「关注点：(空)」这种垃圾行。
 */
async function sendToIde(s: Session) {
  const lines: string[] = [
    `[Memex 续接 · ${s.adapter}]`,
    `项目：${s.project || '(未知)'}`,
    `上次主题：${s.title || '(无标题)'}`,
  ]
  if (s.topics?.length) lines.push(`关注点：${s.topics.join(' / ')}`)
  if (s.decisions?.length) lines.push(`最近决策：${s.decisions.join('；')}`)
  if (s.intent) lines.push(`当时意图：${s.intent}`)
  if (s.next?.length) lines.push(`下一步：${s.next.join('；')}`)
  lines.push('')
  lines.push('请基于以上上下文继续讨论。如需完整对话内容，在 Memex 中打开会话查看。')
  const prompt = lines.join('\n')

  try {
    await navigator.clipboard.writeText(prompt)
    toast.success('已复制续接 prompt 到剪贴板', {
      description: `粘贴到 ${s.adapter} 即可接着上次的话题`,
      duration: 6_000,
    })
  } catch (e) {
    toastBackendError('复制失败', e)
  }
}
</script>

<template>
  <Card class="p-5">
    <div class="mb-4 flex items-center justify-between">
      <div class="flex items-center gap-2">
        <Zap class="size-4" :style="{ color: 'var(--adapter-codex)' }" />
        <h3 class="text-[14px] font-semibold">接着想想？</h3>
        <span class="text-[11px] text-muted-foreground">智能续接你的近期会话</span>
      </div>
      <Button variant="ghost" size="sm" class="h-7 gap-1 text-xs">
        <Settings2 class="size-3" />
        规则
      </Button>
    </div>

    <div class="space-y-2">
      <article
        v-for="s in resumeCandidates"
        :key="s.id"
        class="rounded-lg border p-3"
      >
        <!-- 第 1 行：标题 + IDE 标签。把 project·time 从这里挪走，给标题更多空间。-->
        <div class="mb-1 flex items-baseline justify-between gap-3">
          <span class="truncate text-[13px] font-semibold">{{ s.title }}</span>
          <IdeChip :adapter="s.adapter" class="shrink-0" />
        </div>
        <!-- 第 2 行：会话指标，纯 muted 文本 -->
        <p class="mb-2 text-[12px] text-muted-foreground">
          {{ s.messages }} 条消息 · {{ s.adapter }}
        </p>
        <!-- 第 3 行：按钮（左）+ project·time（右）。左右布局，右侧贴边。-->
        <div class="flex items-center gap-1.5">
          <Button size="sm" variant="outline" class="h-7 gap-1 text-xs" @click="openSession(s)">
            <ArrowUpRight class="size-3" />
            打开会话
          </Button>
          <Button
            size="sm"
            variant="ghost"
            class="h-7 gap-1 text-xs"
            title="复制续接 prompt 到剪贴板，可直接粘贴到 IDE 对话框"
            @click="sendToIde(s)"
          >
            <Send class="size-3" />
            发送到 IDE
          </Button>
          <span class="ml-auto truncate text-[11px] text-muted-foreground">
            {{ s.project }} · {{ fromNow(s.startedAt) }}
          </span>
        </div>
      </article>
      <p v-if="!resumeCandidates.length" class="text-center text-[12px] italic text-muted-foreground">
        暂无可继续的会话
      </p>
    </div>
  </Card>

  <LibrarySessionDrawer v-model:open="drawerOpen" :session="selected" />
</template>
