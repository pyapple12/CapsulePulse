<script setup lang="ts">
/** 统计板（PL018 design 换装）：日期导航 + 时间图谱（分段/RWD 节点/彗星发射流）+ 三值卡
 * + 周卡（堆叠柱/均线/选中环闪现）+ 翻面今日明细。纯展示组件：数据来自 day_detail/week_detail
 * （Rust 侧归约），渲染算法 1:1 自 design index.html 内联脚本翻译（数据源换真实命令）。 */
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";

import type { DaySummary, WeekDay } from "../types";
const props = defineProps<{ refreshKey: number; autoOutHours: number | null; open: boolean }>();

const offset = ref(0);
const day = ref<DaySummary | null>(null);
// 周视图（PL008.6）：锚 = 今日回溯 7 日（旧 → 新），Rust 侧归约，本组件零聚合
const week = ref<WeekDay[]>([]);
const loadError = ref("");

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

// 开板即刷（design：开板时强制重绘）
watch(
  () => props.open,
  (open) => {
    if (open) {
      void refresh();
    }
  },
);

/* ============================================================
   日期导航与标签（design day-label 语义：M月D日 · 周X · 在岗中/已下班）
   ============================================================ */
const WEEKDAY_CN = ["周一", "周二", "周三", "周四", "周五", "周六", "周日"];

const dayLabelText = computed(() => {
  const d = new Date(Date.now() + offset.value * 86_400_000);
  const state = offset.value < 0 || day.value?.duty_ended_at != null ? "已下班" : "在岗中";
  return `${d.getMonth() + 1}月${d.getDate()}日 · ${WEEKDAY_CN[(d.getDay() + 6) % 7]} · ${state}`;
});

/* ============================================================
   时间图谱（design renderGraph 1:1）：bars/nodes 属性更新 + comet 白纹发射
   ============================================================ */
const graphSvg = ref<SVGSVGElement | null>(null);
const axisStart = ref("");
const axisEnd = ref("");

function graphHhmm(ms: number): string {
  const d = new Date(ms);
  return `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
}

/** 已删令牌无关：本表为 design graph.css 消费的纯数值换算 */
interface GraphSeg {
  t0: number;
  t1: number;
  kind: "work" | "rest";
}

/** graphSvg 内的子节点引用（bars 复用更新 / comet 白纹容器） */
const barsEl = ref<SVGGElement | null>(null);
const cometEl = ref<SVGGElement | null>(null);
const cometClipRect = ref<SVGRectElement | null>(null);

const COMET = { speed: 18, spacing: 14 }; // px/s、px（design 定稿）
let cometEpoch = 0;
let cometWalk = 0;
let cometLastFrame = Number.NEGATIVE_INFINITY;
let cometNeedsRebuild = false;
let cometRaf = 0;
let cometWakeTimer = 0;
const reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)");
const SVG_NS = "http://www.w3.org/2000/svg";

/** 图谱整帧渲染（design renderGraph 1:1；数据 = 真实 day.blocks，秒 → ms） */
function renderGraph(): void {
  const svg = graphSvg.value;
  const bars = barsEl.value;
  const d = day.value;
  if (!svg || !bars || !d?.duty_started_at || d.blocks.length === 0) {
    return;
  }
  const clockIn = d.duty_started_at * 1000;
  const autoOut = (props.autoOutHours ?? 8) * 3_600_000;
  const isToday = offset.value >= 0;
  const nowMs = Date.now();

  // 段源：真实块（秒 → ms）；同态延伸只做同 kind（异态分界由下次刷新的块数据承担）
  const segs: GraphSeg[] = d.blocks.map((b) => ({
    t0: b.start * 1000,
    t1: (b.end || Math.floor(nowMs / 1000)) * 1000,
    kind: b.kind,
  }));
  const tail = segs[segs.length - 1];
  if (isToday && d.duty_ended_at == null && tail) {
    const wallNow = Math.floor(nowMs / 1000) * 1000;
    if (wallNow > tail.t1) {
      tail.t1 = wallNow;
    }
  }

  const endTs = isToday ? clockIn + autoOut : segs[segs.length - 1]!.t1;
  const nowTs = isToday ? nowMs : endTs;
  const W = svg.clientWidth;
  if (W > 0 && svg.viewBox.baseVal.width !== W) {
    svg.setAttribute("viewBox", `0 0 ${W} 12`);
  }
  const xpx = (ts: number): number => ((ts - clockIn) / (endTs - clockIn)) * W;

  // bars：数量对齐 + 改属性（design 复用策略）
  interface Bar {
    st: string;
    x: number;
    w: number;
    active: boolean;
  }
  const list: Bar[] = [];
  const nodes: { ts: number; ch: string; kind: string }[] = [];
  segs.forEach((s, i) => {
    const { t0, t1 } = s;
    const st = s.kind;
    const capped = Math.min(t1, nowTs, endTs);
    if (capped <= t0) {
      return;
    }
    list.push({
      st,
      x: xpx(t0),
      w: xpx(capped) - xpx(t0),
      active: nowTs < endTs && capped === nowTs,
    });
    if (i > 0 && st !== segs[i - 1]!.kind) {
      nodes.push({ ts: t0, ch: st === "rest" ? "R" : "W", kind: st });
    }
  });
  if (nowTs < endTs) {
    list.push({ st: "duty", x: xpx(nowTs), w: W - xpx(nowTs), active: false });
  }
  nodes.push({ ts: endTs, ch: "D", kind: "end" });

  while (bars.children.length < list.length) {
    bars.append(document.createElementNS(SVG_NS, "rect"));
  }
  while (bars.children.length > list.length) {
    bars.lastElementChild?.remove();
  }
  [...bars.children].forEach((rect, i) => {
    const b = list[i]!;
    rect.setAttribute("class", `graph-seg ${b.st}${b.active ? " active" : ""}`);
    rect.setAttribute("x", b.x.toFixed(2));
    rect.setAttribute("width", Math.max(0, b.w).toFixed(2));
    rect.setAttribute("y", "0");
    rect.setAttribute("height", "12");
  });

  // nodes：数量变化整层重建（design：避免复用跨色渐变伪影）
  const nodesG = svg.querySelector<SVGGElement>("#graphNodes");
  if (nodesG) {
    nodesG.innerHTML = "";
    for (const n of nodes) {
      const off = n.kind === "end" ? 12 : 6;
      const g = document.createElementNS(SVG_NS, "g");
      g.setAttribute("class", `graph-node ${n.kind}${n.ts <= nowTs ? " lit" : ""}`);
      g.setAttribute("transform", `translate(${(xpx(n.ts) - off).toFixed(2)} 0)`);
      g.innerHTML =
        '<circle cx="6" cy="6" r="5.4" /><text x="6" y="6" dy="-0.5" text-anchor="middle" dominant-baseline="central"></text>';
      (g.querySelector("text") as SVGTextElement).textContent = n.ch;
      nodesG.append(g);
    }
  }

  // 轴标
  axisStart.value = `上班 ${graphHhmm(clockIn)}`;
  axisEnd.value = `下班 ${graphHhmm(endTs)}`;

  // 彗星走廊（design 1:1）：今天 = birthX 起走廊；过去日 walk=0 静默
  const comet = cometEl.value;
  const clipRect = cometClipRect.value;
  if (!comet || !clipRect) {
    return;
  }
  const birthX = xpx(segs[segs.length - 1]!.t0);
  cometWalk = isToday ? W - birthX + 24 : 0;
  if (comet.dataset.awake !== "1") {
    comet.dataset.awake = "1";
    cometEpoch =
      Number(document.timeline.currentTime ?? 0) - ((2 * cometWalk) / COMET.speed) * 1000;
    requestAnimationFrame(function frame(t) {
      if (t - cometLastFrame > 250) {
        cometNeedsRebuild = true;
      }
      cometLastFrame = t;
      if (cometNeedsRebuild) {
        cometNeedsRebuild = false;
        launchComet(true);
      }
      if (!destroyed) {
        cometRaf = requestAnimationFrame(frame);
      }
    });
    (function wake(): void {
      if (destroyed) {
        return;
      }
      const period = (COMET.spacing / COMET.speed) * 1000;
      const animNow = Number(document.timeline.currentTime ?? 0);
      const next = cometEpoch + (Math.floor((animNow - cometEpoch) / period) + 1) * period;
      cometWakeTimer = window.setTimeout(
        () => {
          launchComet(false);
          wake();
        },
        Math.max(0, next - animNow),
      );
    })();
  }
  if (isToday && cometWalk > 24) {
    const wasHidden = comet.style.display === "none";
    comet.style.display = "";
    comet.setAttribute("transform", `translate(${birthX.toFixed(2)} 0)`);
    clipRect.setAttribute("x", birthX.toFixed(2));
    clipRect.setAttribute("width", (W - birthX).toFixed(2));
    if (wasHidden) {
      launchComet(true);
    }
  } else {
    comet.style.display = "none";
    for (const s of [...comet.children]) {
      s.remove();
    }
  }
}

/** 白纹发射（design launchComet 1:1）：纪元网格对齐 + 负延迟相位接入 + 寿命回收 */
function launchComet(force: boolean): void {
  const comet = cometEl.value;
  if (!comet || reducedMotion.matches || cometWalk <= 0) {
    return;
  }
  const now = Number(document.timeline.currentTime ?? 0);
  const period = (COMET.spacing / COMET.speed) * 1000;
  const life = (cometWalk / COMET.speed) * 1000;
  if (force) {
    for (const s of [...comet.children]) {
      s.remove();
    }
  } else {
    if (now - cometLastFrame > 250) {
      return;
    }
    for (const s of [...comet.children] as SVGRectElement[]) {
      if (now - parseFloat(s.dataset.born ?? "0") >= life) {
        s.remove();
      }
    }
  }
  const mNow = Math.floor((now - cometEpoch) / period);
  for (let m = mNow; m >= 0; m--) {
    const born = cometEpoch + m * period;
    const age = now - born;
    if (age >= life) {
      break;
    }
    if (comet.querySelector(`[data-born="${born}"]`)) {
      continue;
    }
    spawnStripe(comet, born, age);
  }
}

function spawnStripe(comet: SVGGElement, born: number, age: number): void {
  const s = document.createElementNS(SVG_NS, "rect");
  s.setAttribute("class", "comet-stripe");
  s.dataset.born = String(born);
  s.setAttribute("x", "-24");
  s.setAttribute("y", "0");
  s.setAttribute("width", "24");
  s.setAttribute("height", "12");
  s.style.setProperty("--walk", `${Math.round(cometWalk)}px`);
  s.style.animationDuration = `${Math.round((cometWalk / COMET.speed) * 1000)}ms`;
  if (age > 0) {
    s.style.animationDelay = `${-Math.round(age)}ms`;
  }
  comet.append(s);
  s.addEventListener("animationend", () => s.remove());
}

/* ============================================================
   周卡（design renderWeekMock 1:1）：堆叠柱（休息绿上/工作橙下/顶角圆）
   + 均线 + 选中环闪现（0.3s 淡入 / 1s 保持 / 0.5s 淡出）
   ============================================================ */
const wcSvg = ref<SVGSVGElement | null>(null);
const wcBars = ref<SVGGElement | null>(null);
const wcSelRing = ref<SVGPathElement | null>(null);
const wcAvgLine = ref<HTMLElement | null>(null);
const avgWorkMin = ref(0);
const flipped = ref(false);
let selTimer = 0;
let destroyed = false;
let lastViewDay = 0;
let weekSig = "";

function roundedTopRect(x: number, y: number, w: number, h: number, r: number): string {
  const rr = Math.min(r, h / 2, w / 2);
  return `M${x},${y + h} L${x},${y + rr} Q${x},${y} ${x + rr},${y} L${x + w - rr},${y} Q${x + w},${y} ${x + w},${y + rr} L${x + w},${y + h} Z`;
}

function fmtCN(min: number): string {
  return `${Math.floor(min / 60)}小时${Math.round(min) % 60}分钟`;
}

/** 周卡整帧渲染（design renderWeekMock 1:1；数据 = 真实 week_detail，秒 → 分钟） */
function renderWeek(): void {
  const svg = wcSvg.value;
  const bars = wcBars.value;
  if (!svg || !bars || week.value.length === 0) {
    return;
  }
  const days = week.value.map((d) => ({
    work: d.work_secs / 60,
    rest: (d.duty_secs - d.work_secs) / 60,
  }));
  const maxTotal = Math.max(...days.map((d) => d.work + d.rest), 1);
  const avgWork = days.reduce((a, d) => a + d.work, 0) / days.length;
  avgWorkMin.value = avgWork;

  const W = svg.clientWidth;
  const H = svg.clientHeight;
  if (W > 0 && H > 0 && (svg.viewBox.baseVal.width !== W || svg.viewBox.baseVal.height !== H)) {
    svg.setAttribute("viewBox", `0 0 ${W} ${H}`);
  }
  const slot = W / 7;
  const plotH = H - 3; // 上下各留 1.5px 描边余量

  let inner = "";
  days.forEach((d, i) => {
    const total = d.work + d.rest;
    const barH = total ? (total / maxTotal) * plotH : 0;
    const restH = total ? (d.rest / total) * barH : 0;
    const workH = total ? (d.work / total) * barH : 0;
    const x = Math.round(i * slot + (slot - 20) / 2); // 柱缘取整（根治小数左缘）
    const y = H - 1.5 - barH;
    const r = Math.min(3, 20 / 2);
    if (d.work > 0) {
      inner +=
        restH > 0
          ? `<rect class="seg work" x="${x}" y="${(y + restH).toFixed(2)}" width="20" height="${workH.toFixed(2)}"/>`
          : `<path class="seg work" d="${roundedTopRect(x, y, 20, workH, r)}"/>`;
    }
    if (restH > 0) {
      inner += `<path class="seg rest" d="${roundedTopRect(x, y, 20, restH, r)}"/>`;
    }
  });
  const sig = `${days.length}|${Math.round(days[days.length - 1]!.work)}|${offset.value}`;
  if (sig !== weekSig) {
    weekSig = sig;
    bars.innerHTML = inner;
  }

  // 均线（design 1:1）
  if (wcAvgLine.value) {
    wcAvgLine.value.style.bottom = `${((avgWork / maxTotal) * 100).toFixed(1)}%`;
  }

  // 选中环闪现：查看日偏移变化时触发（design flashSel 1:1）
  if (offset.value !== lastViewDay) {
    lastViewDay = offset.value;
    const viewIdx = Math.max(0, Math.min(days.length - 1, days.length - 1 + offset.value));
    const i = viewIdx;
    const total = days[i]!.work + days[i]!.rest;
    const barH = total ? (total / maxTotal) * plotH : 0;
    const x = Math.round(i * slot + (slot - 20) / 2);
    const y = H - 1.5 - barH;
    const r = 3;
    if (wcSelRing.value) {
      wcSelRing.value.setAttribute(
        "d",
        `M${x},${y + barH} L${x},${y + r} Q${x},${y} ${x + r},${y} L${x + 20 - r},${y} Q${x + 20},${y} ${x + 20},${y + r} L${x + 20},${y + barH}`,
      );
      wcSelRing.value.classList.add("on");
      if (selTimer !== 0) {
        window.clearTimeout(selTimer);
      }
      selTimer = window.setTimeout(() => {
        wcSelRing.value?.classList.remove("on");
        selTimer = 0;
      }, 1300);
    }
  }
}

/* ============================================================
   翻面今日明细（design renderDetailMock 1:1）
   ============================================================ */
interface DetailRow {
  kind: string;
  startMin: number;
  endMin: number;
  ongoing: boolean;
  secs: number;
}

const detailRows = ref<DetailRow[]>([]);
const detailMax = ref(1);

function fmtT(min: number): string {
  const d = new Date((day.value?.duty_started_at ?? 0) * 1000 + min * 60_000);
  return `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
}

function renderDetail(): void {
  const d = day.value;
  if (!d || d.blocks.length === 0) {
    detailRows.value = [];
    return;
  }
  const nowSec = Math.floor(Date.now() / 1000);
  const rows = d.blocks.map((b, i) => {
    const ongoing = offset.value === 0 && d.duty_ended_at == null && i === d.blocks.length - 1;
    const endSec = offset.value === 0 ? Math.min(b.end, nowSec) : b.end;
    return {
      kind: b.kind,
      startMin: (b.start - (d.duty_started_at ?? 0)) / 60,
      endMin: (endSec - (d.duty_started_at ?? 0)) / 60,
      ongoing,
      secs: Math.max(0, endSec - b.start),
    };
  });
  const settled = rows.filter((r) => !r.ongoing).map((r) => r.secs);
  detailMax.value = Math.max(1, ...(settled.length > 0 ? settled : [1]));
  detailRows.value = rows;
}

function detailTitle(): string {
  if (offset.value === 0) {
    return "今日明细";
  }
  const d = new Date(Date.now() + offset.value * 86_400_000);
  return `${d.getMonth() + 1}月${d.getDate()}日明细`;
}

function durText(secs: number): string {
  return `${Math.round(secs / 60)}分`;
}

function blkWidth(secs: number): string {
  return `${Math.min((secs / detailMax.value) * 100, 100).toFixed(1)}%`;
}

/** 三值卡百分比（design tc-percent：占在岗份额） */
function pct(part: number): number {
  const duty = day.value?.duty_secs ?? 0;
  return duty > 0 ? Math.round((part / duty) * 100) : 0;
}

/** 明细进行中段的时长增长由每秒 renderDetail 重算覆盖（design sig 语义同效） */

onMounted(() => {
  cometClipRect.value = document.querySelector<SVGRectElement>("#graphClipCometRect");
  void refresh();
});

watch(
  () => props.refreshKey,
  () => {
    void refresh();
  },
);
watch(offset, () => {
  void refresh();
});
// 渲染节拍：数据到位后重建 + 每秒重算（design 1s 节流同效）
watch([day, week], () => {
  renderGraph();
  renderWeek();
  renderDetail();
});
const secTimer = window.setInterval(() => {
  renderGraph();
  renderWeek();
  renderDetail();
}, 1000);

onUnmounted(() => {
  destroyed = true;
  window.clearInterval(secTimer);
  if (cometRaf !== 0) {
    cancelAnimationFrame(cometRaf);
  }
  if (cometWakeTimer !== 0) {
    window.clearTimeout(cometWakeTimer);
  }
  if (selTimer !== 0) {
    window.clearTimeout(selTimer);
  }
});
</script>

<template>
  <div id="stats-board" :class="{ open, flipped }">
    <div class="board-glass flip-scene">
      <div class="flip-inner">
        <!-- 正面：统计五区 -->
        <section class="stats sheet flip-face front">
          <div class="daynav">
            <button class="arrow" type="button" title="前一日" @click="offset--">
              <svg
                width="7"
                height="7"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2.4"
                stroke-linecap="round"
                stroke-linejoin="round"
              >
                <path d="m15 5-7 7 7 7" />
              </svg>
            </button>
            <span class="day">{{ dayLabelText }}</span>
            <button
              class="arrow"
              type="button"
              title="后一日"
              :disabled="offset >= 0"
              @click="offset++"
            >
              <svg
                width="7"
                height="7"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2.4"
                stroke-linecap="round"
                stroke-linejoin="round"
              >
                <path d="m9 5 7 7-7 7" />
              </svg>
            </button>
          </div>

          <p v-if="loadError" class="load-error" role="alert">统计加载失败：{{ loadError }}</p>

          <div class="graph-block">
            <div class="graph">
              <svg ref="graphSvg" height="12" width="100%" preserveAspectRatio="none">
                <defs>
                  <clipPath id="graphClip"><rect width="100%" height="12" rx="6" /></clipPath>
                  <clipPath id="graphClipComet">
                    <rect ref="cometClipRect" width="100%" height="12" rx="6" />
                  </clipPath>
                  <linearGradient id="gWork" x1="0" y1="0" x2="0" y2="1">
                    <stop offset="0" class="sw-from" />
                    <stop offset="1" class="sw-to" />
                  </linearGradient>
                  <linearGradient id="gRest" x1="0" y1="0" x2="0" y2="1">
                    <stop offset="0" class="sr-from" />
                    <stop offset="1" class="sr-to" />
                  </linearGradient>
                  <linearGradient id="gDuty" x1="0" y1="0" x2="0" y2="1">
                    <stop offset="0" class="sd-from" />
                    <stop offset="1" class="sd-to" />
                  </linearGradient>
                  <linearGradient
                    id="gStripe"
                    gradientUnits="userSpaceOnUse"
                    x1="-21"
                    y1="-3"
                    x2="-3"
                    y2="15"
                  >
                    <stop offset="0" class="ss-edge" />
                    <stop offset="0.3535" class="ss-edge" />
                    <stop offset="0.3535" class="ss-mid" />
                    <stop offset="0.5498" class="ss-mid" />
                    <stop offset="0.5498" class="ss-edge" />
                    <stop offset="1" class="ss-edge" />
                  </linearGradient>
                </defs>
                <rect class="ghost" width="100%" height="12" rx="6" />
                <g id="graphBars" ref="barsEl" clip-path="url(#graphClip)"></g>
                <!-- 裁剪挂不动的外壳（clipPath 随引用元素坐标系走，挂平移组会被走廊带走） -->
                <g clip-path="url(#graphClipComet)">
                  <g ref="cometEl"></g>
                </g>
                <g id="graphNodes"></g>
              </svg>
            </div>
            <div class="chart-axis">
              <span>{{ axisStart }}</span
              ><span>{{ axisEnd }}</span>
            </div>
          </div>

          <div class="triple-row">
            <div class="gcard t-card t-duty" @click="flipped = true">
              <div class="tc-head">
                <span class="tc-icon">
                  <svg viewBox="-8 -8 16 16" width="16" height="16">
                    <circle r="7" />
                    <g
                      transform="translate(0 0) scale(0.3333) translate(-12 -12)"
                      fill="none"
                      stroke="currentColor"
                      stroke-width="2.5"
                      stroke-linecap="round"
                      stroke-linejoin="round"
                    >
                      <circle cx="12" cy="12" r="10" />
                      <polyline points="12 6 12 12 16 14" />
                    </g>
                  </svg>
                </span>
                <span class="tc-title"
                  ><svg viewBox="0 0 20 14" width="20" height="14">
                    <text x="10" y="7" dy="0.0" text-anchor="middle" dominant-baseline="central">
                      在岗
                    </text>
                  </svg></span
                >
                <span class="tc-percent"
                  ><svg
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    stroke-width="3"
                    stroke-linecap="round"
                    stroke-linejoin="round"
                  >
                    <path d="m18 15-6-6-6 6" />
                  </svg>
                  {{ pct(day?.duty_secs ?? 0) }}%</span
                >
              </div>
              <b>{{ fmtCN((day?.duty_secs ?? 0) / 60) }}</b>
              <span class="tc-hint">今日明细</span>
            </div>
            <div class="gcard t-card t-work">
              <div class="tc-head">
                <span class="tc-icon">
                  <svg viewBox="-8 -8 16 16" width="16" height="16">
                    <circle r="7" />
                    <g
                      transform="translate(0 0) scale(0.3333) translate(-12 -12)"
                      fill="none"
                      stroke="currentColor"
                      stroke-width="2.5"
                      stroke-linecap="round"
                      stroke-linejoin="round"
                    >
                      <path d="M16 20V4a2 2 0 0 0-2-2h-4a2 2 0 0 0-2 2v16" />
                      <rect width="20" height="14" x="2" y="6" rx="2" />
                    </g>
                  </svg>
                </span>
                <span class="tc-title"
                  ><svg viewBox="0 0 20 14" width="20" height="14">
                    <text x="10" y="7" dy="0" text-anchor="middle" dominant-baseline="central">
                      工作
                    </text>
                  </svg></span
                >
                <span class="tc-percent"
                  ><svg
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    stroke-width="3"
                    stroke-linecap="round"
                    stroke-linejoin="round"
                  >
                    <path d="m18 15-6-6-6 6" />
                  </svg>
                  {{ pct(day?.work_secs ?? 0) }}%</span
                >
              </div>
              <b>{{ fmtCN((day?.work_secs ?? 0) / 60) }}</b>
              <div class="tc-range">
                <i class="fill" :style="{ width: `${pct(day?.work_secs ?? 0)}%` }"></i>
              </div>
            </div>
            <div class="gcard t-card t-rest">
              <div class="tc-head">
                <span class="tc-icon">
                  <svg viewBox="-8 -8 16 16" width="16" height="16">
                    <circle r="7" />
                    <g
                      transform="translate(0.3 0) scale(0.3333) translate(-12 -12)"
                      fill="none"
                      stroke="currentColor"
                      stroke-width="2.5"
                      stroke-linecap="round"
                      stroke-linejoin="round"
                    >
                      <path d="M10 2v2" />
                      <path d="M14 2v2" />
                      <path
                        d="M16 8a1 1 0 0 1 1 1v8a4 4 0 0 1-4 4H7a4 4 0 0 1-4-4V9a1 1 0 0 1 1-1h14a4 4 0 1 1 0 8h-1"
                      />
                      <path d="M6 2v2" />
                    </g>
                  </svg>
                </span>
                <span class="tc-title"
                  ><svg viewBox="0 0 20 14" width="20" height="14">
                    <text x="10" y="7" dy="0" text-anchor="middle" dominant-baseline="central">
                      休息
                    </text>
                  </svg></span
                >
                <span class="tc-percent"
                  ><svg
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    stroke-width="3"
                    stroke-linecap="round"
                    stroke-linejoin="round"
                  >
                    <path d="m6 9 6 6 6-6" />
                  </svg>
                  {{ pct(day?.rest_secs ?? 0) }}%</span
                >
              </div>
              <b>{{ fmtCN((day?.rest_secs ?? 0) / 60) }}</b>
              <div class="tc-range">
                <i class="fill" :style="{ width: `${pct(day?.rest_secs ?? 0)}%` }"></i>
              </div>
            </div>
          </div>

          <div class="gcard week-card">
            <div class="wc-head">
              <span class="wc-left">
                <span class="wc-label">日均</span>
                <b class="wc-num">{{ fmtCN(avgWorkMin) }}</b>
              </span>
              <!-- 与上周相比：后端暂无上周均值数据源，delta 行按设计结构占位（PL020 评估） -->
            </div>
            <div class="wc-chart">
              <div class="wc-plot">
                <div ref="wcAvgLine" class="wc-avg"></div>
                <svg id="wcSvg" ref="wcSvg" class="wc-svg" preserveAspectRatio="none">
                  <g id="wcBars" ref="wcBars"></g>
                  <path id="wcSelRing" ref="wcSelRing" />
                </svg>
              </div>
              <div class="wc-days">
                <span>一</span><span>二</span><span>三</span><span>四</span><span>五</span
                ><span>六</span><span class="today">今</span>
              </div>
            </div>
          </div>
        </section>

        <!-- 背面：今日/当日明细 -->
        <section class="detail-face sheet flip-face back">
          <h3>{{ detailTitle() }}</h3>
          <div class="detail">
            <div v-for="(r, i) in detailRows" :key="i" class="dl">
              <span class="time"
                >{{ fmtT(r.startMin) }}{{ r.ongoing ? "" : `–${fmtT(r.endMin)}` }}</span
              >
              <span class="track">
                <span
                  class="blk"
                  :class="r.kind"
                  :data-secs="r.secs"
                  :data-ongoing="r.ongoing ? '' : undefined"
                  :style="{ width: blkWidth(r.secs) }"
                ></span>
              </span>
              <span class="dur">{{ durText(r.secs) }}</span>
              <span v-if="r.ongoing" class="ongoing">至今</span>
            </div>
            <p v-if="detailRows.length === 0 && !loadError" class="empty">本日无打卡记录</p>
          </div>
          <button class="flip-back" type="button" @click="flipped = false">返回统计</button>
        </section>
      </div>
    </div>
  </div>
</template>
