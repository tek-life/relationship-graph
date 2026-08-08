import { defineConfig } from 'vitest/config';

// Vitest 独立配置：不复用 vite.config.ts（其含 Tauri 定制的 strictPort 等 dev server 设置），
// 保证测试基建的引入对 dev/build 行为零影响。
// environment 使用 jsdom：部分 service 模块（如 api.ts）在模块顶层访问 window。
export default defineConfig({
  test: {
    environment: 'jsdom',
    include: ['src/**/*.test.{ts,tsx}'],
  },
});
