<script setup lang="ts">
/** 托盘 hover 今日概览（PL028.1；PATCH007.3/.6/.8 收口）：按后端 DTO 渲染 1 行（未在岗）或
 * 3 行（在岗 / 暂下班）。**几何归 Rust**：宽固定 `hover::WINDOW_WIDTH`、高由本组件量内容后
 * 经 `tray_hover_resize` 命令重设（JS `setSize` 需 ACL 会静默拒绝）；本组件只渲染 + 量高。
 * 数据：挂载拉一次 + 后端每次显示与每秒节拍推 `hover-update`（PATCH007.1/.6）。
 * 格式：一律 `XX小时XX分钟`（PATCH007.8，展示层统一口径）。布局：**全部左对齐**。 */
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import { splitDuration } from "../../format";
import type { HoverSummary } from "../../types";

const summary = ref<HoverSummary | null>(null);
/** 量尺寸的对象 = **内容层**（PATCH007.21：卡片是 `height: 100%` 铺满窗，量卡片会自反馈 ✗） */
const bodyEl = ref<HTMLElement | null>(null);
/** 读 CSS 用（内边距/边框/外边距从样式里取 ✓，不手抄常量 ✓） */
const cardEl = ref<HTMLElement | null>(null);
let unlisten: (() => void) | undefined;

// PATCH007.12 方案 B：数字拆三段交 CSS `.num`（min-width:2ch + 右对齐）对齐，不做字符补位 ✓
const remainParts = computed(() => splitDuration((summary.value?.remaining_secs ?? 0) / 60));
const workParts = computed(() => splitDuration((summary.value?.work_secs ?? 0) / 60));
const restParts = computed(() => splitDuration((summary.value?.rest_secs ?? 0) / 60));

/** 把目标尺寸**向上**取到 **4 的倍数**（PATCH007.24 用户定案）：125% 下 4 逻辑 = 5 物理 ⇒
 * **逻辑与物理同时为整数** ⇒ 系统取整与 WebView 视口取整不再产生任何差（可视区 = 窗 = 卡片）；
 * 100% / 150% / 175% / 200% 下同样成立 ✓。**只向上**（只多不少 ⇒ 内容永不裁 ✓）。 */
function snap4(value: number): number {
  return Math.ceil(value / 4) * 4;
}

/** 窗尺寸自适应（PATCH007.8 + .14 + .23 + .24）：**高恒按内容实测** ✓、**单行宽按内容实测** ✓
 * （PATCH007.14 用户定案），两者一律经 `snap4` **向上取到 4 的倍数**（双整数 ⇒ 无取整差 ✓）；
 * 多行态传 `null` 交后端取固定 228（= 4×57 ✓ 已双整数）。Rust 命令设完会**重锚托盘**（右缘恒对齐
 * 图标右缘 ⇒ 宽度变化表现为**左缘移动、右缘不动** ✓）。 */
watch(
  summary,
  async () => {
    await nextTick();
    const body = bodyEl.value;
    const card = cardEl.value;
    if (body == null || card == null) {
      return;
    }
    // PATCH007.19/.21/.23/.24（A 方案延伸）：**量内容**（`.hover-body` ✓）+ 从 CSS 读内边距/边框/外边距 ✓
    // （不手抄常量 ✓）⇒ 窗 = 内容 + 这些 ✓；卡片 `height: 100%` 铺满 ⇒ 取整差藏进卡片内 ✓；
    // 尺寸一律经 **`snap4`** 向上取到 4 的倍数（PATCH007.24 ✓）—— `.23` 的 `ceil` 只保证逻辑整数，
    // 单行 34 → 42.5 物理仍落半像素 ⇒ 底边框被裁；4 的倍数 = 逻辑 + 物理双整数 ⇒ 差从根上消失 ✓
    const bodyRect = body.getBoundingClientRect();
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
    const singleLine = summary.value?.single_line === true;
    try {
      await invoke("tray_hover_resize", {
        width: singleLine ? snap4(bodyRect.width + chromeX) : null,
        height: snap4(bodyRect.height + chromeY),
      });
    } catch (err) {
      console.error("悬浮窗尺寸调整失败", err);
    }
  },
  { deep: true },
);

onMounted(() => {
  // 高度链修正（PATCH007.3，抄 CT `TrayPreview.vue`）：`html`/`body`/`#app` 无显式高度时
  // `height: 100%` 会塌缩为内容高 ⇒ 窗体剩余区露出 webview 默认底（用户截图"两层"）。
  // 本窗实例直改，只影响悬浮窗、不动主窗布局。
  document.documentElement.style.height = "100%";
  document.documentElement.style.background = "transparent";
  document.documentElement.style.overflow = "hidden"; // PATCH007.19：任何瞬时溢出都不出滚动条 ✓
  document.body.style.height = "100%";
  document.body.style.background = "transparent";
  document.body.style.overflow = "hidden";
  const root = document.getElementById("app");
  if (root != null) {
    root.style.height = "100%";
    root.style.background = "transparent";
    root.style.overflow = "hidden";
  }
  void invoke<HoverSummary>("tray_hover")
    .then((value) => (summary.value = value))
    .catch((err) => console.error("tray_hover 调用失败", err));
  listen<HoverSummary>("hover-update", (event) => {
    summary.value = event.payload;
  })
    .then((un) => {
      unlisten = un;
    })
    .catch((err) => console.error("hover-update 监听注册失败", err));
});

onUnmounted(() => unlisten?.());
</script>

<template>
  <div ref="cardEl" class="hover-card">
    <div ref="bodyEl" class="hover-body">
      <template v-if="summary">
        <div class="row main">
          <span class="label">{{ summary.label }}</span>
          <span v-if="summary.remaining_secs != null" class="value">
            剩余 <span class="num">{{ remainParts.h }}</span
            >小时<span class="num">{{ remainParts.m }}</span
            >分钟
          </span>
        </div>
        <template v-if="!summary.single_line">
          <div class="row">
            <span class="label">工作</span>
            <span class="value">
              <span class="num">{{ workParts.h }}</span
              >小时<span class="num">{{ workParts.m }}</span
              >分钟
            </span>
            <span class="label gap">休息</span>
            <span class="value">
              {{ restParts.h }}小时<span class="num">{{ restParts.m }}</span
              >分钟
            </span>
          </div>
          <div class="row">
            <span class="label">离休息提醒</span>
            <span class="value">{{ summary.until_reminder_min.toFixed(1) }} 分钟</span>
          </div>
        </template>
      </template>
    </div>
  </div>
</template>

<style scoped>
/* 悬浮卡（PL028.1）：窗为纯 alpha 透明（tauri.conf transparent），卡自带玻璃配方——
   窗内采样有效（与浮板同款：深纱 + backdrop blur）；圆角/描边与主窗浮板同语言。
   纯展示件：`user-select: none` 防"按住拖选卡片文字"（PATCH007.3，抄 CT TrayPreview 同款）。 */
.hover-card {
  box-sizing: border-box;
  /* PATCH007.21/.23（用户定案）：**卡片 ≡ 窗**（`height: 100%` 铺满 + **无 margin** ✓ —— 与 CT
     `TrayPreview` 同构 ✓）：`.21` 曾写成 `margin: 1px 0`（简写 = **上下** 1px、左右 0 —— 顺序写反 ✗）
     ⇒ 卡片比窗高 1px、下缘掉出窗外 ⇒ **底边框整个被裁**（`.22` 高度吸附救不了：溢出恒 1px、与窗高
     无关 ✓）。归零后卡片外缘 = 窗边缘 ⇒ 边框永远画在窗内 ✓（**PATCH007.24 起窗尺寸恒为"逻辑 +
     物理双整数"（4 的倍数）⇒ 无半像素机会 ✓**）。去掉 `width: 100%`（块级 auto 宽 = 容器宽 ✓；
     **PATCH007.25 起单行态同款铺满 ✓**） */
  height: 100%;
  margin: 0;
  padding: 8px 9px; /* PATCH007.19：**左右 9px**（让出被 1px 边框吃掉的 1px ×2 ✓ —— 内容可用宽
     与"窗 230 + 内边距 10"完全等价 ✓；文字距外缘 10px vs CT 的 11px，差 1px 不可见 ✓）；上下保持 8px ✓ */
  border-radius: 8px; /* 照抄 CT（TrayPreview 原值 ✓；原 10px 已按用户指示改回 8 ✓） */
  background: rgba(18, 20, 26, 0.86); /* 保持 CP 现状 ✓ */
  /* PATCH007.16（照抄 CT）：**无外投影** ✗（卡片两种态都正好铺满窗 ⇒ 投影必被窗裁切 ⇒ 下/右暗带、
     上/左干净 = 用户报的"不一致" ✓）、**无 backdrop-filter** ✗（透明窗内无采样对象 ✓）、
     边框改**真 `border`** ✓（原用 `inset` box-shadow 模拟 ✗ —— 其描边在圆角处宽度被压缩 ⇒ 直段与
     角上粗细不均 ✓；真 border 由浏览器按圆角均匀绘制 ✓）。圆角 10 / 底色 0.86 按用户定保持 ✓ */
  border: 1px solid rgba(255, 255, 255, 0.18); /* 照抄 CT ✓（PATCH007.18 试 0.8px 无变化 ⇒ 已回退 ✓） */
  color: #f2f4f8;
  font-family: system-ui, sans-serif;
  display: flex;
  flex-direction: column;
  justify-content: center;
  user-select: none;
  cursor: default;
}
/* 测量对象（PATCH007.8；PATCH007.25 补 `width: max-content`）：**内容层收缩包裹** ⇒ 量出的宽 =
   内容固有宽 ✓（卡片已铺满窗，若 body 被拉伸则量到的是窗宽 = 自反馈 ✗）；高度随内容 ✓ */
.hover-body {
  display: flex;
  flex-direction: column;
  gap: 6px;
  width: max-content;
}
/* PATCH007.14/.25：单行态窗宽自适应改由**内容层**承担（`.hover-body { width: max-content }` ✓）——
   PATCH007.25 起**卡片一律铺满窗**（原 `.single { width: max-content }` 让卡片比窗窄 ≤1px ⇒
   右缘露背景 = 亮/暗缝 ✗，已删 ✓）。外投影/blur 已在基础样式统一去掉（PATCH007.16）✓ */
/* 三行**全部左对齐**（PATCH007.8：不做任何右对齐） */
.row {
  display: flex;
  align-items: baseline;
  gap: 6px;
  font-size: 11px;
  line-height: 1.2;
}
.row.main .label,
.row.main .value {
  font-size: 13px;
  font-weight: 600;
}
.label {
  opacity: 0.72;
}
.value {
  font-variant-numeric: tabular-nums;
}
/* PATCH007.12 方案 B：数字列定宽右对齐（`ch` = "0" 的宽 ⇒ 与数字同宽、不依赖字体 ✓）——
   单位数右端与两位数对齐；「不补 0」只是空出 1ch 位置 ✓；休息小时位不加 `.num`（恒单字符 ✓） */
.num {
  display: inline-block;
  min-width: 2ch;
  text-align: right;
}
.gap {
  margin-left: 8px;
}
</style>
