<script setup lang="ts">
/**
 * 线索删除二次确认 Dialog。
 *
 * 文案明确告诉用户：只删主题分组，不会删会话本身——避免误以为会丢数据。
 */
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Loader2 } from 'lucide-vue-next'
import type { ThreadRow } from '@/types'
import { useI18n } from '@/i18n'

defineProps<{
  target: ThreadRow | null
  deleting: boolean
}>()

const emit = defineEmits<{ confirm: []; cancel: [] }>()
const { t } = useI18n()
</script>

<template>
  <Dialog
    :open="target !== null"
    @update:open="(v: boolean) => { if (!v) emit('cancel') }"
  >
    <DialogContent class="w-[92vw] !max-w-md">
      <DialogHeader>
        <DialogTitle>{{ t('library.threads.delete.title') }}</DialogTitle>
        <DialogDescription>
          {{ t('library.threads.delete.description', { name: target?.name ?? '', count: target?.sessionCount ?? 0 }) }}
          <br />
          <span class="text-muted-foreground">
            {{ t('library.threads.delete.warning') }}
          </span>
        </DialogDescription>
      </DialogHeader>
      <DialogFooter>
        <Button
          type="button"
          variant="outline"
          :disabled="deleting"
          @click="emit('cancel')"
        >
          {{ t('library.threads.delete.cancel') }}
        </Button>
        <Button
          type="button"
          class="bg-destructive text-destructive-foreground hover:bg-destructive/90"
          :disabled="deleting"
          @click="emit('confirm')"
        >
          <Loader2 v-if="deleting" class="mr-1.5 size-3.5 animate-spin" />
          {{ t('library.threads.delete.confirm') }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
