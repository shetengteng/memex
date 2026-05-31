<script setup lang="ts">
import { ref, provide } from 'vue'
import type { ViewName } from '@/types'
import HomeView from '@/views/HomeView.vue'
import SearchView from '@/views/SearchView.vue'
import SessionView from '@/views/SessionView.vue'
import SettingsView from '@/views/SettingsView.vue'

const currentView = ref<ViewName>('home')
const selectedSessionId = ref<string | null>(null)

function navigate(view: ViewName, sessionId?: string) {
  currentView.value = view
  if (sessionId) selectedSessionId.value = sessionId
}

function back() {
  if (currentView.value === 'session') {
    currentView.value = 'search'
  } else {
    currentView.value = 'home'
  }
}

provide('navigate', navigate)
provide('back', back)

function onKeydown(e: KeyboardEvent) {
  if (e.key === 'Escape') {
    if (currentView.value === 'home') {
      // TODO: hide window via Tauri API
    } else {
      back()
    }
  }
}
</script>

<template>
  <div
    class="flex h-screen w-screen flex-col overflow-hidden bg-background"
    @keydown="onKeydown"
    tabindex="0"
  >
    <HomeView v-if="currentView === 'home'" />
    <SearchView v-else-if="currentView === 'search'" />
    <SessionView
      v-else-if="currentView === 'session'"
      :session-id="selectedSessionId ?? ''"
    />
    <SettingsView v-else-if="currentView === 'settings'" />
  </div>
</template>
