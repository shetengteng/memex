<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import { openUrl } from '@tauri-apps/plugin-opener'
import { AlertTriangle, Copy, ExternalLink, Terminal } from 'lucide-vue-next'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogTitle,
} from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
import { useMemex } from '@/composables/useMemex'
import { useI18n } from '@/i18n'

type OllamaSetupKind = 'not_installed' | 'no_model'

const { t } = useI18n()
const memex = useMemex()
const router = useRouter()

const open = ref(false)
const kind = ref<OllamaSetupKind>('not_installed')
const configuredModel = ref('qwen2.5')
const cmdCopied = ref(false)
const brewCopied = ref(false)

const DISMISSED_KEY = 'ollama_setup_dismissed'

async function checkOllama() {
  try {
    const dismissed = await memex.getConfig(DISMISSED_KEY)
    if (dismissed === 'true') return

    const model = await memex.getConfig('llm.ollama_model')
    if (model) configuredModel.value = model.replace(/:.*$/, '')

    const resp = await fetch('http://127.0.0.1:11434/api/tags', {
      signal: AbortSignal.timeout(3000),
    })
    if (!resp.ok) {
      kind.value = 'not_installed'
      open.value = true
      return
    }
    const data = (await resp.json()) as { models?: { name: string }[] }
    const models = data.models ?? []
    if (models.length === 0) {
      kind.value = 'no_model'
      open.value = true
    }
  } catch {
    kind.value = 'not_installed'
    open.value = true
  }
}

function dismiss() {
  open.value = false
}

async function dismissForever() {
  open.value = false
  try {
    await memex.setConfig(DISMISSED_KEY, 'true')
  } catch {
    /* best-effort */
  }
}

function goToSettings() {
  open.value = false
  router.push('/settings')
}

async function copyCmd() {
  try {
    await navigator.clipboard.writeText(`ollama pull ${configuredModel.value}`)
    cmdCopied.value = true
    setTimeout(() => {
      cmdCopied.value = false
    }, 1500)
  } catch {
    /* ignore */
  }
}

async function copyBrew() {
  try {
    await navigator.clipboard.writeText('brew install ollama')
    brewCopied.value = true
    setTimeout(() => {
      brewCopied.value = false
    }, 1500)
  } catch {
    /* ignore */
  }
}

async function openWebsite() {
  try {
    await openUrl('https://ollama.com/download')
  } catch {
    /* ignore */
  }
}

onMounted(() => {
  void checkOllama()
})
</script>

<template>
  <Dialog v-model:open="open">
    <DialogContent class="max-w-md">
      <div class="flex items-start gap-3">
        <div class="flex h-9 w-9 shrink-0 items-center justify-center rounded-full bg-amber-500/10">
          <AlertTriangle class="size-4 text-amber-500" />
        </div>
        <div class="flex-1 space-y-3">
          <DialogTitle class="text-sm font-semibold leading-snug">
            {{
              kind === 'not_installed'
                ? t('ollama_setup.title_not_installed')
                : t('ollama_setup.title_no_model')
            }}
          </DialogTitle>
          <DialogDescription class="text-xs leading-relaxed text-muted-foreground">
            {{
              kind === 'not_installed'
                ? t('ollama_setup.desc_not_installed')
                : t('ollama_setup.desc_no_model')
            }}
          </DialogDescription>

          <div v-if="kind === 'not_installed'" class="space-y-2">
            <Button variant="default" size="sm" class="h-7 gap-1 text-xs" @click="openWebsite">
              <ExternalLink class="size-3" />
              {{ t('ollama_setup.install_ollama') }}
            </Button>
            <div class="flex items-center gap-1.5 text-[11px] text-muted-foreground">
              <span>{{ t('ollama_setup.brew_hint') }}</span>
              <code class="rounded bg-muted px-1.5 py-0.5 font-mono text-[11px]">brew install ollama</code>
              <button
                class="inline-flex cursor-pointer items-center gap-0.5 text-[11px] text-primary hover:underline"
                @click="copyBrew"
              >
                <Copy class="size-3" />
                {{ brewCopied ? t('common.copied') : t('common.copy') }}
              </button>
            </div>
          </div>

          <div v-else class="space-y-2">
            <p class="text-[11px] font-medium text-muted-foreground">
              {{ t('ollama_setup.recommended_model') }}:
              <strong class="text-foreground">{{ configuredModel }}</strong>
            </p>
            <div class="flex items-center gap-1.5">
              <div class="flex items-center gap-1 rounded-md border border-border bg-muted px-2 py-1">
                <Terminal class="size-3 text-muted-foreground" />
                <code class="font-mono text-[11px]">ollama pull {{ configuredModel }}</code>
              </div>
              <Button variant="ghost" size="sm" class="h-7 px-2 text-[11px]" @click="copyCmd">
                <Copy class="mr-0.5 size-3" />
                {{ cmdCopied ? t('common.copied') : t('common.copy') }}
              </Button>
            </div>
            <p class="text-[11px] leading-relaxed text-muted-foreground">
              {{ t('ollama_setup.other_models') }}
            </p>
          </div>
        </div>
      </div>

      <div class="mt-4 flex items-center justify-between border-t border-border/40 pt-3">
        <button
          class="cursor-pointer text-[11px] text-muted-foreground hover:text-foreground hover:underline"
          @click="dismissForever"
        >
          {{ t('ollama_setup.dont_show') }}
        </button>
        <div class="flex items-center gap-2">
          <Button variant="ghost" size="sm" class="h-7 text-xs" @click="dismiss">
            {{ t('ollama_setup.dismiss') }}
          </Button>
          <Button variant="default" size="sm" class="h-7 text-xs" @click="goToSettings">
            {{ t('ollama_setup.go_settings') }}
          </Button>
        </div>
      </div>
    </DialogContent>
  </Dialog>
</template>
