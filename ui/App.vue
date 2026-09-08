<script setup lang="ts">
import { onMounted, onUnmounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";

import StatsCard from "./components/StatsCard.vue";
import TimerCard from "./components/TimerCard.vue";

// 玻璃卡片 + 拖动区沿用 PL001 阶段 B 判定通过的形态（G1–G4）；计时在 TimerCard，统计聚合在 Rust

/** stats 命令返回体（镜像 Rust 侧 SessionStats serde 结构，单一来源在 Rust） */
interface SessionStats {
  today_secs: number;
  week_secs: number;
  all_secs: number;
}

// 统计为低频数据：挂载 + 动作后（TimerCard changed 事件）+ 30s 兜底，不进 100ms tick
const STATS_TICK_MS = 30_000;

const todaySecs = ref(0);
const weekSecs = ref(0);
const allSecs = ref(0);
let statsTimer: number | undefined;

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

onMounted(() => {
  void refreshStats();
  statsTimer = window.setInterval(() => void refreshStats(), STATS_TICK_MS);
});

onUnmounted(() => {
  if (statsTimer !== undefined) {
    window.clearInterval(statsTimer);
  }
});
</script>

<template>
  <main class="glass-card" data-tauri-drag-region>
    <h1 class="title" data-tauri-drag-region>CapsulePulse</h1>
    <StatsCard :today-secs="todaySecs" :week-secs="weekSecs" :all-secs="allSecs" />
    <TimerCard @changed="refreshStats" />
  </main>
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

.title {
  font-size: 22px;
  font-weight: 600;
  margin: 0;
  opacity: 0.75;
  cursor: default;
}

/* 深浅色跟随系统（G3）：双主题下文字与卡片底均保持可读 */
@media (prefers-color-scheme: dark) {
  .glass-card {
    background: rgba(30, 30, 30, 0.35);
    color: #e8e8e8;
  }
}
</style>
