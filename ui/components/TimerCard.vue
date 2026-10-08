<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";

// IPC DTO 镜像类型统一在 types.ts（单一来源 = Rust serde 结构）
import type { DaySummary, SessionStatus } from "../types";
// 展示格式化共享助手（FIX002.13 收敛）
import { pad } from "../format";

// 轮询周期 100ms：十分秒位（HH:MM:SS.d）每 0.1s 跳动需要 ≤100ms 拉取
const TICK_MS = 100;

// 动作（开始/暂停/继续/重开）后通知父组件刷新统计；失败上抛文案；
// clock：打卡请求上抛（确认框由父组件持有）
const emit = defineEmits<{
  changed: [];
  error: [string];
  clock: [direction: "in" | "out"];
}>();

// day = 当日明细（dial 弧段 + 电池上班时刻的数据源）；thresholdMin = 沙漏周期；
// targetHours = 自动下班小时（电池分母 + dial 底轨跨度）
const props = defineProps<{
  day: DaySummary | null;
  thresholdMin: number;
  targetHours: number | null;
}>();

const state = ref<"idle" | "running" | "paused">("idle");
const totalMs = ref(0);
// 在岗态（PL005）：未上班时计时控制置灰禁用 + pill 方向
const onDuty = ref(false);
// tick 时戳：电池/指针/日期卡的反应性时间源（100ms 刷新）
const nowTs = ref(Date.now());
// 指针/日期卡每秒档；dial 弧每秒重建
const secTs = ref(Date.now());
let timer: number | undefined;
let lastSec = -1;

const isRunning = computed(() => state.value === "running");

/** 拉取会话快照（tick 定案：前端 setInterval 拉取，Rust 不推送）+ 推进时敏渲染 */
async function refresh(): Promise<void> {
  try {
    const snapshot = await invoke<SessionStatus>("session_status");
    state.value = snapshot.state;
    totalMs.value = snapshot.total_ms;
    onDuty.value = snapshot.on_duty;
  } catch (err) {
    console.error("session_status 调用失败", err);
  }
  const now = Date.now();
  nowTs.value = now;
  const sec = Math.floor(now / 1000);
  if (sec !== lastSec) {
    lastSec = sec;
    secTs.value = now;
  }
  tickHourglass();
  tickBattery();
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

/* ============================================================
   ① 日期卡（design renderDate 1:1）：12 小时制 + 英文长日期 + 昼夜类
   ============================================================ */
const WEEK_EN = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];
const MONTH_EN = [
  "January",
  "February",
  "March",
  "April",
  "May",
  "June",
  "July",
  "August",
  "September",
  "October",
  "November",
  "December",
];

function ordinal(n: number): string {
  const s = ["th", "st", "nd", "rd"];
  const v = n % 100;
  return n + (s[(v - 20) % 10] || s[v] || s[0]);
}

const dcTime = computed(() => {
  const d = new Date(nowTs.value);
  return `${d.getHours() % 12 || 12}:${pad(d.getMinutes())}`;
});
const dcAmpm = computed(() => (new Date(nowTs.value).getHours() < 12 ? "AM" : "PM"));
const dcDay = computed(() => {
  const d = new Date(nowTs.value);
  return `${WEEK_EN[d.getDay()]}, ${MONTH_EN[d.getMonth()]} ${ordinal(d.getDate())}`;
});
const isDaytime = computed(() => {
  const h = new Date(nowTs.value).getHours();
  return h >= 6 && h < 18;
});

/* ============================================================
   ② dial 区间时钟（design renderDial/arcPath 1:1）：上班角起底轨 + 块弧 + 三针
   ============================================================ */
const DIAL_C = 90;
const DIAL_R = 72;

function angleOf(ts: number): number {
  const d = new Date(ts);
  return (((d.getHours() % 12) * 60 + d.getMinutes() + d.getSeconds() / 60) / 720) * 360;
}

function polar(r: number, deg: number): [number, number] {
  const a = ((deg - 90) * Math.PI) / 180;
  return [DIAL_C + r * Math.cos(a), DIAL_C + r * Math.sin(a)];
}

function arcPath(r: number, a0: number, a1: number): string {
  const delta = (((a1 - a0) % 360) + 360) % 360;
  if (delta < 0.5) {
    return "";
  }
  const [x0, y0] = polar(r, a0);
  const [x1, y1] = polar(r, a0 + delta);
  const large = delta > 180 ? 1 : 0;
  return `M${x0.toFixed(2)} ${y0.toFixed(2)} A${r} ${r} 0 ${large} 1 ${x1.toFixed(2)} ${y1.toFixed(2)}`;
}

const autoOutMs = computed(() => (props.targetHours ?? 8) * 3_600_000);
const clockInMs = computed(() => (props.day?.duty_started_at ?? 0) * 1000);

const hourStyle = computed(() => {
  const d = new Date(secTs.value);
  const ang = ((d.getHours() % 12) + d.getMinutes() / 60) * 30;
  return { transform: `rotate(${ang}deg)` };
});
const minuteStyle = computed(() => {
  const d = new Date(secTs.value);
  const ang = (d.getMinutes() + d.getSeconds() / 60) * 6;
  return { transform: `rotate(${ang}deg)` };
});
const secondStyle = computed(() => ({
  transform: `rotate(${new Date(secTs.value).getSeconds() * 6}deg)`,
}));

/** dial 弧整层 SVG（design 1:1）：底轨 + 各块弧（记录只画到当前时刻） */
const dialArcsSvg = computed(() => {
  if (!props.day?.duty_started_at) {
    return "";
  }
  const clockIn = clockInMs.value;
  const autoOut = autoOutMs.value;
  let svg = `<path class="dial-track" d="${arcPath(DIAL_R, angleOf(clockIn), angleOf(clockIn) + (autoOut / 3_600_000 / 12) * 360)}" />`;
  const nowTsMs = Date.now();
  const endTs = clockIn + autoOut;
  for (const b of props.day.blocks) {
    const t0 = b.start * 1000;
    const t1 = Math.min(b.end * 1000, nowTsMs, endTs);
    if (t1 <= t0) {
      continue;
    }
    const sa = angleOf(t0);
    const delta = ((t1 - t0) / (12 * 3_600_000)) * 360;
    svg += `<path class="dial-arc ${b.kind}" d="${arcPath(DIAL_R, sa, sa + delta)}" />`;
  }
  return svg;
});

/* ============================================================
   ③ flip-clock 翻转计时（design 1:1）：JS 生成 7 格 + flap 节点插拔
   ============================================================ */
const flipClockEl = ref<HTMLElement | null>(null);
let flipEls: HTMLElement[] = [];
const reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)");

function buildFlipClock(): void {
  const host = flipClockEl.value;
  if (!host) {
    return;
  }
  host.innerHTML = "";
  const layout = ["d", "d", ":", "d", "d", ":", "d", "d", ".", "s"] as const;
  for (const t of layout) {
    if (t === ":") {
      const sep = document.createElement("span");
      sep.className = "flip-sep";
      sep.textContent = ":";
      host.append(sep);
    } else if (t === ".") {
      const sep = document.createElement("span");
      sep.className = "flip-sep";
      sep.textContent = ".";
      host.append(sep);
    } else {
      const cell = document.createElement("div");
      cell.className = t === "s" ? "flip small" : "flip";
      cell.dataset.v = "0";
      cell.innerHTML =
        '<div class="half top"><span>0</span></div><div class="half bottom"><span>0</span></div>';
      host.append(cell);
    }
  }
  flipEls = [...host.querySelectorAll<HTMLElement>(".flip")];
}

/** 单格翻页（design setFlip 1:1）：静态两面归旧值 → 插 flap → animationend 收尾 */
function setFlip(el: HTMLElement, v: string): void {
  if (el.dataset.v === v) {
    return;
  }
  const old = el.dataset.v ?? "0";
  el.dataset.v = v;
  const top = el.querySelector<HTMLElement>(".half.top");
  const bottom = el.querySelector<HTMLElement>(".half.bottom");
  if (!top || !bottom) {
    return;
  }
  top.firstElementChild!.textContent = old;
  bottom.firstElementChild!.textContent = old;
  el.querySelectorAll(".flap").forEach((n) => n.remove());
  if (reducedMotion.matches) {
    top.firstElementChild!.textContent = v;
    bottom.firstElementChild!.textContent = v;
    return;
  }
  const f1 = document.createElement("div");
  f1.className = "half top flap flap-top";
  f1.innerHTML = `<span>${old}</span>`;
  const f2 = document.createElement("div");
  f2.className = "half bottom flap flap-bottom";
  f2.innerHTML = `<span>${v}</span>`;
  f1.onanimationend = () => {
    top.firstElementChild!.textContent = v;
    f1.remove();
  };
  f2.onanimationend = () => {
    bottom.firstElementChild!.textContent = v;
    f2.remove();
  };
  el.append(f1, f2);
}

/** 十分秒位拆位（design 1:1）：小时不封顶 */
watch(totalMs, (ms) => {
  if (flipEls.length === 0) {
    return;
  }
  const t = Math.floor(ms / 100);
  const d = t % 10;
  const sec = Math.floor(t / 10) % 60;
  const min = Math.floor(t / 600) % 60;
  const hr = Math.floor(t / 36000);
  const digits = [
    String(Math.floor(hr / 10) % 10),
    String(hr % 10),
    String(Math.floor(min / 10) % 10),
    String(min % 10),
    String(Math.floor(sec / 10) % 10),
    String(sec % 10),
    String(d),
  ];
  digits.forEach((v, i) => setFlip(flipEls[i]!, v));
});

/* ============================================================
   ④ 沙漏（design 状态机 1:1）：--hg-dur/--hg-delay + .run/.flowing + model 翻转
   ============================================================ */
const hourglassEl = ref<HTMLElement | null>(null);
const HG_FLIP = 0.6; // 秒，与 CSS --hg-flip 一致
const HG_DRAIN = 0.8; // 暂停快速漏完时长（秒）
const HG_MODEL_T = "translate(13.75px, 9.25px)";
let hgKey = "";
let hgFlowTimer = 0;
let hgPrevRun = false;
let hgThres = props.thresholdMin;
let hgP0 = 0; // 水位记录（0 = 上满下空，1 = 上空下满）
let hgT0 = Date.now();
let hgDur = props.thresholdMin * 60_000;

function hgMs(): number {
  return props.thresholdMin * 60_000;
}

function hgPNow(): number {
  return Math.min(1, hgP0 + (Date.now() - hgT0) / hgDur);
}

function hgClearRun(): void {
  if (hgFlowTimer !== 0) {
    window.clearTimeout(hgFlowTimer);
    hgFlowTimer = 0;
  }
  hourglassEl.value?.classList.remove("flowing", "run");
}

/** 翻半圈：先无过渡钉 -180° 一帧，再清样式过渡回 0°（design 1:1） */
function hgFlipRestart(): void {
  const model = hourglassEl.value?.querySelector<HTMLElement>(".loader__model");
  if (!model) {
    return;
  }
  model.style.transition = "none";
  model.style.transform = `${HG_MODEL_T} rotate(-180deg)`;
  void model.getBoundingClientRect();
  hourglassEl.value?.classList.add("run");
  model.style.transition = "";
  model.style.transform = "";
}

/** 沙漏状态机（design 1:1，100ms tick 内调用，hgKey 去重） */
function tickHourglass(): void {
  const hg = hourglassEl.value;
  if (!hg) {
    return;
  }
  const done = hgPNow() >= 1;
  const key = `${isRunning.value}|${props.thresholdMin}|${done}`;
  if (key === hgKey) {
    return;
  }
  hgKey = key;
  const started = isRunning.value && !hgPrevRun;
  const thresChanged = props.thresholdMin !== hgThres;
  const pCur = hgPNow();
  hgPrevRun = isRunning.value;
  hgThres = props.thresholdMin;

  if ((started || thresChanged) && isRunning.value) {
    // ① 新工作段：翻正回满 + 按阈值匀速漏
    hgClearRun();
    hg.style.setProperty("--hg-dur", `${hgMs() / 1000}s`);
    hg.style.setProperty("--hg-delay", `${HG_FLIP}s`);
    hgP0 = 0;
    hgDur = hgMs();
    hgT0 = Date.now();
    hgFlipRestart();
    hgFlowTimer = window.setTimeout(() => hg.classList.add("flowing"), HG_FLIP * 1000);
  } else if (thresChanged) {
    // ①b 阈值变了但暂停：只重置内部水位
    hgP0 = 0;
    hgDur = hgMs();
    hgT0 = Date.now();
  } else if (!isRunning.value && !done) {
    // ② 暂停且还有沙：翻半圈 + 从当前水位快漏
    hgClearRun();
    hg.style.setProperty("--hg-dur", `${HG_DRAIN}s`);
    hg.style.setProperty("--hg-delay", `${-(1 - pCur) * HG_DRAIN}s`);
    hgP0 = 1 - pCur;
    hgDur = HG_DRAIN * 1000;
    hgT0 = Date.now();
    hgFlipRestart();
  } else if (isRunning.value && done) {
    // ③ 工作漏完：不翻转，负一周期停在终态
    hgClearRun();
    hg.style.setProperty("--hg-dur", `${hgMs() / 1000}s`);
    hg.style.setProperty("--hg-delay", `${-hgMs() / 1000}s`);
    hgP0 = 1;
    hgDur = 1;
    hgT0 = Date.now();
    void hg.getBoundingClientRect();
    hg.classList.add("run");
  }
}

// 阈值变更强制重置（design：hgKey = ""）
watch(
  () => props.thresholdMin,
  () => {
    hgKey = "";
  },
);

/* ============================================================
   ⑤ 电池（design 1:1）：工作日进度（墙钟），下班冻结，阈值色三档
   ============================================================ */
const batPct = ref(100);
const batMid = ref(false);
const batLow = ref(false);
let batFrozen: number | null = null;

function clamp01(n: number): number {
  return Math.min(1, Math.max(0, n));
}

function batteryRatio(): number {
  if (!onDuty.value && batFrozen != null) {
    return batFrozen;
  }
  if (!props.day?.duty_started_at) {
    return 1;
  }
  return clamp01(1 - (Date.now() - clockInMs.value) / autoOutMs.value);
}

function tickBattery(): void {
  const ratio = batteryRatio();
  batPct.value = Math.round(ratio * 100);
  batMid.value = ratio >= 0.2 && ratio < 0.45;
  batLow.value = ratio < 0.2;
}

// 下班瞬间冻结电量（design：confirmOk 同口径）；上班/新班解冻
watch(onDuty, (duty, prev) => {
  if (prev && !duty) {
    batFrozen = batteryRatio();
  }
  if (!prev && duty) {
    batFrozen = null;
  }
});

/* ============================================================
   ⑥ 三段式控制（design 语义）：work = 开始/继续，rest = 暂停，reset = 重开；
   重置后 duty=true + 空闲 → 休息段呈选中态（已暂停归零）
   ============================================================ */
const runMode = computed(() => (!onDuty.value ? "" : isRunning.value ? "work" : "rest"));

function onWork(): void {
  if (!onDuty.value || isRunning.value) {
    return;
  }
  void act(state.value === "idle" ? "session_start" : "session_resume");
}

function onRest(): void {
  if (isRunning.value) {
    void act("session_pause");
  }
}

/* ============================================================
   生命周期
   ============================================================ */
onMounted(() => {
  buildFlipClock();
  void refresh();
  timer = window.setInterval(() => void refresh(), TICK_MS);
});

onUnmounted(() => {
  if (timer !== undefined) {
    window.clearInterval(timer);
  }
  if (hgFlowTimer !== 0) {
    window.clearTimeout(hgFlowTimer);
  }
});
</script>

<template>
  <!-- 片段根（无包装层）：设计 .stage 的 flex gap 链要求五件套为直接子件（CT 坑 2 教训） -->
  <!-- ① 日期卡（左）+ 区间时钟（右），等高 -->
  <div class="timer-top">
    <div class="date-card" :class="{ day: isDaytime }">
      <div class="dc-time-row">
        <span class="dc-time">{{ dcTime }}</span>
        <span class="dc-ampm">{{ dcAmpm }}</span>
      </div>
      <div class="dc-day">{{ dcDay }}</div>
      <svg
        class="dc-ic dc-moon"
        xmlns="http://www.w3.org/2000/svg"
        viewBox="0 0 16 16"
        fill="currentColor"
        aria-hidden="true"
      >
        <path
          d="M6 .278a.768.768 0 0 1 .08.858 7.208 7.208 0 0 0-.878 3.46c0 4.021 3.278 7.277 7.318 7.277.527 0 1.04-.055 1.533-.16a.787.787 0 0 1 .81.316.733.733 0 0 1-.031.893A8.349 8.349 0 0 1 8.344 16C3.734 16 0 12.286 0 7.71 0 4.266 2.114 1.312 5.124.06A.752.752 0 0 1 6 .278z"
        />
        <path
          d="M10.794 3.148a.217.217 0 0 1 .412 0l.387 1.162c.173.518.579.924 1.097 1.097l1.162.387a.217.217 0 0 1 0 .412l-1.162.387a1.734 1.734 0 0 0-1.097 1.097l-.387 1.162a.217.217 0 0 1-.412 0l-.387-1.162A1.734 1.734 0 0 0 9.31 6.593l-1.162-.387a.217.217 0 0 1 0-.412l1.162-.387a1.734 1.734 0 0 0 1.097-1.097l.387-1.162zM13.863.099a.145.145 0 0 1 .274 0l.258.774c.115.346.386.617.732.732l.774.258a.145.145 0 0 1 0 .274l-.774.258a1.156 1.156 0 0 0-.732.732l-.258.774a.145.145 0 0 1-.274 0l-.258-.774a1.156 1.156 0 0 0-.732-.732l-.774-.258a.145.145 0 0 1 0-.274l.774-.258c.346-.115.617-.386.732-.732L13.863.1z"
        />
      </svg>
      <svg
        class="dc-ic dc-sun"
        xmlns="http://www.w3.org/2000/svg"
        viewBox="0 0 16 16"
        fill="currentColor"
        aria-hidden="true"
      >
        <path
          d="M8 12a4 4 0 1 0 0-8 4 4 0 0 0 0 8zM8 0a.5.5 0 0 1 .5.5v2a.5.5 0 0 1-1 0v-2A.5.5 0 0 1 8 0zm0 13a.5.5 0 0 1 .5.5v2a.5.5 0 0 1-1 0v-2A.5.5 0 0 1 8 13zm8-5a.5.5 0 0 1-.5.5h-2a.5.5 0 0 1 0-1h2a.5.5 0 0 1 .5.5zM3 8a.5.5 0 0 1-.5.5h-2a.5.5 0 0 1 0-1h2A.5.5 0 0 1 3 8zm10.657-5.657a.5.5 0 0 1 0 .707l-1.414 1.415a.5.5 0 1 1-.707-.708l1.414-1.414a.5.5 0 0 1 .707 0zm-9.193 9.193a.5.5 0 0 1 0 .707L3.05 13.657a.5.5 0 0 1-.707-.707l1.414-1.414a.5.5 0 0 1 .707 0zm9.193 2.121a.5.5 0 0 1-.707 0l-1.414-1.414a.5.5 0 0 1 .707-.707l1.414 1.414a.5.5 0 0 1 0 .707zM4.464 4.465a.5.5 0 0 1-.707 0L2.343 3.05a.5.5 0 1 1 .707-.707l1.414 1.414a.5.5 0 0 1 0 .708z"
        />
      </svg>
    </div>
    <div class="dial">
      <div class="face">
        <p class="v-index">II</p>
        <p class="h-index">II</p>
        <!-- eslint-disable-next-line vue/no-v-html -->
        <svg class="face-arcs" viewBox="0 0 180 180" aria-hidden="true">
          <g v-html="dialArcsSvg"></g>
        </svg>
        <div class="hour" :style="hourStyle"></div>
        <div class="minute" :style="minuteStyle"></div>
        <div class="second" :style="secondStyle"></div>
      </div>
    </div>
  </div>

  <!-- ② 翻转计时（时 : 分 : 秒 . 十分位，JS 生成 7 格） -->
  <div ref="flipClockEl" class="flip-clock"></div>

  <!-- ③ 沙漏（提醒阈值计时器）+ START + 电量 -->
  <div class="status-row">
    <svg
      id="hourglass"
      ref="hourglassEl"
      class="hourglass"
      viewBox="0 0 56 56"
      role="img"
      aria-label="提醒阈值沙漏"
    >
      <clipPath id="sand-mound-top">
        <path
          d="M 14.613 13.087 C 15.814 12.059 19.3 8.039 20.3 6.539 C 21.5 4.789 21.5 2.039 21.5 2.039 L 3 2.039 C 3 2.039 3 4.789 4.2 6.539 C 5.2 8.039 8.686 12.059 9.887 13.087 C 11 14.039 12.25 14.039 12.25 14.039 C 12.25 14.039 13.5 14.039 14.613 13.087 Z"
          class="loader__sand-mound-top"
        ></path>
      </clipPath>
      <clipPath id="sand-mound-bottom">
        <path
          d="M 14.613 20.452 C 15.814 21.48 19.3 25.5 20.3 27 C 21.5 28.75 21.5 31.5 21.5 31.5 L 3 31.5 C 3 31.5 3 28.75 4.2 27 C 5.2 25.5 8.686 21.48 9.887 20.452 C 11 19.5 12.25 19.5 12.25 19.5 C 12.25 19.5 13.5 19.5 14.613 20.452 Z"
          class="loader__sand-mound-bottom"
        ></path>
      </clipPath>
      <g transform="translate(2,2)">
        <g
          transform="rotate(-90,26,26)"
          stroke-linecap="round"
          stroke-dashoffset="153.94"
          stroke-dasharray="153.94 153.94"
          stroke="hsl(0,0%,100%)"
          fill="none"
        >
          <circle
            transform="rotate(0,26,26)"
            r="24.5"
            cy="26"
            cx="26"
            stroke-width="2.5"
            class="loader__motion-thick"
          ></circle>
          <circle
            transform="rotate(90,26,26)"
            r="24.5"
            cy="26"
            cx="26"
            stroke-width="1.75"
            class="loader__motion-medium"
          ></circle>
          <circle
            transform="rotate(180,26,26)"
            r="24.5"
            cy="26"
            cx="26"
            stroke-width="1"
            class="loader__motion-thin"
          ></circle>
        </g>
        <g transform="translate(13.75,9.25)" class="loader__model">
          <g class="loader__shadow">
            <use href="#hgBody"></use>
            <rect height="2" width="24.5"></rect>
            <rect height="2" width="24.5" y="31.5"></rect>
          </g>
          <path
            id="hgBody"
            d="M 1.5 2 L 23 2 C 23 2 22.5 8.5 19 12 C 16 15.5 13.5 13.5 13.5 16.75 C 13.5 20 16 18 19 21.5 C 22.5 25 23 31.5 23 31.5 L 1.5 31.5 C 1.5 31.5 2 25 5.5 21.5 C 8.5 18 11 20 11 16.75 C 11 13.5 8.5 15.5 5.5 12 C 2 8.5 1.5 2 1.5 2 Z"
            class="loader__body"
          ></path>

          <g stroke-linecap="round" stroke="hsl(35,90%,78%)">
            <line
              y2="20.75"
              x2="12"
              y1="15.75"
              x1="12"
              stroke-dasharray="0.25 33.75"
              stroke-width="1"
              class="loader__sand-grain-left"
            ></line>
            <line
              y2="21.75"
              x2="12.5"
              y1="16.75"
              x1="12.5"
              stroke-dasharray="0.25 33.75"
              stroke-width="1"
              class="loader__sand-grain-right"
            ></line>
            <line
              y2="31.5"
              x2="12.25"
              y1="18"
              x1="12.25"
              stroke-dasharray="0.5 107.5"
              stroke-width="1"
              class="loader__sand-drop"
            ></line>
            <line
              y2="31.5"
              x2="12.25"
              y1="14.75"
              x1="12.25"
              stroke-dasharray="54 54"
              stroke-width="1.5"
              class="loader__sand-fill"
            ></line>
            <line
              y2="31.5"
              x2="12"
              y1="16"
              x1="12"
              stroke-dasharray="1 107"
              stroke-width="1"
              stroke="hsl(35,90%,83%)"
              class="loader__sand-line-left"
            ></line>
            <line
              y2="31.5"
              x2="12.5"
              y1="16"
              x1="12.5"
              stroke-dasharray="12 96"
              stroke-width="1"
              stroke="hsl(35,90%,83%)"
              class="loader__sand-line-right"
            ></line>

            <g stroke-width="0" fill="hsl(35,90%,78%)">
              <path
                d="M 12.25 15 L 15.392 13.486 C 21.737 11.168 22.5 2 22.5 2 L 2 2.013 C 2 2.013 2.753 11.046 9.009 13.438 L 12.25 15 Z"
                clip-path="url(#sand-mound-top)"
              ></path>
              <path
                d="M 12.25 18.5 L 15.392 20.014 C 21.737 22.332 22.5 31.5 22.5 31.5 L 2 31.487 C 2 31.487 2.753 22.454 9.009 20.062 Z"
                clip-path="url(#sand-mound-bottom)"
              ></path>
            </g>

            <g stroke-width="2" stroke-linecap="round" opacity="0.7" fill="none">
              <path
                d="M 19.437 3.421 C 19.437 3.421 19.671 6.454 17.914 8.846 C 16.157 11.238 14.5 11.5 14.5 11.5"
                stroke="hsl(0,0%,100%)"
                class="loader__glare-top"
              ></path>
              <path
                transform="rotate(180,12.25,16.75)"
                d="M 19.437 3.421 C 19.437 3.421 19.671 6.454 17.914 8.846 C 16.157 11.238 14.5 11.5 14.5 11.5"
                stroke="hsla(0,0%,100%,0)"
                class="loader__glare-bottom"
              ></path>
            </g>

            <!-- 上下盖（不继承沙粒组的沙色描边，纯黑：#000 + #333 高光） -->
            <rect height="2" width="24.5" fill="#000" stroke="none"></rect>
            <rect
              height="1"
              width="19.5"
              y="0.5"
              x="2.5"
              ry="0.5"
              rx="0.5"
              fill="#333"
              stroke="none"
            ></rect>
            <rect height="2" width="24.5" y="31.5" fill="#000" stroke="none"></rect>
            <rect
              height="1"
              width="19.5"
              y="32"
              x="2.5"
              ry="0.5"
              rx="0.5"
              fill="#333"
              stroke="none"
            ></rect>
          </g>
        </g>
      </g>
    </svg>
    <span class="start-label" :class="{ working: isRunning }">
      <span class="sl-base">START</span>
      <span class="sl-fill">START</span>
    </span>
    <div class="battery" :class="{ mid: batMid, low: batLow }">
      <div class="bat-shell"><div class="bat-fill" :style="{ width: `${batPct}%` }"></div></div>
      <div class="bat-nub"></div>
      <span class="bat-pct">{{ batPct }}%</span>
    </div>
  </div>

  <!-- ④ 打卡 pill（上班/下班；确认框由父组件持有） -->
  <button class="pill-cast" type="button" @click="emit('clock', onDuty ? 'out' : 'in')">
    <svg
      width="14"
      height="14"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      stroke-width="2.4"
      stroke-linecap="round"
      stroke-linejoin="round"
    >
      <path d="M8 7V5a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" />
      <rect x="4" y="7" width="16" height="13" rx="2" />
    </svg>
    <span>{{ onDuty ? "下班" : "上班" }}</span>
  </button>

  <!-- ⑤ 三段式控制：工作/休息互斥选中；重置 = 瞬时（按下回弹，不保持选中） -->
  <div class="seg-control" :class="{ off: !onDuty }">
    <label class="seg">
      <input
        type="radio"
        name="runseg"
        value="work"
        :disabled="!onDuty"
        :checked="runMode === 'work'"
        @change="onWork"
      />
      <span class="text">工作</span>
    </label>
    <label class="seg">
      <input
        type="radio"
        name="runseg"
        value="rest"
        :disabled="!onDuty"
        :checked="runMode === 'rest'"
        @change="onRest"
      />
      <span class="text">休息</span>
    </label>
    <button class="seg seg-reset" type="button" :disabled="!onDuty" @click="act('session_restart')">
      <span class="text">重置</span>
    </button>
  </div>
</template>
