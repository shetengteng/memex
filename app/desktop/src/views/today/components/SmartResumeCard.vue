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
import { useI18n } from '@/i18n'

const { t } = useI18n()

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
  if (diff < 60) return t('today.rel.seconds', { n: Math.floor(diff) })
  if (diff < 3600) return t('today.rel.minutes', { n: Math.floor(diff / 60) })
  if (diff < 86400) return t('today.rel.hours', { n: Math.floor(diff / 3600) })
  return t('today.rel.days', { n: Math.floor(diff / 86400) })
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
    t('today.resume.prompt.header', { adapter: s.adapter }),
    t('today.resume.prompt.project', { value: s.project || t('today.resume.prompt.unknown') }),
    t('today.resume.prompt.last_topic', { value: s.title || t('today.resume.prompt.untitled') }),
  ]
  if (s.topics?.length) lines.push(t('today.resume.prompt.topics', { value: s.topics.join(' / ') }))
  if (s.decisions?.length) lines.push(t('today.resume.prompt.decisions', { value: s.decisions.join('; ') }))
  if (s.intent) lines.push(t('today.resume.prompt.intent', { value: s.intent }))
  if (s.next?.length) lines.push(t('today.resume.prompt.next', { value: s.next.join('; ') }))
  lines.push('')
  lines.push(t('today.resume.prompt.footer'))
  const prompt = lines.join('\n')

  try {
    await navigator.clipboard.writeText(prompt)
    toast.success(t('today.resume.copied'), {
      description: t('today.resume.copied_desc', { adapter: s.adapter }),
      duration: 6_000,
    })
  } catch (e) {
    toastBackendError(t('today.resume.copy_failed'), e)
  }
}
</script>

<template>
  <Card class="p-5">
    <div class="mb-4 flex items-center justify-between">
      <div class="flex items-center gap-2">
        <Zap class="size-4" :style="{ color: 'var(--adapter-codex)' }" />
        <h3 class="text-[14px] font-semibold">{{ t('today.resume.title') }}</h3>
        <span class="text-[11px] text-muted-foreground">{{ t('today.resume.subtitle') }}</span>
      </div>
      <Button variant="ghost" size="sm" class="h-7 gap-1 text-xs">
        <Settings2 class="size-3" />
        {{ t('today.resume.rules') }}
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
          {{ t('today.resume.msg_count', { count: s.messages, adapter: s.adapter }) }}
        </p>
        <!-- 第 3 行：按钮（左）+ project·time（右）。左右布局，右侧贴边。-->
        <div class="flex items-center gap-1.5">
          <Button size="sm" variant="outline" class="h-7 gap-1 text-xs" @click="openSession(s)">
            <ArrowUpRight class="size-3" />
            {{ t('today.resume.open') }}
          </Button>
          <Button
            size="sm"
            variant="ghost"
            class="h-7 gap-1 text-xs"
            :title="t('today.resume.send_title')"
            @click="sendToIde(s)"
          >
            <Send class="size-3" />
            {{ t('today.resume.send') }}
          </Button>
          <span class="ml-auto truncate text-[11px] text-muted-foreground">
            {{ s.project }} · {{ fromNow(s.startedAt) }}
          </span>
        </div>
      </article>
      <p v-if="!resumeCandidates.length" class="text-center text-[12px] italic text-muted-foreground">
        {{ t('today.resume.empty') }}
      </p>
    </div>
  </Card>

  <LibrarySessionDrawer v-model:open="drawerOpen" :session="selected" />
</template>
