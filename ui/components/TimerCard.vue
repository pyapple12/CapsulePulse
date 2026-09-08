<script setup lang="ts">
import { onMounted, onUnmounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";

/** status 命令返回体（镜像 Rust 侧 SessionStatus serde 结构，单一来源在 Rust） */
interface SessionStatus {
  state: "idle" | "running" | "paused";
  total_ms: number;
}

// 轮询周期 100ms：十分秒位（HH:MM:SS.d）每 0.1s 跳动需要 ≤100ms 拉取；
// 10 次/s 本地 IPC 开销可忽略。（演进：1s→250ms 修秒进位迟到，2026-09-08 用户定案改十分秒位后→100ms）
const TICK_MS = 100;

const state = ref<"idle" | "running" | "paused">("idle");
const totalMs = ref(0);
let timer: number | undefined;

/** 拉取会话快照（tick 定案：前端 setInterval 拉取，Rust 不推送） */
async function refresh(): Promise<void> {
  try {
    const snapshot = await invoke<SessionStatus>("session_status");
    state.value = snapshot.state;
    totalMs.value = snapshot.total_ms;
  } catch (err) {
    console.error("session_status 调用失败", err);
  }
}

/** 执行命令后立即刷新快照（不等下一秒） */
async function act(command: string): Promise<void> {
  try {
    await invoke(command);
    await refresh();
  } catch (err) {
    console.error(`${command} 调用失败`, err);
  }
}

/** 毫秒 → HH:MM:SS.d（十分秒位：点击后 0.1s 内即开始跳动，消除整秒格式的静止感） */
function formatDisplay(total: number): string {
  const h = Math.floor(total / 3_600_000);
  const m = Math.floor((total % 3_600_000) / 60_000);
  const s = Math.floor((total % 60_000) / 1000);
  const d = Math.floor((total % 1000) / 100);
  const pad = (n: number) => String(n).padStart(2, "0");
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
        class="btn primary"
        type="button"
        @click="act('session_start')"
      >
        开始
      </button>
      <button
        v-else-if="state === 'running'"
        class="btn primary"
        type="button"
        @click="act('session_pause')"
      >
        暂停
      </button>
      <template v-else>
        <button class="btn primary" type="button" @click="act('session_resume')">继续</button>
        <button class="btn" type="button" @click="act('session_restart')">重开</button>
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

/* 大计时器：等宽数字（tabular-nums 防跳动） */
.digits {
  font-family: "Cascadia Mono", Consolas, monospace;
  font-variant-numeric: tabular-nums;
  font-size: 44px;
  letter-spacing: 2px;
  cursor: default;
}

.controls {
  display: flex;
  gap: 12px;
}

/* 大圆角按钮：半透明底适配玻璃卡片双主题 */
.btn {
  min-width: 104px;
  padding: 12px 0;
  border: 1px solid rgba(128, 128, 128, 0.4);
  border-radius: 12px;
  background: rgba(128, 128, 128, 0.12);
  color: inherit;
  font-size: 16px;
  cursor: pointer;
  transition: background 0.15s;
}

.btn:hover {
  background: rgba(128, 128, 128, 0.24);
}

.btn.primary {
  background: rgba(0, 122, 255, 0.75);
  border-color: transparent;
  color: #fff;
}

.btn.primary:hover {
  background: rgba(0, 122, 255, 0.9);
}
</style>
