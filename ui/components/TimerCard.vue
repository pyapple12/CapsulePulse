<script setup lang="ts">
import { onMounted, onUnmounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";

// IPC DTO 镜像类型统一在 types.ts（单一来源 = Rust serde 结构）
import type { SessionStatus } from "../types";
// 展示格式化共享助手（FIX002.13 收敛）
import { pad } from "../format";
// PL008.4：进度环（定稿线框主体），数字经插槽居环中
import ProgressRing from "./ProgressRing.vue";

// 轮询周期 100ms：十分秒位（HH:MM:SS.d）每 0.1s 跳动需要 ≤100ms 拉取；
// 10 次/s 本地 IPC 开销可忽略。（演进：1s→250ms 修秒进位迟到，2026-09-08 用户定案改十分秒位后→100ms）
const TICK_MS = 100;

// 动作（开始/暂停/继续/重开）后通知父组件刷新统计行；失败上抛文案（FIX002.3 可见反馈）；
// clock：打卡请求上抛（PL008.4 pill 移入本组件，确认框仍由父组件持有）
const emit = defineEmits<{
  changed: [];
  error: [string];
  clock: [direction: "in" | "out"];
}>();

// 环口径数据由父组件下发（day_detail.work_secs 与自动下班小时数，PL008.4）
defineProps<{ workSecs: number; targetHours: number | null }>();

const state = ref<"idle" | "running" | "paused">("idle");
const totalMs = ref(0);
// 在岗态（PL005）：未上班时计时按钮置灰禁用 + pill 方向 + 环虚线置灰
const onDuty = ref(false);
let timer: number | undefined;

/** 拉取会话快照（tick 定案：前端 setInterval 拉取，Rust 不推送） */
async function refresh(): Promise<void> {
  try {
    const snapshot = await invoke<SessionStatus>("session_status");
    state.value = snapshot.state;
    totalMs.value = snapshot.total_ms;
    onDuty.value = snapshot.on_duty;
  } catch (err) {
    console.error("session_status 调用失败", err);
  }
}

/** 执行命令后立即刷新快照与统计（不等下一 tick）；失败上抛父组件可见反馈 */
async function act(command: string): Promise<void> {
  try {
    await invoke(command);
    await refresh();
    emit("changed");
  } catch (err) {
    emit("error", `${command} 失败：${String(err)}`);
    console.error(`${command} 调用失败`, err);
  }
}

/** 毫秒 → HH:MM:SS.d（十分秒位：点击后 0.1s 内即开始跳动，消除整秒格式的静止感） */
function formatDisplay(total: number): string {
  const h = Math.floor(total / 3_600_000);
  const m = Math.floor((total % 3_600_000) / 60_000);
  const s = Math.floor((total % 60_000) / 1000);
  const d = Math.floor((total % 1000) / 100);
  return `${pad(h)}:${pad(m)}:${pad(s)}.${d}`;
}

onMounted(() => {
  void refresh();
  timer = window.setInterval(() => void refresh(), TICK_MS);
});

onUnmounted(() => {
  if (timer !== undefined) {
    window.clearInterval(timer);
  }
});
</script>

<template>
  <section class="timer">
    <ProgressRing :on-duty="onDuty" :work-secs="workSecs" :target-hours="targetHours">
      <div class="digits" :class="{ 'digits-solid': !onDuty }" data-tauri-drag-region>
        {{ formatDisplay(totalMs) }}
      </div>
    </ProgressRing>
    <button
      class="pill"
      :class="{ active: onDuty }"
      type="button"
      @click="emit('clock', onDuty ? 'out' : 'in')"
    >
      {{ onDuty ? "下班" : "上班" }}
    </button>
    <div class="controls">
      <button
        v-if="state === 'idle'"
        class="btn-primary"
        type="button"
        :disabled="!onDuty"
        @click="act('session_start')"
      >
        开始
      </button>
      <button
        v-else-if="state === 'running'"
        class="btn-primary"
        type="button"
        @click="act('session_pause')"
      >
        暂停
      </button>
      <template v-else>
        <button
          class="btn-primary"
          type="button"
          :disabled="!onDuty"
          @click="act('session_resume')"
        >
          继续
        </button>
        <button class="btn-ghost" type="button" :disabled="!onDuty" @click="act('session_restart')">
          重开
        </button>
      </template>
    </div>
  </section>
</template>

<style scoped>
.timer {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 18px;
  width: 100%;
}

/* 环心大计时器（定稿线框 40px）：展示级字族 + 等宽数字（tabular-nums 防跳动）。
   PL007.5 渐变描字经 background-clip 上字；未上班挂 .digits-solid 纯色回退（定稿线框） */
.digits {
  background: var(--grad-digit);
  background-clip: text;
  -webkit-background-clip: text;
  -webkit-text-fill-color: transparent;
  font-family: var(--font-stack);
  font-variant-numeric: tabular-nums;
  font-size: 40px;
  font-weight: 600;
  letter-spacing: 1px;
  line-height: 1.1;
  cursor: default;
}

/* 纯色回退：恢复实色文字（保留等宽与字号） */
.digits-solid {
  background: none;
  -webkit-text-fill-color: initial;
  color: var(--ink);
}

/* 打卡 pill（PL007.3 糖果蒙皮，PL008.4 移入环主体区）：未上班 = 紫 accent 描边可按态；
   在岗中 = 紫渐变实底 + 内凹高光（视觉常驻"已按下"） */
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
  background: var(--grad-primary);
  color: #fff;
  box-shadow:
    var(--rim-light),
    inset 0 2px 6px rgba(0, 0, 0, 0.22),
    var(--shadow-candy);
}

/* 按钮配方（实底/玻璃/置灰）在 App.vue 全局 .btn-primary/.btn-ghost，此处只留尺寸 */
.controls {
  display: flex;
  gap: 12px;
}

.controls button {
  min-width: 112px;
  padding: 12px 0;
  font-size: 16px;
}
</style>
