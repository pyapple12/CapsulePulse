import vue from "@vitejs/plugin-vue";
import { defineConfig } from "vitest/config";

/**
 * 前端测试配置（独立于 `vite.config.ts`：**不污染构建配置**，只承载测试环境与扫描范围）。
 *
 * ⚠️ **注意**（FIX008.20 更正）：`tsconfig.json` 的 `include` 覆盖整个 `ui/`（含 `ui/tests/`）⇒ 同样
 * 在 `vue-tsc` 管辖内，**测试文件类型错会挡构建**（原注释写"不受测试类型影响"，与事实相反）；
 * 若要彻底分离，需拆 `tsconfig.app.json` 并把 `ui/tests` 放进 `exclude`。
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
