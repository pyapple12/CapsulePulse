import vue from "@vitejs/plugin-vue";
import { defineConfig } from "vitest/config";

/**
 * 前端测试配置（独立于 vite.config.ts：构建链与测试链互不牵扯，`npm run build` 的
 * `vue-tsc --noEmit` 也不受测试类型影响）。
 *
 * 运行环境选 jsdom（StatsView 大量 SVG/几何 DOM 操作，比 happy-dom 更贴行为）；
 * 测试只覆盖 `ui/tests/`，不扫 `core/`（cargo 产物目录，扫了会拖慢/误收）。
 */
export default defineConfig({
  plugins: [vue()],
  test: {
    environment: "jsdom",
    include: ["ui/tests/**/*.spec.ts"],
    setupFiles: ["ui/tests/setup.ts"],
  },
});
