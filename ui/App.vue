<script setup lang="ts">
import { onMounted, onUnmounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import ConfirmModal from "./components/ConfirmModal.vue";
import SettingsPanel from "./components/SettingsPanel.vue";
import StatsCard from "./components/StatsCard.vue";
import StatsView from "./components/StatsView.vue";
import TimerCard from "./components/TimerCard.vue";
// IPC DTO 镜像类型统一收敛在 types.ts（单一来源 = Rust serde 结构，防多处声明漂移）
import type { ReminderSettings, SessionStats, SessionStatus } from "./types";
// 提示音经 vite 打包（哈希进 dist）——不用 public/ 目录（publicDir 默认在根，曾有 404 教训）
import chimeUrl from "../assets/house_alarm-clock_loud.mp3";

// 玻璃卡片 + 拖动区沿用 PL001 阶段 B 判定形态；计时在 TimerCard，统计聚合/提醒判定在 Rust

// 统计为低频数据：挂载 + 动作后（TimerCard changed 事件）+ 30s 兜底，不进 100ms tick
const STATS_TICK_MS = 30_000;

const todaySecs = ref(0);
const weekSecs = ref(0);
const allSecs = ref(0);
const settings = ref<ReminderSettings | null>(null);
const panelVisible = ref(false);
const reminderVisible = ref(false);
// 触发阈值取自 reminder-due 事件 payload（Rust 侧评估时的真实设置），不做前端默认值兜底
const reminderThreshold = ref(0);
// 设置保存失败的可见反馈（面板内展示，成功或重开面板时清除）
const saveError = ref("");
const chimeRef = ref<HTMLAudioElement | null>(null);
// PL005：在岗态 + 打卡确认框 + 双标签视图
const onDuty = ref(false);
const confirmMode = ref<"in" | "out" | null>(null);
const activeTab = ref<"timer" | "stats">("timer");
const autoOutVisible = ref(false);
const autoOutAt = ref(0);
// 统计视图刷新信号：计时/打卡动作后自增，StatsView watch 重拉（保持 Rust 不推送定案）
const statsRefreshKey = ref(0);
let statsTimer: number | undefined;
let unlistenReminder: (() => void) | undefined;
let unlistenAutoOut: (() => void) | undefined;

/** 拉取统计快照 */
async function refreshStats(): Promise<void> {
  try {
    const s = await invoke<SessionStats>("session_stats");
    todaySecs.value = s.today_secs;
    weekSecs.value = s.week_secs;
    allSecs.value = s.all_secs;
  } catch (err) {
    console.error("session_stats 调用失败", err);
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

/** 拉取在岗态（挂载 + 打卡动作后刷新；日常翻转由确认框动作与自动下班事件驱动） */
async function refreshDuty(): Promise<void> {
  try {
    const s = await invoke<SessionStatus>("session_status");
    onDuty.value = s.on_duty;
  } catch (err) {
    console.error("session_status 调用失败", err);
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

/** ⚙ 开合面板；打开时清掉上一轮保存失败的错误提示 */
function togglePanel(): void {
  panelVisible.value = !panelVisible.value;
  if (panelVisible.value) {
    saveError.value = "";
  }
}

/** TimerCard 动作后：统计即刷；动作即处理提醒（暂停/重开 = 新段），文案条随之隐藏 */
function onTimerChanged(): void {
  void refreshStats();
  statsRefreshKey.value++;
  reminderVisible.value = false;
}

/** 切到统计页：单日明细即时重拉（动作后的快照可能已是旧账，如开始计时当秒的工作块） */
function onTabClick(tab: "timer" | "stats"): void {
  activeTab.value = tab;
  if (tab === "stats") {
    statsRefreshKey.value++;
  }
}

/** 打卡 pill 点击 → 弹对应方向确认框（双向确认，不直接执行） */
function onPillClick(): void {
  confirmMode.value = onDuty.value ? "out" : "in";
}

/** 确认框取消：仅收起 */
function onConfirmCancel(): void {
  confirmMode.value = null;
}

/** 确认框确认：执行打卡 → 刷新在岗态与统计 */
async function onConfirmOk(): Promise<void> {
  const mode = confirmMode.value;
  confirmMode.value = null;
  if (mode == null) {
    return;
  }
  try {
    await invoke(mode === "in" ? "clock_in" : "clock_out");
  } catch (err) {
    console.error("打卡命令调用失败", err);
  }
  await refreshDuty();
  void refreshStats();
  statsRefreshKey.value++;
}

/** Unix 秒 → 本地 HH:MM（自动下班文案条） */
function hhmm(secs: number): string {
  const d = new Date(secs * 1000);
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

onMounted(() => {
  void refreshStats();
  void refreshSettings();
  void refreshDuty();
  statsTimer = window.setInterval(() => void refreshStats(), STATS_TICK_MS);
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
    void refreshDuty();
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
  unlistenReminder?.();
  unlistenAutoOut?.();
});
</script>

<template>
  <main class="glass-card" data-tauri-drag-region>
    <div class="topbar">
      <h1 class="title" data-tauri-drag-region>CapsulePulse</h1>
      <button class="gear" type="button" title="设置" @click="togglePanel">⚙</button>
    </div>
    <p v-if="reminderVisible" class="reminder">
      已连续工作 {{ reminderThreshold }} 分钟，休息一下吧
    </p>
    <p v-if="autoOutVisible" class="reminder auto-out">已于 {{ hhmm(autoOutAt) }} 自动下班</p>
    <div class="tabs" role="tablist">
      <span class="tabs-thumb" :class="{ right: activeTab === 'stats' }" aria-hidden="true"></span>
      <button
        class="tab"
        :class="{ active: activeTab === 'timer' }"
        type="button"
        role="tab"
        :aria-selected="activeTab === 'timer'"
        @click="activeTab = 'timer'"
      >
        计时
      </button>
      <button
        class="tab"
        :class="{ active: activeTab === 'stats' }"
        type="button"
        role="tab"
        :aria-selected="activeTab === 'stats'"
        @click="onTabClick('stats')"
      >
        统计
      </button>
    </div>
    <StatsCard :today-secs="todaySecs" :week-secs="weekSecs" :all-secs="allSecs" />
    <!-- 双标签均 v-show 保活：TimerCard 的 100ms tick 是提醒/自动下班评估口，切页不得中断 -->
    <div v-show="activeTab === 'timer'" class="timer-pane">
      <button class="pill" :class="{ active: onDuty }" type="button" @click="onPillClick">
        {{ onDuty ? "下班" : "上班" }}
      </button>
      <TimerCard @changed="onTimerChanged" />
    </div>
    <StatsView v-show="activeTab === 'stats'" :refresh-key="statsRefreshKey" />
    <Transition name="sheet">
      <div v-if="panelVisible && settings" class="overlay" @click.self="panelVisible = false">
        <section class="floating-sheet" role="dialog" aria-label="设置">
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
/* —— PL006 设计令牌（全局唯一来源）：所有组件经 var() 消费，玻璃配方全应用只此一份 —— */
:root {
  --accent: #0071e3;
  --ink: #1d1d1f;
  --ink-2: color-mix(in srgb, #1d1d1f 55%, transparent);
  --font-stack: "SF Pro Display", "Segoe UI Variable Display", "Segoe UI", sans-serif;
  --glass-bg: rgba(255, 255, 255, 0.55);
  --glass-blur: blur(28px) saturate(1.6);
  --glass-highlight:
    inset 0 1px rgba(255, 255, 255, 0.35), inset 0 0 0 0.5px rgba(255, 255, 255, 0.16);
  --shadow-float: 0 8px 24px rgba(0, 0, 0, 0.18);
  --r-card: 20px;
  --r-ctrl: 13px;
  --r-pill: 999px;
  --r-sheet: 24px;
  --ease-spring: cubic-bezier(0.34, 1.56, 0.64, 1);
}

@media (prefers-color-scheme: dark) {
  :root {
    --accent: #0a84ff;
    --ink: #f5f5f7;
    --ink-2: color-mix(in srgb, #f5f5f7 55%, transparent);
    --glass-bg: rgba(30, 30, 30, 0.45);
    --glass-highlight:
      inset 0 1px rgba(255, 255, 255, 0.12), inset 0 0 0 0.5px rgba(255, 255, 255, 0.1);
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
  background: var(--glass-bg);
  backdrop-filter: var(--glass-blur);
  box-shadow: var(--glass-highlight), var(--shadow-float);
  color: var(--ink);
  user-select: none;
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

/* 动效退避：系统开启"减弱动态效果"时全部退化为直切 */
@media (prefers-reduced-motion: reduce) {
  *,
  *::before,
  *::after {
    transition-duration: 0.01ms !important;
    animation-duration: 0.01ms !important;
  }
}
</style>

<style scoped>
/* 玻璃卡片：四周留 12px 露出 Acrylic 底；本体 = 唯一玻璃配方（令牌）+ 高光内描边 */
.glass-card {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 16px;
  height: calc(100vh - 24px);
  margin: 12px;
  border-radius: var(--r-card);
  background: var(--glass-bg);
  backdrop-filter: var(--glass-blur);
  box-shadow: var(--glass-highlight);
  user-select: none;
  color: var(--ink);
  font-family: var(--font-stack);
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
  border: none;
  background: transparent;
  color: inherit;
  font-size: 16px;
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

.reminder.auto-out {
  background: rgba(48, 209, 88, 0.22);
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

/* iOS 式分段控件：玻璃胶囊轨道 + 白色滑块随 activeTab 位移 */
.tabs {
  position: relative;
  display: grid;
  grid-template-columns: 1fr 1fr;
  width: 200px;
  padding: 3px;
  border-radius: var(--r-pill);
  background: rgba(120, 120, 128, 0.16);
}

.tabs-thumb {
  position: absolute;
  top: 3px;
  left: 3px;
  width: calc(50% - 3px);
  height: calc(100% - 6px);
  border-radius: var(--r-pill);
  background: rgba(255, 255, 255, 0.75);
  box-shadow: 0 1px 4px rgba(0, 0, 0, 0.12);
  transition: transform 0.22s var(--ease-spring);
}

.tabs-thumb.right {
  transform: translateX(100%);
}

.tab {
  position: relative;
  z-index: 1;
  padding: 5px 0;
  border: none;
  border-radius: var(--r-pill);
  background: transparent;
  color: var(--ink-2);
  font-size: 13px;
  cursor: pointer;
  transition: color 0.2s ease;
}

.tab.active {
  color: var(--ink);
  font-weight: 600;
}

@media (prefers-color-scheme: dark) {
  .tabs-thumb {
    background: rgba(255, 255, 255, 0.22);
  }
}

/* 计时页容器：打卡 pill + 计时卡片 */
.timer-pane {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 14px;
  width: 100%;
}

/* 打卡 pill：未上班 = accent 描边可按态；在岗中 = accent 着色玻璃 + 内凹高光（视觉常驻"已按下"） */
.pill {
  padding: 8px 36px;
  border: 1px solid color-mix(in srgb, var(--accent) 75%, transparent);
  border-radius: var(--r-pill);
  background: transparent;
  color: var(--accent);
  font-size: 14px;
  font-weight: 600;
  letter-spacing: 2px;
  cursor: pointer;
  transition: all 0.15s ease;
}

.pill:hover {
  background: color-mix(in srgb, var(--accent) 12%, transparent);
}

.pill:active {
  transform: scale(0.96);
}

.pill.active {
  border-color: transparent;
  background: color-mix(in srgb, var(--accent) 30%, transparent);
  backdrop-filter: var(--glass-blur);
  color: var(--ink);
  box-shadow:
    var(--glass-highlight),
    inset 0 2px 6px rgba(0, 0, 0, 0.18);
}
</style>
