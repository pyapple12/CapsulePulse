<script setup lang="ts">
/** 托盘 hover 今日概览（PL028.1）：按后端 DTO 渲染 1 行（未在岗）或 3 行（在岗 / 暂下班）。
 * 窗口尺寸由后端在显示前按行数设定（几何归 Rust，两端同源）；本组件只渲染。
 * 数据：挂载拉一次 + 每次显示由后端推 `hover-update`（免二次取数）。 */
import { onMounted, onUnmounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import type { HoverSummary } from "../../types";

const summary = ref<HoverSummary | null>(null);
let unlisten: (() => void) | undefined;

/** 秒 → `Xh Ym`（0 值也显式显示，不留空位歧义） */
function hmm(secs: number): string {
  const total = Math.max(0, Math.floor(secs));
  return `${Math.floor(total / 3600)}h ${Math.floor((total % 3600) / 60)}m`;
}

onMounted(() => {
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
  <div class="hover-card">
    <template v-if="summary">
      <div class="row main">
        <span class="label">{{ summary.label }}</span>
        <span v-if="summary.remaining_secs != null" class="value">
          {{ hmm(summary.remaining_secs) }}
        </span>
      </div>
      <template v-if="!summary.single_line">
        <div class="row">
          <span class="label">工作</span>
          <span class="value">{{ hmm(summary.work_secs) }}</span>
          <span class="label gap">休息</span>
          <span class="value">{{ hmm(summary.rest_secs) }}</span>
        </div>
        <div class="row">
          <span class="label">离休息提醒</span>
          <span class="value">{{ summary.until_reminder_min.toFixed(1) }} 分钟</span>
        </div>
      </template>
    </template>
  </div>
</template>

<style scoped>
/* 悬浮卡（PL028.1）：窗为纯 alpha 透明（tauri.conf transparent），卡自带玻璃配方——
   窗内采样有效（与浮板同款：深纱 + backdrop blur）；圆角/描边与主窗浮板同语言。 */
.hover-card {
  box-sizing: border-box;
  width: 100%;
  height: 100%;
  padding: 10px 12px;
  border-radius: 10px;
  background: rgba(18, 20, 26, 0.86);
  box-shadow:
    inset 0 0 0 1px rgba(255, 255, 255, 0.14),
    0 8px 24px rgba(0, 0, 0, 0.35);
  backdrop-filter: blur(20px) saturate(1.3);
  color: #f2f4f8;
  font-family: system-ui, sans-serif;
  display: flex;
  flex-direction: column;
  justify-content: center;
  gap: 6px;
}
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
.row.main .value {
  margin-left: auto;
}
.label {
  opacity: 0.72;
}
.value {
  font-variant-numeric: tabular-nums;
}
.gap {
  margin-left: 8px;
}
</style>
