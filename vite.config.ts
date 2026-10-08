import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

// Vite 配置：Vue 插件 + 固定端口；不配 devUrl，构建产物内嵌进 Tauri exe（自包含，规避 dev 端口依赖）
export default defineConfig({
  plugins: [vue()],
  clearScreen: false,
  server: { port: 5173, strictPort: true },
  // cargo 编译句柄锁会使 chokidar EBUSY 崩掉 dev 服务（CT 同款坑）：watch 忽略 core/target
  watch: { ignored: ["**/core/target/**"] },
  envPrefix: ["VITE_", "TAURI_ENV_"],
});
