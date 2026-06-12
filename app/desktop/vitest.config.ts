import { defineConfig, mergeConfig } from 'vitest/config'
import viteConfig from './vite.config'

export default mergeConfig(
  viteConfig,
  defineConfig({
    test: {
      environment: 'happy-dom',
      globals: true,
      include: ['src/**/*.{test,spec}.ts'],
      // happy-dom 提供的 DOM 给 @vue/test-utils 用
      restoreMocks: true,
    },
  }),
)
