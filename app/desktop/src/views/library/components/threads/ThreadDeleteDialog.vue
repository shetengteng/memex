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

defineProps<{
  target: ThreadRow | null
  deleting: boolean
}>()

const emit = defineEmits<{ confirm: []; cancel: [] }>()
</script>

<template>
  <Dialog
    :open="target !== null"
    @update:open="(v: boolean) => { if (!v) emit('cancel') }"
  >
    <DialogContent class="w-[92vw] !max-w-md">
      <DialogHeader>
        <DialogTitle>删除线索</DialogTitle>
        <DialogDescription>
          将删除「{{ target?.name }}」（{{ target?.sessionCount }} 个会话的关联）。
          <br />
          <span class="text-muted-foreground">
            只是删除主题分组，不会删除会话本身。下次"全量聚类"可能会再生成同名线索。
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
          取消
        </Button>
        <Button
          type="button"
          class="bg-destructive text-destructive-foreground hover:bg-destructive/90"
          :disabled="deleting"
          @click="emit('confirm')"
        >
          <Loader2 v-if="deleting" class="mr-1.5 size-3.5 animate-spin" />
          删除
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
