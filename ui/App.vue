<script setup lang="ts">
import { onMounted, onUnmounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import SettingsPanel from "./components/SettingsPanel.vue";
import StatsCard from "./components/StatsCard.vue";
import TimerCard from "./components/TimerCard.vue";
// IPC DTO 镜像类型统一收敛在 types.ts（单一来源 = Rust serde 结构，防多处声明漂移）
import type { ReminderSettings, SessionStats } from "./types";
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
let statsTimer: number | undefined;
let unlistenReminder: (() => void) | undefined;

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
  reminderVisible.value = false;
}

onMounted(() => {
  void refreshStats();
  void refreshSettings();
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
});

onUnmounted(() => {
  if (statsTimer !== undefined) {
    window.clearInterval(statsTimer);
  }
  unlistenReminder?.();
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
    <StatsCard :today-secs="todaySecs" :week-secs="weekSecs" :all-secs="allSecs" />
    <TimerCard @changed="onTimerChanged" />
    <SettingsPanel
      v-if="panelVisible && settings"
      :settings="settings"
      :error="saveError"
      @save="onSaveSettings"
    />
  </main>
  <audio ref="chimeRef" :src="chimeUrl" preload="auto"></audio>
</template>

<style scoped>
/* 玻璃卡片：四周留 12px 露出 Acrylic 底（G1 透明可见），卡片本体半透明 + backdrop-filter 叠加模糊（G2） */
.glass-card {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 16px;
  height: calc(100vh - 24px);
  margin: 12px;
  border-radius: 16px;
  background: rgba(255, 255, 255, 0.35);
  backdrop-filter: blur(24px);
  user-select: none;
  color: #1f2328;
}

.topbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  width: 100%;
  padding: 0 4px;
}

.title {
  font-size: 22px;
  font-weight: 600;
  margin: 0;
  opacity: 0.75;
  cursor: default;
}

.gear {
  border: none;
  background: transparent;
  color: inherit;
  font-size: 16px;
  cursor: pointer;
  opacity: 0.7;
}

.gear:hover {
  opacity: 1;
}

/* 提醒文案条：达阈值后的卡片内提示（通知/声音之外的第三通道） */
.reminder {
  margin: 0;
  padding: 6px 14px;
  border-radius: 8px;
  background: rgba(255, 193, 7, 0.25);
  font-size: 13px;
}

/* 深浅色跟随系统（G3）：双主题下文字与卡片底均保持可读 */
@media (prefers-color-scheme: dark) {
  .glass-card {
    background: rgba(30, 30, 30, 0.35);
    color: #e8e8e8;
  }
}
</style>
