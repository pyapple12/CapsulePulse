<script setup lang="ts">
import { onMounted, onUnmounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";

// IPC DTO 镜像类型统一在 types.ts（单一来源 = Rust serde 结构）
import type { SessionStatus } from "../types";
// 展示格式化共享助手（FIX002.13 收敛）
import { pad } from "../format";

// 轮询周期 100ms：十分秒位（HH:MM:SS.d）每 0.1s 跳动需要 ≤100ms 拉取；
// 10 次/s 本地 IPC 开销可忽略。（演进：1s→250ms 修秒进位迟到，2026-09-08 用户定案改十分秒位后→100ms）
const TICK_MS = 100;

// 动作（开始/暂停/继续/重开）后通知父组件刷新统计行；失败上抛文案（FIX002.3 可见反馈）
const emit = defineEmits<{ changed: []; error: [string] }>();

const state = ref<"idle" | "running" | "paused">("idle");
const totalMs = ref(0);
// 在岗态（PL005）：未上班时计时按钮置灰禁用——后端门禁之外的前端面
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
    <div class="digits" data-tauri-drag-region>{{ formatDisplay(totalMs) }}</div>
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
  gap: 20px;
  width: 100%;
}

/* 大计时器：展示级字族（SF/Segoe UI Variable）+ 等宽数字（tabular-nums 防跳动），内容层无框无底。
   PL007.5 渐变描字：紫罗兰渐变经 background-clip 上字；对比度不达标或裁切不可用时挂 .digits-solid 回退纯色 */
.digits {
  background: var(--grad-digit);
  background-clip: text;
  -webkit-background-clip: text;
  -webkit-text-fill-color: transparent;
  font-family: var(--font-stack);
  font-variant-numeric: tabular-nums;
  font-size: 56px;
  font-weight: 600;
  letter-spacing: 1px;
  line-height: 1.1;
  cursor: default;
}

/* 纯色回退：恢复实色文字（保留等宽与字号），一键换回 */
.digits-solid {
  background: none;
  -webkit-text-fill-color: initial;
  color: var(--ink);
}

/* 按钮配方（实底/玻璃/置灰/按压）在 App.vue 全局 .btn-primary/.btn-ghost，此处只留尺寸 */
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
