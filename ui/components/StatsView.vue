<script setup lang="ts">
/** 统计视图（PL005）：时间图谱 + 在岗/工作/休息三值 + 段明细 + 前后日翻看。
 * 纯展示组件：所有数据来自 day_detail 命令（Rust 侧归约），本组件零业务聚合。 */
import { onMounted, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";

import type { DaySummary } from "../types";
const props = defineProps<{ refreshKey: number }>();

const offset = ref(0);
const day = ref<DaySummary | null>(null);

/** 拉取单日明细（offset 为日偏移：0 今日 / -1 昨日） */
async function refresh(): Promise<void> {
  try {
    day.value = await invoke<DaySummary>("day_detail", { offset: offset.value });
  } catch (err) {
    console.error("day_detail 调用失败", err);
  }
}

/** Unix 秒 → 本地 HH:MM（图谱与明细的时刻显示） */
function hhmm(secs: number): string {
  const d = new Date(secs * 1000);
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

/** 秒数 → 简洁时长（Xh Ym；不足 1 小时只显分钟） */
function fmt(total: number): string {
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  return h > 0 ? `${h}h ${m}m` : `${m}m`;
}

/** 日期标签：今日 / 昨日 / M月D日 */
function dayLabel(off: number): string {
  if (off === 0) return "今日";
  if (off === -1) return "昨日";
  const d = new Date(Date.now() + off * 86_400_000);
  return `${d.getMonth() + 1}月${d.getDate()}日`;
}

/** 图谱区块宽度百分比（相对在岗窗口；窗口外的多班间隙留空） */
function blockWidth(start: number, end: number): string {
  const from = day.value?.duty_started_at;
  const to = day.value?.duty_ended_at ?? Date.now() / 1000;
  if (from == null || to <= from) {
    return "0%";
  }
  return `${((end - start) / (to - from)) * 100}%`;
}

/** 进行中块判定：在岗未收班时的最后一个块（明细时长列显"至今"） */
function isOngoing(i: number): boolean {
  return day.value != null && day.value.duty_ended_at == null && i === day.value.blocks.length - 1;
}

onMounted(refresh);
watch(() => props.refreshKey, refresh);
watch(offset, refresh);
</script>

<template>
  <section class="stats-view">
    <div class="nav">
      <button class="arrow" type="button" title="前一日" @click="offset--">‹</button>
      <span class="day-label">{{ dayLabel(offset) }}</span>
      <button class="arrow" type="button" title="后一日" :disabled="offset >= 0" @click="offset++">
        ›
      </button>
    </div>

    <template v-if="day && day.blocks.length > 0">
      <div class="chart">
        <div
          v-for="(b, i) in day.blocks"
          :key="i"
          class="seg"
          :class="b.kind"
          :style="{ width: blockWidth(b.start, b.end) }"
        ></div>
      </div>
      <div class="chart-axis">
        <span>{{ hhmm(day.duty_started_at ?? 0) }}</span>
        <span>{{ day.duty_ended_at == null ? "在岗中" : hhmm(day.duty_ended_at) }}</span>
      </div>
      <div class="triple">
        <div class="triple-item">
          <span class="t-label">在岗</span>
          <span class="t-value">{{ fmt(day.duty_secs) }}</span>
        </div>
        <div class="triple-item">
          <span class="t-label">工作</span>
          <span class="t-value">{{ fmt(day.work_secs) }}</span>
        </div>
        <div class="triple-item">
          <span class="t-label">休息</span>
          <span class="t-value">{{ fmt(day.rest_secs) }}</span>
        </div>
      </div>
      <ul class="detail">
        <li v-for="(b, i) in day.blocks" :key="`r${i}`" class="detail-row">
          <span class="range">{{ hhmm(b.start) }} – {{ hhmm(b.end) }}</span>
          <span class="dur">{{ isOngoing(i) ? "至今" : fmt(b.end - b.start) }}</span>
          <span class="kind" :class="b.kind">{{ b.kind === "work" ? "工作" : "休息" }}</span>
        </li>
      </ul>
    </template>
    <p v-else class="empty">本日无打卡记录</p>
  </section>
</template>

<style scoped>
.stats-view {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 10px;
  width: 100%;
}

.nav {
  display: flex;
  align-items: center;
  gap: 14px;
}

.arrow {
  width: 28px;
  padding: 2px 0;
  border: 1px solid rgba(128, 128, 128, 0.3);
  border-radius: var(--r-pill);
  background: rgba(128, 128, 128, 0.1);
  color: inherit;
  font-size: 14px;
  cursor: pointer;
  transition: background 0.15s ease;
}

.arrow:hover {
  background: rgba(128, 128, 128, 0.2);
}

.arrow:disabled {
  opacity: 0.35;
  cursor: not-allowed;
}

.day-label {
  font-size: 15px;
  font-weight: 600;
}

/* 时间图谱：连续圆角胶囊条，工作 accent 实底 / 休息 ink 低明度；块间 2px 呼吸缝（纯 CSS，不引图表库） */
.chart {
  display: flex;
  gap: 2px;
  width: 100%;
  height: 14px;
  border-radius: var(--r-pill);
  overflow: hidden;
}

.seg {
  height: 100%;
  transition: width 0.3s ease;
}

.seg.work {
  background: var(--accent);
}

.seg.rest {
  background: color-mix(in srgb, currentColor 12%, transparent);
}

.chart-axis {
  display: flex;
  justify-content: space-between;
  width: 100%;
  color: var(--ink-2);
  font-size: 11px;
}

/* 三值：两行对仗（标签 Caption 层 + 数值 Title 层），替代一行"｜"挤排 */
.triple {
  display: flex;
  justify-content: space-evenly;
  width: 100%;
  margin: 0;
}

.triple-item {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 2px;
}

.t-label {
  color: var(--ink-2);
  font-size: 11px;
}

.t-value {
  font-size: 15px;
  font-weight: 600;
  font-variant-numeric: tabular-nums;
}

/* 段明细：起止 HH:MM + 时长 + 类型；去常驻底色，hover 微亮；列表过长内部滚动 */
.detail {
  width: 100%;
  max-height: 118px;
  margin: 0;
  padding: 0;
  list-style: none;
  overflow-y: auto;
}

.detail-row {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 4px 10px;
  border-radius: 8px;
  font-size: 12px;
  transition: background 0.15s ease;
}

.detail-row:hover {
  background: rgba(128, 128, 128, 0.12);
}

.range {
  flex: 1;
  font-variant-numeric: tabular-nums;
}

.dur {
  color: var(--ink-2);
}

.kind.work {
  color: var(--accent);
}

.kind.rest {
  color: var(--ink-2);
}

.empty {
  margin: 24px 0;
  color: var(--ink-2);
  font-size: 13px;
}
</style>
