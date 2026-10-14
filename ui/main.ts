import { createApp } from "vue";
import { getCurrentWindow } from "@tauri-apps/api/window";

import App from "./App.vue";
import TrayHover from "./src/components/TrayHover.vue";
import "./style.css";

// DEV 冒烟基座：浏览器直开 ui/（无 Tauri 外壳）时拦截 IPC（对比探针依赖）；
// 生产构建经 import.meta.env.DEV 静态消除，dist 零痕迹
if (import.meta.env.DEV) {
  const { installMockIpc } = await import("./src/dev/mock-invoke");
  installMockIpc();
}

/** 当前窗口 label（无 Tauri runtime 的纯浏览器回 "main"）。 */
function currentWindowLabel(): string {
  try {
    return getCurrentWindow().label;
  } catch {
    return "main";
  }
}

/** DEV 基座专用：`?window=tray-hover` 按悬浮窗渲染（浏览器里 label 恒为 main，
 * 悬浮窗样式只能这样目验）；生产经 `import.meta.env.DEV` 静态消除。 */
function devWindowOverride(): string | null {
  if (!import.meta.env.DEV) {
    return null;
  }
  return new URLSearchParams(window.location.search).get("window");
}

// 应用入口：按窗口 label 分流（PL028.1）——托盘 hover 概览窗挂概览组件，其余挂主窗骨架
const label = devWindowOverride() ?? currentWindowLabel();
if (label === "tray-hover") {
  createApp(TrayHover).mount("#app");
} else {
  createApp(App).mount("#app");
}
