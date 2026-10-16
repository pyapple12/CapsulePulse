<script setup lang="ts">
/** 托盘右键菜单窗（PATCH008）：自绘 HTML 菜单替代原生菜单——外观照 **CP hover 卡配方**
 * （同一套底 / 描边 / 圆角 / 字号 ⇒ 两窗同外观），五项动作经 `tray_menu_action` 统一分发
 * （Rust 侧先收窗再执行）。勾选态事实源 = `get_settings` 初值 + `prefs-changed` 广播回显；
 * 尺寸量内容后经 `tray_menu_resize` 重设（向上取 4 的倍数 ⇒ 逻辑物理双整数，PATCH007.24 同款）。 */
import { nextTick, onMounted, onUnmounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import type { PrefsSnapshot, ReminderSettings } from "../../types";

/** 菜单项动作全集（与 Rust `tray_menu_action` 分发一致；id 沿用原菜单项语义） */
type MenuAction =
  "focus_main" | "toggle_timer" | "toggle_always_on_top" | "toggle_snap_to_edge" | "quit";

const cardEl = ref<HTMLElement | null>(null);
/** 量尺寸的对象 = **内容层**（卡片铺满窗，量卡片会自反馈 ✗） */
const bodyEl = ref<HTMLElement | null>(null);
/** 窗口置顶勾选态（事实源 = config.json：`get_settings` 初值 + `prefs-changed` 回显） */
const topOn = ref(false);
/** 贴边吸附勾选态（同上） */
const snapOn = ref(false);
let unlisten: (() => void) | undefined;

/** 把目标尺寸**向上**取到 **4 的倍数**（PATCH007.24 同款纪律）：125% 下 4 逻辑 = 5 物理 ⇒
 * 逻辑与物理同时为整数 ⇒ 可视区 = 窗 = 卡片，无取整差；**只向上**（只多不少 ⇒ 内容永不裁）。 */
function snap4(value: number): number {
  return Math.ceil(value / 4) * 4;
}

/** 量内容定窗（PATCH008.3）：**内容层** + 从 CSS 读 padding / border / margin 作 chrome ⇒
 * 窗 = 内容 + chrome，一律 `snap4`；宽高都走本管线（菜单窗无固定宽）。 */
async function resizeToContent(): Promise<void> {
  await nextTick();
  const body = bodyEl.value;
  const card = cardEl.value;
  if (body == null || card == null) {
    return;
  }
  const rect = body.getBoundingClientRect();
  const cs = getComputedStyle(card);
  const chromeX =
    parseFloat(cs.paddingLeft) +
    parseFloat(cs.paddingRight) +
    parseFloat(cs.borderLeftWidth) +
    parseFloat(cs.borderRightWidth) +
    parseFloat(cs.marginLeft) +
    parseFloat(cs.marginRight);
  const chromeY =
    parseFloat(cs.paddingTop) +
    parseFloat(cs.paddingBottom) +
    parseFloat(cs.borderTopWidth) +
    parseFloat(cs.borderBottomWidth) +
    parseFloat(cs.marginTop) +
    parseFloat(cs.marginBottom);
  try {
    await invoke("tray_menu_resize", {
      width: snap4(rect.width + chromeX),
      height: snap4(rect.height + chromeY),
    });
  } catch (err) {
    console.error("菜单窗尺寸调整失败", err);
  }
}

/** 点菜单项：动作分发（收窗与执行都在 Rust 侧统一做——原生菜单同款语义） */
async function onAction(action: MenuAction): Promise<void> {
  try {
    await invoke("tray_menu_action", { action });
  } catch (err) {
    console.error("菜单动作执行失败", err);
  }
}

onMounted(async () => {
  // 高度链修正（hover 窗同款）：`html`/`body`/`#app` 无显式高度时 `height: 100%` 塌缩露
  // webview 底；三层一并 `overflow: hidden`（瞬时溢出不出滚动条）。本窗实例直改。
  document.documentElement.style.height = "100%";
  document.documentElement.style.background = "transparent";
  document.documentElement.style.overflow = "hidden";
  document.body.style.height = "100%";
  document.body.style.background = "transparent";
  document.body.style.overflow = "hidden";
  const root = document.getElementById("app");
  if (root != null) {
    root.style.height = "100%";
    root.style.background = "transparent";
    root.style.overflow = "hidden";
  }
  // FIX006.20e：**先 listen 再取值**——原顺序（先取值后 listen）在两步之间丢 `prefs-changed`，
  // 勾选态会陈旧；取值期间若已收到事件则以事件为准（`prefsSeen` 守卫，防旧值覆盖新值）。
  let prefsSeen = false;
  listen<PrefsSnapshot>("prefs-changed", (event) => {
    prefsSeen = true;
    topOn.value = event.payload.always_on_top;
    snapOn.value = event.payload.snap_to_edge;
  })
    .then((un) => {
      unlisten = un;
    })
    .catch((err) => console.error("prefs-changed 监听注册失败", err));
  try {
    const settings = await invoke<ReminderSettings>("get_settings");
    if (!prefsSeen) {
      topOn.value = settings.always_on_top;
      snapOn.value = settings.snap_to_edge;
    }
  } catch (err) {
    console.error("菜单勾选态拉取失败", err);
  }
  await resizeToContent();
});

onUnmounted(() => unlisten?.());
</script>

<template>
  <div ref="cardEl" class="menu-card">
    <div ref="bodyEl" class="menu-body">
      <button class="mi" type="button" @click="onAction('focus_main')">
        <span class="mi-check" aria-hidden="true"></span>
        <span class="mi-text">聚焦主窗</span>
      </button>
      <button class="mi" type="button" @click="onAction('toggle_timer')">
        <span class="mi-check" aria-hidden="true"></span>
        <span class="mi-text">切换状态</span>
      </button>
      <div class="mi-sep" aria-hidden="true"></div>
      <button class="mi" type="button" @click="onAction('toggle_always_on_top')">
        <span class="mi-check" aria-hidden="true">
          <svg v-if="topOn" viewBox="0 0 12 12"><polyline points="2.5,6.5 5,9 9.5,3" /></svg>
        </span>
        <span class="mi-text">窗口置顶</span>
      </button>
      <button class="mi" type="button" @click="onAction('toggle_snap_to_edge')">
        <span class="mi-check" aria-hidden="true">
          <svg v-if="snapOn" viewBox="0 0 12 12"><polyline points="2.5,6.5 5,9 9.5,3" /></svg>
        </span>
        <span class="mi-text">贴边吸附</span>
      </button>
      <div class="mi-sep" aria-hidden="true"></div>
      <button class="mi" type="button" @click="onAction('quit')">
        <span class="mi-check" aria-hidden="true"></span>
        <span class="mi-text">退出</span>
      </button>
    </div>
  </div>
</template>

<style scoped>
/* 菜单卡（PATCH008）：**照抄 hover 卡配方**（同一套底 / 描边 / 圆角 ⇒ 两窗同外观 ✓）——
   卡片 ≡ 窗（`height: 100%` + 无 margin）；尺寸经 `snap4` 双整数（PATCH007.24 纪律）⇒
   无半像素机会、无亮/暗缝、底边框完整。左右内距留给**行**（hover 高亮可铺满整宽 ✓）。 */
.menu-card {
  box-sizing: border-box;
  height: 100%;
  margin: 0;
  padding: 6px 0;
  border-radius: 8px; /* 照抄 hover ✓ */
  background: rgba(18, 20, 26, 0.86); /* 照抄 hover ✓ */
  border: 1px solid rgba(255, 255, 255, 0.18); /* 照抄 hover ✓ */
  color: #f2f4f8;
  font-family: system-ui, sans-serif;
  display: flex;
  flex-direction: column;
  justify-content: center; /* 取整余量 ≤4 逻辑 ⇒ 上下均分、不可见 */
  user-select: none;
  cursor: default;
}
/* 测量对象（同 hover）：内容层收缩包裹 ⇒ 量出的宽高 = 内容固有尺寸 ✓ */
.menu-body {
  display: flex;
  flex-direction: column;
  width: max-content;
}
/* 菜单行（骨架照 CT `.tm-item`，数值按 CP 体系；行高 / 字号实机目测定稿） */
.mi {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
  height: 28px;
  padding: 0 24px 0 8px; /* 右侧 = 原 10 + 14（勾选列宽）——用户 2026-10-16 定案 */
  border: none;
  background: transparent;
  color: inherit;
  font: inherit;
  font-size: 12px;
  text-align: left;
  cursor: pointer;
}
.mi:hover {
  background: rgba(255, 255, 255, 0.08);
}
/* 勾选列定宽（原生菜单同款列结构，勾选标记不挤动文本） */
.mi-check {
  flex: none;
  width: 14px;
  height: 14px;
  display: grid;
  place-items: center;
}
.mi-check svg {
  width: 11px;
  height: 11px;
}
.mi-check polyline {
  fill: none;
  stroke: #f2f4f8;
  stroke-width: 1.8;
  stroke-linecap: round;
  stroke-linejoin: round;
}
.mi-text {
  flex: 1;
  white-space: nowrap;
}
/* 分隔线（CP 原生菜单两处：显示/暂停 ｜ 置顶/贴边 ｜ 退出） */
.mi-sep {
  height: 1px;
  margin: 4px 8px;
  background: rgba(255, 255, 255, 0.1);
}
</style>
