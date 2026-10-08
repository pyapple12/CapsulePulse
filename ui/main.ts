import { createApp } from "vue";
import App from "./App.vue";
import "./style.css";

// DEV 冒烟基座：浏览器直开 ui/（无 Tauri 外壳）时拦截 IPC（对比探针依赖）；
// 生产构建经 import.meta.env.DEV 静态消除，dist 零痕迹
if (import.meta.env.DEV) {
  const { installMockIpc } = await import("./src/dev/mock-invoke");
  installMockIpc();
}

// 应用入口：挂载 Vue 根组件（展示层骨架，业务逻辑零含量）
createApp(App).mount("#app");
