<script setup lang="ts">
/** 统计视图（PL005）：时间图谱 + 在岗/工作/休息三值 + 段明细 + 前后日翻看。
 * 纯展示组件：所有数据来自 day_detail 命令（Rust 侧归约），本组件零业务聚合。 */
import { onMounted, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
// PL008.3：日导航箭头换 lucide 线性图标（与 dock 同源）
import { ChevronLeft, ChevronRight } from "lucide-vue-next";

// 展示格式化共享助手（FIX002.13 收敛）
import { fmtDuration, hhmm } from "../format";
import type { DaySummary, WeekDay } from "../types";
const props = defineProps<{ refreshKey: number }>();

const offset = ref(0);
const day = ref<DaySummary | null>(null);
// 周视图（PL008.6）：锚 = 今日回溯 7 日（旧 → 新），Rust 侧归约，本组件零聚合
const week = ref<WeekDay[]>([]);
// 星期单字标签（weekday: 1 = 周一 … 7 = 周日）
const WEEKDAY_LABELS = ["一", "二", "三", "四", "五", "六", "日"] as const;
// 拉取失败可见化（FIX002.4）：区分"空日"与"加载失败"，避免错误伪装成无打卡
const loadError = ref("");

/** 拉取单日明细 + 周视图（offset 为日偏移：0 今日 / -1 昨日）；任一失败保留旧数据并标错误
 *  （FIX003.10：周卡失败与单日同通道可见，不再静默消失） */
async function refresh(): Promise<void> {
  try {
    day.value = await invoke<DaySummary>("day_detail", { offset: offset.value });
    loadError.value = "";
  } catch (err) {
    loadError.value = String(err);
    console.error("day_detail 调用失败", err);
  }
  try {
    week.value = await invoke<WeekDay[]>("week_detail");
  } catch (err) {
    loadError.value = String(err);
    console.error("week_detail 调用失败", err);
  }
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

/** 周条宽度：当日工作秒 ÷ 7 日峰值（全零日返回 0%） */
function weekBarWidth(secs: number): string {
  const peak = Math.max(...week.value.map((d) => d.work_secs), 0);
  if (peak <= 0) {
    return "0%";
  }
  return `${(secs / peak) * 100}%`;
}

onMounted(refresh);
watch(() => props.refreshKey, refresh);
watch(offset, refresh);
</script>

<template>
  <section class="stats-view">
    <div class="panel glass-panel nav-panel">
      <button class="arrow" type="button" title="前一日" @click="offset--">
        <ChevronLeft :size="15" :stroke-width="2.2" aria-hidden="true" />
      </button>
      <span class="day-label">{{ dayLabel(offset) }}</span>
      <button class="arrow" type="button" title="后一日" :disabled="offset >= 0" @click="offset++">
        <ChevronRight :size="15" :stroke-width="2.2" aria-hidden="true" />
      </button>
    </div>

    <p v-if="loadError" class="load-error" role="alert">统计加载失败：{{ loadError }}</p>

    <template v-if="day && day.blocks.length > 0">
      <div class="panel glass-panel chart-panel">
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
      </div>
      <div class="panel glass-panel triple-panel">
        <div class="triple">
          <div class="triple-item glass-chip chip-duty">
            <span class="t-label">在岗</span>
            <span class="t-value t-duty">{{ fmtDuration(day.duty_secs) }}</span>
          </div>
          <div class="triple-item glass-chip chip-work">
            <span class="t-label">工作</span>
            <span class="t-value t-work">{{ fmtDuration(day.work_secs) }}</span>
          </div>
          <div class="triple-item glass-chip chip-rest">
            <span class="t-label">休息</span>
            <span class="t-value">{{ fmtDuration(day.rest_secs) }}</span>
          </div>
        </div>
      </div>
    </template>
    <!-- 错误态不显空文案（FIX003.10）：加载失败时"本日无打卡记录"会伪装成真实空数据 -->
    <p v-else-if="!loadError" class="empty">本日无打卡记录</p>
    <div v-if="week.length > 0" class="panel glass-panel week-panel">
      <p class="week-title">最近 7 日</p>
      <div
        v-for="(d, i) in week"
        :key="d.date"
        class="week-row"
        :class="{ today: i === week.length - 1 }"
      >
        <span class="week-day">{{ WEEKDAY_LABELS[d.weekday - 1] }}</span>
        <span class="week-track">
          <span class="week-bar" :style="{ width: weekBarWidth(d.work_secs) }"></span>
        </span>
        <span class="week-secs">{{ fmtDuration(d.work_secs) }}</span>
      </div>
    </div>
    <template v-if="day && day.blocks.length > 0">
      <ul class="panel glass-panel detail-panel">
        <li v-for="(b, i) in day.blocks" :key="`r${i}`" class="detail-row">
          <span class="range">{{ hhmm(b.start) }} – {{ hhmm(b.end) }}</span>
          <span class="dur">{{ isOngoing(i) ? "至今" : fmtDuration(b.end - b.start) }}</span>
          <span class="kind" :class="b.kind">{{ b.kind === "work" ? "工作" : "休息" }}</span>
        </li>
      </ul>
    </template>
  </section>
</template>

<style scoped>
/* 统计页（PL008.5 卡片化）：吃掉 dock 与口径行之间的全部富余高度；明细卡 flex:1 为
   弹性主承压面（窗口拉高 → 明细卡变高内滚），min-height:0 释放 flex 收缩许可 */
.stats-view {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 6px;
  width: 100%;
  flex: 1;
  min-height: 0;
  overflow: hidden;
}

/* 浮起卡公共形态：材质由全局 .glass-panel 提供，此处只定圆角与内距节奏 */
.panel {
  border-radius: var(--r-card);
  padding: 7px 12px;
  box-sizing: border-box;
  flex-shrink: 0;
}

.nav-panel {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 14px;
  width: 100%;
  padding: 3px 12px;
}

.chart-panel {
  width: 100%;
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.triple-panel {
  width: 100%;
  padding: 8px 10px;
}

.arrow {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  padding: 2px 0;
  border: 1px solid rgba(128, 128, 128, 0.3);
  border-radius: var(--r-pill);
  background: rgba(128, 128, 128, 0.1);
  color: inherit;
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

/* 时间图谱（PL007.4 蒙皮）：玻璃内凹轨道 + rim 光；工作 = 紫渐变实条 / 休息 = 薄荷半透明；
   块间 2px 呼吸缝（纯 CSS，不引图表库），宽度口径不变 */
.chart {
  display: flex;
  gap: 2px;
  width: 100%;
  height: 18px;
  padding: 2px;
  border-radius: var(--r-pill);
  background: rgba(120, 120, 128, 0.14);
  box-shadow:
    inset 0 1px 3px rgba(0, 0, 0, 0.1),
    var(--rim-light);
  overflow: hidden;
}

.seg {
  height: 100%;
  transition: width 0.3s ease;
}

.seg.work {
  background: var(--grad-primary);
  box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.35);
}

.seg.rest {
  background: color-mix(in srgb, var(--mint-bright) 65%, transparent);
}

.chart-axis {
  display: flex;
  justify-content: space-between;
  width: 100%;
  color: var(--ink-2);
  font-size: 11px;
}

/* 三值（PL007.5）：三枚 .glass-chip 胶囊（材质在全局类），此处只留尺寸与身份色：
   在岗 = 紫描边 + 紫值 / 工作 = 薄荷描边 + 薄荷值 / 休息 = 中性描边（PL007 方向定案 1） */
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
  padding: 6px 14px;
  border-radius: var(--r-pill);
}

.chip-duty {
  border: 1px solid color-mix(in srgb, var(--accent) 35%, transparent);
}

.chip-work {
  border: 1px solid color-mix(in srgb, var(--mint) 35%, transparent);
}

.chip-rest {
  border: 1px solid rgba(128, 128, 128, 0.3);
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

.t-value.t-duty {
  color: var(--accent);
}

.t-value.t-work {
  color: var(--mint);
}

/* 周视图卡（PL008.6）：7 条横向条形，今日紫渐变高亮、其余玻璃中性底；
   条长 = 当日 work_secs 占 7 日峰值（纯展示换算，零业务聚合）。
   行高压缩（默认 560 高五卡 + dock 须全容纳，溢出实测教训） */
.week-panel {
  display: flex;
  flex-direction: column;
  gap: 3px;
  width: 100%;
  padding: 8px 12px;
}

.week-title {
  margin: 0;
  color: var(--ink-2);
  font-size: 10px;
}

.week-row {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 10px;
}

.week-day {
  width: 12px;
  color: var(--ink-2);
  text-align: center;
}

.week-track {
  display: block;
  flex: 1;
  height: 6px;
  border-radius: var(--r-pill);
  background: color-mix(in srgb, var(--ink) 7%, transparent);
  overflow: hidden;
}

.week-bar {
  display: block;
  height: 100%;
  border-radius: var(--r-pill);
  background: color-mix(in srgb, var(--ink) 26%, transparent);
  transition: width 0.3s ease;
}

.week-row.today .week-day {
  color: var(--accent);
  font-weight: 600;
}

.week-row.today .week-bar {
  background: var(--grad-primary);
  box-shadow: 0 0 5px rgba(139, 92, 246, 0.45);
}

.week-secs {
  width: 44px;
  color: var(--ink-2);
  font-variant-numeric: tabular-nums;
  text-align: right;
}

/* 段明细卡（PL008.5）：flex:1 吃富余高度 + 内部滚动（去固定 max-height，弹性主承压面）；
   行 hover 微亮 */
.detail-panel {
  width: 100%;
  flex: 1;
  min-height: 64px;
  margin: 0;
  padding: 4px 8px;
  list-style: none;
  overflow-y: auto;
}

.detail-row {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 3px 8px;
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
  color: var(--mint);
}

@media (prefers-color-scheme: dark) {
  .chart {
    background: rgba(0, 0, 0, 0.28);
  }
}

.empty {
  margin: 24px 0;
  color: var(--ink-2);
  font-size: 13px;
}

/* 加载失败提示（FIX002.4）：区分空日与失败，双主题可读的错误红 */
.load-error {
  margin: 0;
  padding: 6px 14px;
  border-radius: var(--r-pill);
  background: rgba(179, 38, 30, 0.28);
  font-size: 12px;
}
</style>
