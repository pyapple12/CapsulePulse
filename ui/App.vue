<script setup lang="ts">
import { onMounted, onUnmounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
// PL008.3：dock 时代 ⚙ 换 lucide 线性图标（与 DockNav 同源图标库）
import { Settings } from "lucide-vue-next";

import ConfirmModal from "./components/ConfirmModal.vue";
import DockNav from "./components/DockNav.vue";
import SettingsPanel from "./components/SettingsPanel.vue";
import StatsCard from "./components/StatsCard.vue";
import StatsView from "./components/StatsView.vue";
import TimerCard from "./components/TimerCard.vue";
// IPC DTO 镜像类型统一收敛在 types.ts（单一来源 = Rust serde 结构，防多处声明漂移）
import type { DaySummary, ReminderSettings, SessionStats } from "./types";
// 展示格式化共享助手（FIX002.13 收敛）
import { hhmm } from "./format";
// 提示音经 vite 打包（哈希进 dist）——不用 public/ 目录（publicDir 默认在根，曾有 404 教训）
import chimeUrl from "../assets/house_alarm-clock_loud.mp3";

// 玻璃卡片 + 拖动区沿用 PL001 阶段 B 判定形态；计时在 TimerCard，统计聚合/提醒判定在 Rust

// 统计为低频数据：挂载 + 动作后（TimerCard changed 事件）+ 30s 兜底，不进 100ms tick
const STATS_TICK_MS = 30_000;

const todaySecs = ref(0);
const weekSecs = ref(0);
const allSecs = ref(0);
// 今日工作秒（PL008.4 环口径：day_detail.work_secs，daywork 口径，随统计刷新节奏更新）
const todayWorkSecs = ref(0);
const settings = ref<ReminderSettings | null>(null);
const panelVisible = ref(false);
const reminderVisible = ref(false);
// 触发阈值取自 reminder-due 事件 payload（Rust 侧评估时的真实设置），不做前端默认值兜底
const reminderThreshold = ref(0);
// 设置保存失败的可见反馈（面板内展示，成功或重开面板时清除）
const saveError = ref("");
// 动作/打卡失败的可见反馈（FIX002.3：命令失败不再只进 console）；动作成功即清除
const actionError = ref("");
const chimeRef = ref<HTMLAudioElement | null>(null);
// PL005：打卡确认框 + 双标签视图（pill 移入 TimerCard 后，在岗态由其自有轮询驱动）
const confirmMode = ref<"in" | "out" | null>(null);
const activeTab = ref<"timer" | "stats">("timer");
const autoOutVisible = ref(false);
const autoOutAt = ref(0);
// 统计视图刷新信号：计时/打卡动作后自增，StatsView watch 重拉（保持 Rust 不推送定案）
const statsRefreshKey = ref(0);
let statsTimer: number | undefined;
let unlistenReminder: (() => void) | undefined;
let unlistenAutoOut: (() => void) | undefined;
// PL011 分态纱浓度：聚焦磨砂态 0% 纱（磨砂已足够）、失焦透明态 30% 纱（保可读）——
// 初值经 isFocused 查询兜底，此后随 Rust 的 window-focus 事件翻转
const windowFocused = ref(false);
let unlistenFocus: (() => void) | undefined;

// —— PL009.1 指针跟随高光：rAF 节流把指针写进各材质元素的 --mx/--my（元素相对坐标），
// 高光层位置随之移动；reduced-motion 用户直接跳过（动效全退避红线）——
const reducedMotionQuery = window.matchMedia("(prefers-reduced-motion: reduce)");
let pointerRaf = 0;

// —— PL010.1 拖拽修复：data-tauri-drag-region 只在"被点中元素自身"带属性时生效，
// 弹性布局铺满后 main 无裸区可点（回归 bug）——改为全局 mousedown 接线：
// 交互元素白名单命中不抢，其余一律启动窗口拖拽（点按语义不受影响）——
const DRAG_INTERACTIVE =
  "button, input, textarea, select, a, .dock, .floating-sheet, .pill, .detail-panel";

/** 非交互区按下即启动窗口拖拽 */
function onWindowDown(e: MouseEvent): void {
  if (e.button !== 0) {
    return;
  }
  const target = e.target as HTMLElement | null;
  if (target?.closest(DRAG_INTERACTIVE)) {
    return;
  }
  void getCurrentWindow().startDragging();
}

/** pointermove 节流器：一帧最多计算一次，逐材质元素换算元素相对坐标写入 CSS 变量 */
function onPointerMove(e: PointerEvent): void {
  if (pointerRaf !== 0 || reducedMotionQuery.matches) {
    return;
  }
  const { clientX, clientY } = e;
  pointerRaf = window.requestAnimationFrame(() => {
    pointerRaf = 0;
    for (const el of document.querySelectorAll<HTMLElement>(".glass-panel, .iridescent")) {
      const rect = el.getBoundingClientRect();
      el.style.setProperty("--mx", `${clientX - rect.left}px`);
      el.style.setProperty("--my", `${clientY - rect.top}px`);
    }
  });
}

/** 拉取统计快照 + 今日工作秒（环口径：day_detail.work_secs，与统计同节奏更新） */
async function refreshStats(): Promise<void> {
  try {
    const s = await invoke<SessionStats>("session_stats");
    todaySecs.value = s.today_secs;
    weekSecs.value = s.week_secs;
    allSecs.value = s.all_secs;
  } catch (err) {
    console.error("session_stats 调用失败", err);
  }
  try {
    const day = await invoke<DaySummary>("day_detail", { offset: 0 });
    todayWorkSecs.value = day.work_secs;
  } catch (err) {
    console.error("day_detail 调用失败（环口径沿用上次取值）", err);
  }
}

/** 拉取提醒设置 */
async function refreshSettings(): Promise<void> {
  try {
    settings.value = await invoke<ReminderSettings>("get_settings");
  } catch (err) {
    console.error("get_settings 调用失败", err);
  }
}

/** 提示音（sound_enabled 门控；播放失败降级仅通知——容错白名单） */
function playChime(): void {
  if (settings.value && !settings.value.sound_enabled) {
    return;
  }
  chimeRef.value?.play().catch((err) => {
    console.warn("提示音播放失败（降级仅通知）", err);
  });
}

/** 保存设置（Rust 侧校验 + 持久化 + 即时生效）：成功收起面板；失败错误态传入面板可见反馈 */
async function onSaveSettings(s: ReminderSettings): Promise<void> {
  try {
    await invoke("set_settings", { settings: s });
    settings.value = s;
    saveError.value = "";
    panelVisible.value = false;
  } catch (err) {
    // CommandError 经 IPC 序列化为文案字符串；面板内展示 + console 留痕
    saveError.value = `设置保存失败：${String(err)}`;
    console.error("set_settings 调用失败", err);
  }
}

/** ⚙ 开合面板；打开时清掉上一轮保存失败的错误提示；settings 未就绪时可见反馈不静默（FIX002.3） */
function togglePanel(): void {
  if (settings.value == null) {
    actionError.value = "设置加载失败，请重启应用重试";
    return;
  }
  panelVisible.value = !panelVisible.value;
  if (panelVisible.value) {
    saveError.value = "";
  }
}

/** TimerCard 动作后：统计即刷；动作即处理提醒（暂停/重开 = 新段），文案条与错误条随之隐藏 */
function onTimerChanged(): void {
  void refreshStats();
  statsRefreshKey.value++;
  reminderVisible.value = false;
  actionError.value = "";
}

/** 切到统计页：单日明细即时重拉（动作后的快照可能已是旧账，如开始计时当秒的工作块） */
function onTabClick(tab: "timer" | "stats"): void {
  activeTab.value = tab;
  if (tab === "stats") {
    statsRefreshKey.value++;
  }
}

/** 打卡 pill（TimerCard 上抛方向）→ 弹对应方向确认框（双向确认，不直接执行） */
function onPillClick(direction: "in" | "out"): void {
  confirmMode.value = direction;
}

/** 确认框取消：仅收起 */
function onConfirmCancel(): void {
  confirmMode.value = null;
}

/** 确认框确认：执行打卡 → 刷新在岗态与统计；失败经错误条可见（FIX002.3） */
async function onConfirmOk(): Promise<void> {
  const mode = confirmMode.value;
  confirmMode.value = null;
  if (mode == null) {
    return;
  }
  actionError.value = "";
  try {
    await invoke(mode === "in" ? "clock_in" : "clock_out");
  } catch (err) {
    actionError.value = `打卡失败：${String(err)}`;
    console.error("打卡命令调用失败", err);
  }
  // 下班成功即清提醒条（FIX002.2：提醒触发后直接下班不再滞留）
  if (mode === "out" && !actionError.value) {
    reminderVisible.value = false;
  }
  void refreshStats();
  statsRefreshKey.value++;
}

onMounted(() => {
  void refreshStats();
  void refreshSettings();
  statsTimer = window.setInterval(() => void refreshStats(), STATS_TICK_MS);
  window.addEventListener("pointermove", onPointerMove);
  window.addEventListener("mousedown", onWindowDown);
  // 焦点态初值兜底：错过启动期事件也不至于滞留错误纱浓度（查询失败仅记录，退回默认纱态）
  getCurrentWindow()
    .isFocused()
    .then((focused) => {
      windowFocused.value = focused;
    })
    .catch((err) => console.error("isFocused 查询失败", err));
  // window-focus：Rust Focused 事件转发（payload = 聚焦与否），驱动分态纱与 focused class
  listen<boolean>("window-focus", (event) => {
    windowFocused.value = event.payload;
  })
    .then((un) => {
      unlistenFocus = un;
    })
    .catch((err) => console.error("window-focus 监听注册失败", err));
  // reminder-due：Rust 侧评估触发（payload = 触发时的真实阈值分钟数，直显文案条）；注册失败必须可见
  listen<number>("reminder-due", (event) => {
    reminderThreshold.value = event.payload;
    reminderVisible.value = true;
    playChime();
  })
    .then((un) => {
      unlistenReminder = un;
    })
    .catch((err) => console.error("reminder-due 监听注册失败", err));
  // workday-auto-out：payload = 回填下班时刻（上班 + N），文案条告知 + 界面即刷
  listen<number>("workday-auto-out", (event) => {
    autoOutAt.value = event.payload;
    autoOutVisible.value = true;
    void refreshStats();
    statsRefreshKey.value++;
  })
    .then((un) => {
      unlistenAutoOut = un;
    })
    .catch((err) => console.error("workday-auto-out 监听注册失败", err));
});

onUnmounted(() => {
  if (statsTimer !== undefined) {
    window.clearInterval(statsTimer);
  }
  window.removeEventListener("pointermove", onPointerMove);
  window.removeEventListener("mousedown", onWindowDown);
  if (pointerRaf !== 0) {
    window.cancelAnimationFrame(pointerRaf);
  }
  unlistenReminder?.();
  unlistenAutoOut?.();
  unlistenFocus?.();
});
</script>

<template>
  <main class="glass-card" :class="{ focused: windowFocused }">
    <div class="topbar">
      <h1 class="title">CapsulePulse</h1>
      <button class="gear" type="button" title="设置" @click="togglePanel">
        <Settings :size="16" :stroke-width="2.2" aria-hidden="true" />
      </button>
    </div>
    <p v-if="reminderVisible" class="reminder">
      已连续工作 {{ reminderThreshold }} 分钟，休息一下吧
    </p>
    <p v-if="autoOutVisible" class="reminder auto-out">已于 {{ hhmm(autoOutAt) }} 自动下班</p>
    <p v-if="actionError" class="reminder action-error" role="alert">{{ actionError }}</p>
    <StatsCard :today-secs="todaySecs" :week-secs="weekSecs" :all-secs="allSecs" />
    <!-- 双标签均 v-show 保活：TimerCard 的 100ms tick 是提醒/自动下班评估口，切页不得中断 -->
    <div v-show="activeTab === 'timer'" class="timer-pane">
      <TimerCard
        :work-secs="todayWorkSecs"
        :target-hours="settings?.workday_auto_out_hours ?? null"
        @changed="onTimerChanged"
        @error="actionError = $event"
        @clock="onPillClick"
      />
    </div>
    <StatsView v-show="activeTab === 'stats'" :refresh-key="statsRefreshKey" />
    <DockNav :active="activeTab" @change="onTabClick" />
    <Transition name="sheet">
      <div v-if="panelVisible && settings" class="overlay" @click.self="panelVisible = false">
        <section class="floating-sheet iridescent" role="dialog" aria-label="设置">
          <SettingsPanel :settings="settings" :error="saveError" @save="onSaveSettings" />
        </section>
      </div>
    </Transition>
  </main>
  <ConfirmModal
    :open="confirmMode != null"
    :title="confirmMode === 'in' ? '上班打卡' : '下班打卡'"
    :message="confirmMode === 'in' ? '开始一天工作吗？' : '结束一天工作吗？'"
    @confirm="onConfirmOk"
    @cancel="onConfirmCancel"
  />
  <audio ref="chimeRef" :src="chimeUrl" preload="auto"></audio>
</template>

<style>
/* —— PL010 设计令牌（全局唯一来源）：所有组件经 var() 消费，玻璃配方全应用只此一份。
   真实玻璃材质：高透薄纱体色 + 亮边定义形状 + 顶缘 rim + 落影；磨砂由 Mica 承担 —— */
:root {
  --accent: #7c3aed;
  --mint: #0f766e;
  --mint-bright: #34d399;
  --ink: #1d1d1f;
  --ink-2: color-mix(in srgb, #1d1d1f 55%, transparent);
  --font-stack: "SF Pro Display", "Segoe UI Variable Display", "Segoe UI", sans-serif;
  --glass-bg: rgba(255, 255, 255, 0.3);
  --glass-stroke: inset 0 0 0 1.5px rgba(255, 255, 255, 0.78);
  --panel-bg: rgba(255, 255, 255, 0.38);
  --panel-stroke: inset 0 0 0 1px rgba(255, 255, 255, 0.65);
  --panel-cast: rgba(90, 70, 140, 0.1);
  --btn-cast: rgba(90, 70, 140, 0.35);
  --text-shadow: none;
  --chip-bg: rgba(255, 255, 255, 0.45);
  --glass-blur: blur(28px) saturate(1.6);
  --glass-highlight:
    inset 0 1px rgba(255, 255, 255, 0.35), inset 0 0 0 0.5px rgba(255, 255, 255, 0.16);
  --rim-light: inset 0 1.5px 0 rgba(255, 255, 255, 0.9);
  --edge-glow: 0 0 0 1px rgba(255, 255, 255, 0.55), 0 2px 12px rgba(139, 92, 246, 0.25);
  --shadow-candy: 0 16px 40px rgba(80, 60, 120, 0.25);
  --grad-primary: linear-gradient(165deg, #8a5cff 0%, #7448f5 55%, #6a3ae8 100%);
  --grad-digit: linear-gradient(165deg, #7c3aed 0%, #5b21b6 100%);
  --grad-mint: linear-gradient(165deg, #34d399 0%, #10b981 100%);
  --iridescent: linear-gradient(135deg, #fcd9ed 0%, #e1defe 45%, #b7f5fc 100%);
  --r-card: 20px;
  --r-ctrl: 13px;
  --r-pill: 999px;
  --r-sheet: 24px;
  --ease-spring: cubic-bezier(0.34, 1.56, 0.64, 1);
}

@media (prefers-color-scheme: dark) {
  :root {
    /* 暗夜衍生：同一配方的低透深纱版（浅字 + 暗投影保对比），磨砂仍由 Mica 承担 */
    --accent: #c0b0fd;
    --mint: #5eead4;
    --mint-bright: #2dd4bf;
    --ink: #f5f5f7;
    --ink-2: color-mix(in srgb, #f5f5f7 55%, transparent);
    --glass-bg: rgba(0, 0, 0, 0.3);
    --glass-stroke: inset 0 0 0 1.5px rgba(255, 255, 255, 0.28);
    --panel-bg: rgba(255, 255, 255, 0.07);
    --panel-stroke: inset 0 0 0 1px rgba(255, 255, 255, 0.14);
    --panel-cast: rgba(0, 0, 0, 0.3);
    --btn-cast: rgba(0, 0, 0, 0.45);
    --text-shadow: 0 1px 3px rgba(0, 0, 0, 0.4);
    --chip-bg: rgba(255, 255, 255, 0.1);
    --glass-highlight:
      inset 0 1px rgba(255, 255, 255, 0.12), inset 0 0 0 0.5px rgba(255, 255, 255, 0.1);
    --rim-light: inset 0 1.5px 0 rgba(255, 255, 255, 0.32);
    --edge-glow: 0 0 0 1px rgba(255, 255, 255, 0.2), 0 2px 16px rgba(167, 139, 250, 0.4);
    --shadow-candy: 0 16px 40px rgba(0, 0, 0, 0.5);
    --grad-primary: linear-gradient(165deg, #7a5cf0 0%, #5b3fd6 55%, #4c2fb8 100%);
    --grad-digit: linear-gradient(165deg, #d8c7ff 0%, #a78bfa 100%);
    --grad-mint: linear-gradient(165deg, #0c8a60 0%, #075e42 100%);
    --iridescent: linear-gradient(
      135deg,
      rgba(52, 36, 86, 0.92) 0%,
      rgba(38, 28, 64, 0.94) 45%,
      rgba(22, 52, 66, 0.92) 100%
    );
  }
}

/* —— 糖果材质工具类（PL007.1）：半径由消费方自定，材质配方收敛于此 ——
   .glass-panel 玻璃面板（PL008 布局卡的底材）/ .glass-chip 胶囊小件 / .iridescent 虹彩浮层 */
.glass-panel {
  background: var(--panel-bg);
  box-shadow: var(--panel-stroke), var(--rim-light), var(--panel-cast);
}

.glass-chip {
  background: var(--chip-bg);
  box-shadow: var(--rim-light), var(--glass-highlight);
}

.iridescent {
  background: var(--iridescent);
  backdrop-filter: var(--glass-blur);
  box-shadow: var(--rim-light), var(--edge-glow), var(--shadow-candy);
}

/* —— PL009.1 指针跟随高光：材质元素 ::after 叠加 radial 高光层，位置由 JS 写入的
   --mx/--my（元素相对坐标）驱动，悬停时浮现、随指针移动 —— */
.glass-panel,
.iridescent {
  position: relative;
}

.glass-panel::after,
.iridescent::after {
  content: "";
  position: absolute;
  inset: 0;
  border-radius: inherit;
  background: radial-gradient(
    200px circle at var(--mx, 50%) var(--my, 0%),
    rgba(255, 255, 255, 0.16),
    transparent 65%
  );
  opacity: 0;
  transition: opacity 0.3s ease;
  pointer-events: none;
}

.glass-panel:hover::after,
.iridescent:hover::after {
  opacity: 1;
}

/* —— PL009.2 环境光呼吸：虹彩面渐变位 8s 缓移（放大画布再平移），环境光"活"感；
   reduced-motion 由下方显式退避（无限动画不能只靠全局时长归零）—— */
.iridescent {
  background-size: 200% 200%;
  animation: iridescent-breathe 8s ease-in-out infinite;
}

@keyframes iridescent-breathe {
  0% {
    background-position: 0% 0%;
  }

  50% {
    background-position: 100% 100%;
  }

  100% {
    background-position: 0% 0%;
  }
}

/* —— 浮层共用形态：确认框与设置面板同规格（居中玻璃片 + 压暗遮罩）—— */
.overlay {
  position: fixed;
  inset: 0;
  z-index: 20;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(0, 0, 0, 0.25);
  backdrop-filter: blur(6px);
}

.floating-sheet {
  display: flex;
  flex-direction: column;
  gap: 14px;
  width: 240px;
  padding: 20px;
  border-radius: var(--r-sheet);
  /* 材质（虹彩底 + 轮廓光 + 彩色泛光 + 糖果投影）由全局 .iridescent 提供（PL007.6） */
  color: var(--ink);
  /* 确认框是 .glass-card 的兄弟节点，须显式继承展示级字族（FIX002：字体脱管修复） */
  font-family: var(--font-stack);
  user-select: none;
}

/* —— PL010 按钮厚度三件套（配方表）：白顶光层 + 底缘暗线 + 落影 = 立体玻璃圆柱；
   主 = 紫渐变体色、次 = 薄荷渐变体色 —— */
.btn-primary {
  border: none;
  border-radius: var(--r-pill);
  background:
    linear-gradient(180deg, rgba(255, 255, 255, 0.28), rgba(255, 255, 255, 0) 38%),
    var(--grad-primary);
  box-shadow:
    var(--rim-light),
    inset 0 -2px 0 rgba(0, 0, 0, 0.18),
    0 8px 20px var(--btn-cast);
  color: #fff;
  font-family: var(--font-stack);
  cursor: pointer;
  transition:
    filter 0.15s ease,
    transform 0.15s ease;
}

.btn-primary:hover {
  filter: brightness(1.08);
}

.btn-primary:active {
  filter: brightness(0.95);
  transform: scale(0.96);
}

.btn-ghost {
  border: 1px solid color-mix(in srgb, var(--mint) 45%, transparent);
  border-radius: var(--r-pill);
  background:
    linear-gradient(180deg, rgba(255, 255, 255, 0.28), rgba(255, 255, 255, 0) 38%), var(--grad-mint);
  box-shadow:
    var(--rim-light),
    inset 0 -2px 0 rgba(0, 0, 0, 0.14),
    0 6px 16px var(--btn-cast);
  color: inherit;
  font-family: var(--font-stack);
  cursor: pointer;
  transition:
    filter 0.15s ease,
    transform 0.15s ease;
}

.btn-ghost:hover {
  filter: brightness(1.05) saturate(1.15);
}

.btn-ghost:active {
  filter: brightness(0.95);
  transform: scale(0.96);
}

/* —— PL009.3 微交互：按压涟漪（中心 radial 扩散一次）；按钮相对定位 + 裁切 —— */
.btn-primary,
.btn-ghost {
  position: relative;
  overflow: hidden;
}

.btn-primary::after,
.btn-ghost::after {
  content: "";
  position: absolute;
  inset: 0;
  border-radius: inherit;
  background: radial-gradient(circle, rgba(255, 255, 255, 0.35), transparent 65%);
  opacity: 0;
  transform: scale(0.5);
  pointer-events: none;
}

.btn-primary:active::after,
.btn-ghost:active::after {
  opacity: 1;
  transform: scale(1);
  transition:
    transform 0.25s ease-out,
    opacity 0.25s ease-out;
}

/* 置灰（未上班）：不可点击且视觉降级，双主题可辨识；糖果光效一并退场 */
.btn-primary:disabled,
.btn-ghost:disabled {
  background: rgba(128, 128, 128, 0.14);
  border-color: rgba(128, 128, 128, 0.22);
  box-shadow: none;
  color: inherit;
  opacity: 0.45;
  cursor: not-allowed;
}

.sheet-enter-active,
.sheet-leave-active {
  transition: opacity 0.15s ease;
}

.sheet-enter-active .floating-sheet {
  transition: transform 0.15s var(--ease-spring);
}

.sheet-enter-from,
.sheet-leave-to {
  opacity: 0;
}

.sheet-enter-from .floating-sheet {
  transform: scale(0.92);
}

@media (prefers-color-scheme: dark) {
  .overlay {
    background: rgba(0, 0, 0, 0.35);
  }
}

/* 动效退避：系统开启"减弱动态效果"时全部退化为直切；
   无限循环的呼吸动画显式关闭（时长归零对 infinite 动画无意义），
   指针高光层直接不渲染（PL009 红线：reduced-motion 全退避） */
@media (prefers-reduced-motion: reduce) {
  .iridescent {
    animation: none;
  }

  .glass-panel::after,
  .iridescent::after {
    content: none;
  }

  *,
  *::before,
  *::after {
    transition-duration: 0.01ms !important;
    animation-duration: 0.01ms !important;
  }
}
</style>

<style scoped>
/* 玻璃板（PL010.8 全窗单层收敛 / PL011 焦点联动材质）：聚焦 = DWM Acrylic 背板真磨砂（0% 纱），
   失焦 = 纯 alpha 透明 + 30% 纱（浅白/暗黑双主题，保可读）
   + 亮边 + rim + 落影；内容呼吸内缩（padding 24/20）；8px 圆角对齐系统窗口圆角 */
.glass-card {
  position: relative;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 10px;
  width: 100%;
  height: 100vh;
  padding: 24px 20px;
  box-sizing: border-box;
  border-radius: 8px;
  box-shadow: var(--glass-stroke), var(--rim-light), var(--shadow-candy);
  overflow: hidden;
  text-shadow: var(--text-shadow);
  user-select: none;
  color: var(--ink);
  font-family: var(--font-stack);
}

/* 纱体色层：叠在 DWM 背板/透明底之上、内容之下（体色即玻璃自身材质的一部分）；
   分态浓度（PL011 用户定案）——失焦透明态 30% 纱保可读，聚焦磨砂态退 0%（磨砂已足够） */
.glass-card::before {
  content: "";
  position: absolute;
  inset: 0;
  z-index: 1;
  border-radius: inherit;
  background: var(--glass-bg);
  pointer-events: none;
}

.glass-card.focused::before {
  background: transparent;
}

/* 直接子件一律浮于体色层之上（.overlay 是 fixed 全屏遮罩，排除） */
.glass-card > :not(.overlay) {
  position: relative;
  z-index: 1;
}

.topbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  width: 100%;
  padding: 0 4px;
}

.title {
  font-size: 15px;
  font-weight: 600;
  letter-spacing: 0.3px;
  margin: 0;
  opacity: 0.8;
  cursor: default;
}

.gear {
  display: flex;
  border: none;
  background: transparent;
  color: inherit;
  cursor: pointer;
  opacity: 0.55;
}

.gear:hover {
  opacity: 0.9;
}

/* 提醒/自动下班胶囊条：tint 着色（warn 琥珀 / good 绿），插入时滑入 */
.reminder {
  margin: 0;
  padding: 6px 16px;
  border-radius: var(--r-pill);
  background: rgba(255, 159, 10, 0.22);
  font-size: 13px;
  animation: banner-in 0.18s ease;
}

/* 自动下班条（PL007.6）：绿 → 薄荷渐变半透明（统一糖果色板）；提醒条琥珀保留原样 */
.reminder.auto-out {
  background: linear-gradient(135deg, rgba(52, 211, 153, 0.35) 0%, rgba(110, 231, 183, 0.3) 100%);
}

/* 动作/打卡失败文案条（FIX002.3）：红 tint，命令失败对用户可见 */
.reminder.action-error {
  background: rgba(179, 38, 30, 0.28);
}

@keyframes banner-in {
  from {
    opacity: 0;
    transform: translateY(-8px);
  }

  to {
    opacity: 1;
    transform: translateY(0);
  }
}

@media (prefers-color-scheme: dark) {
  .reminder.auto-out {
    background: linear-gradient(135deg, rgba(16, 185, 129, 0.35) 0%, rgba(13, 148, 136, 0.3) 100%);
  }
}

/* 计时页容器（PL008 定底布局）：吃掉 dock 以上全部富余高度，内容纵向居中 */
.timer-pane {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 14px;
  width: 100%;
  flex: 1;
  min-height: 0;
}

/* 打卡 pill 蒙皮随组件迁移 TimerCard（PL008.4），此处不再有 pill 样式 */
</style>
