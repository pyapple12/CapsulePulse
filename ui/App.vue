<script setup lang="ts">
import { onMounted, onUnmounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import SettingsPanel from "./components/SettingsPanel.vue";
import StatsCard from "./components/StatsCard.vue";
import TimerCard from "./components/TimerCard.vue";
// 提示音经 vite 打包（哈希进 dist）——不用 public/ 目录（publicDir 默认在根，曾有 404 教训）
import chimeUrl from "../assets/house_alarm-clock_loud.mp3";

// 玻璃卡片 + 拖动区沿用 PL001 阶段 B 判定形态；计时在 TimerCard，统计聚合/提醒判定在 Rust

/** stats 命令返回体（镜像 Rust 侧 SessionStats serde 结构，单一来源在 Rust） */
interface SessionStats {
  today_secs: number;
  week_secs: number;
  all_secs: number;
}

/** get/set_settings 命令返回体（镜像 Rust 侧 ReminderSettings serde 结构） */
interface ReminderSettings {
  threshold_min: number;
  sound_enabled: boolean;
  notify_enabled: boolean;
}

// 统计为低频数据：挂载 + 动作后（TimerCard changed 事件）+ 30s 兜底，不进 100ms tick
const STATS_TICK_MS = 30_000;

const todaySecs = ref(0);
const weekSecs = ref(0);
const allSecs = ref(0);
const settings = ref<ReminderSettings | null>(null);
const panelVisible = ref(false);
const reminderVisible = ref(false);
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

/** 保存设置（Rust 侧校验 + 持久化 + 即时生效），成功后收起面板 */
async function onSaveSettings(s: ReminderSettings): Promise<void> {
  try {
    await invoke("set_settings", { settings: s });
    settings.value = s;
    panelVisible.value = false;
  } catch (err) {
    console.error("set_settings 调用失败", err);
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
  // reminder-due：Rust 侧评估触发（payload = 阈值分钟数）；注册失败必须可见（ACL/事件教训）
  listen("reminder-due", () => {
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
      <button class="gear" type="button" title="设置" @click="panelVisible = !panelVisible">
        ⚙
      </button>
    </div>
    <p v-if="reminderVisible" class="reminder">
      已连续工作 {{ settings?.threshold_min ?? 50 }} 分钟，休息一下吧
    </p>
    <StatsCard :today-secs="todaySecs" :week-secs="weekSecs" :all-secs="allSecs" />
    <TimerCard @changed="onTimerChanged" />
    <SettingsPanel v-if="panelVisible && settings" :settings="settings" @save="onSaveSettings" />
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
