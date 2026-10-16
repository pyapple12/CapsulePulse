<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";

// IPC DTO 镜像类型统一在 types.ts（单一来源 = Rust serde 结构）
import {
  BATTERY_RED_RATIO,
  BATTERY_TOTAL_HOURS,
  BATTERY_YELLOW_FLOOR,
  type DaySummary,
  type SessionStatus,
} from "../types";
// 展示格式化共享助手（FIX002.13 收敛）
import { pad } from "../format";
// 段序列共享构建（FIX006.4：表盘 / 电池 / 图谱单一实现）
import { daySegments } from "../src/day-segments";

// 轮询周期 100ms：十分秒位（HH:MM:SS.d）每 0.1s 跳动需要 ≤100ms 拉取
const TICK_MS = 100;

// 动作（开始/暂停/继续/重开）后通知父组件刷新统计；失败上抛文案；
// clock：打卡请求上抛（确认框由父组件持有）
const emit = defineEmits<{
  changed: [];
  error: [string];
  clock: [direction: "in" | "out"];
  /** PATCH010.8：会话提示（**分钟粒度**、只在变化时发）——设置板阈值行的下限与锁定判据；
   * 走"TimerCard 顺手上报 → App → 设置板"（用户定案：单一来源、零时差、零额外 IPC） */
  session: [running: boolean, segmentMin: number, reminderFired: boolean];
}>();

// day = 当日明细（dial 弧段 + 电池的数据源）；thresholdMin = 沙漏周期；
// autoOutHours = 今日基准（PATCH006.4：电池黄阈值 `max(30%, 1 − 基准/12)` 需要它）
// FIX006.3：二者随 `settings` 而来，**未就绪 / 读取失败时为 undefined**（前端不造默认值）——
// 沙漏不画水位、电池不判加班区；可见错误由 App 侧统一弹（文案条）
const props = defineProps<{
  day: DaySummary | null;
  thresholdMin?: number;
  autoOutHours?: number;
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

/** 会话提示上抛（PATCH010.8）：**分钟粒度 + 只在变化时发**——设置板阈值行据此收窄下限 / 置灰。
 * `segmentMin` 取 `ceil(本段毫秒 / 60000)`，与后端下限口径（`ceil(本段秒 / 60) + 1`）同源。 */
let sessionHintKey = "";
function emitSessionHint(stateKey: string, segmentMs: number, reminderFired: boolean): void {
  const segmentMin = Math.ceil(segmentMs / 60_000);
  const key = `${stateKey}|${segmentMin}|${reminderFired}`;
  if (key === sessionHintKey) {
    return;
  }
  sessionHintKey = key;
  emit("session", stateKey === "running", segmentMin, reminderFired);
}

/** 拉取会话快照（tick 定案：前端 setInterval 拉取，Rust 不推送）+ 推进时敏渲染 */
async function refresh(): Promise<void> {
  let segmentMs = 0;
  try {
    const snapshot = await invoke<SessionStatus>("session_status");
    state.value = snapshot.state;
    totalMs.value = snapshot.total_ms;
    onDuty.value = snapshot.on_duty;
    segmentMs = snapshot.segment_ms;
    emitSessionHint(snapshot.state, snapshot.segment_ms, snapshot.reminder_fired);
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
  tickHourglass(segmentMs);
  tickBattery();
}

/** 执行命令后立即刷新快照与统计（不等下一 tick）；失败上抛父组件可见反馈。
 * args 供带参命令使用（PATCH002：`session_set_run_mode` 的 mode） */
async function act(command: string, args?: Record<string, unknown>): Promise<void> {
  try {
    await invoke(command, args);
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

/** 序号英文后缀（`1st` / `2nd` / `3rd` / `4th`；**11–13 恒 `th`** 是英语例外，不能用最后一位判）。 */
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
   ② dial 区间时钟（design renderDial/arcPath 1:1；PL026.4 起双圈化）：
   外圈 = 当天 AM（00–12）、内圈 = 当天 PM（12–24）、内内圈 = 次日 AM（24–36）；
   半径 72/63/54、线宽 7、圈间恒 2px；**空圈什么都不画**；跨 12:00 的段与灰弧按边界切开；
   三针不动（12h 钟表、落外圈刻度）。
   ============================================================ */
const DIAL_C = 90;
/** 三圈半径（PL026.4）：外圈 AM / 内圈 PM / 内内圈 次日 AM（线宽 7、圈间 2px：72−7−2=63、63−7−2=54） */
const DIAL_RINGS = [72, 63, 54];
const RING_MS = 12 * 3_600_000;

/** 本地当天零点（毫秒）——分圈基准（跨零点后仍以"当天"为圈 0/1 的基准） */
function dayStartMs(ts: number): number {
  const d = new Date(ts);
  d.setHours(0, 0, 0, 0);
  return d.getTime();
}

/** 时间戳 → (圈号, 圈内角度)：**以 day0（当天零点毫秒）为基准**——圈 0 = 00–12、圈 1 = 12–24、
 * 圈 2 = 24–36（次日 AM）。基准必须由调用方传入（不能用"ts 自己那天的零点"：跨零点后
 * 那会跳回圈 0，令切分原地打转）。 */
function ringPos(day0: number, ts: number): { ring: number; angle: number } {
  const hours = (ts - day0) / 3_600_000; // 距基准零点的小时数（跨零点 > 24）
  const ring = Math.min(DIAL_RINGS.length - 1, Math.max(0, Math.floor(hours / 12)));
  return { ring, angle: ((hours - ring * 12) / 12) * 360 };
}

/** 把 [t0, t1) 按 12h 圈边界切成若干 (圈, 起角, 止角) 片段（跨 12:00 / 跨零点逐圈画） */
function splitByRing(t0: number, t1: number): { ring: number; a0: number; a1: number }[] {
  const day0 = dayStartMs(t0);
  const out: { ring: number; a0: number; a1: number }[] = [];
  let cur = t0;
  while (cur < t1) {
    const { ring, angle } = ringPos(day0, cur);
    const ringStart = day0 + ring * RING_MS;
    const end = Math.min(t1, ringStart + RING_MS);
    if (end <= cur) {
      break; // 兜底：不前进即停（超出第三圈 36h 的区间不再画，绝不原地打转）
    }
    out.push({ ring, a0: angle, a1: ((end - ringStart) / RING_MS) * 360 });
    cur = end;
  }
  return out;
}

/** 极坐标 → 画布坐标（**角度以 12 点为 0°、顺时针**：故 −90° 对齐 y 轴负向）。 */
function polar(r: number, deg: number): [number, number] {
  const a = ((deg - 90) * Math.PI) / 180;
  return [DIAL_C + r * Math.cos(a), DIAL_C + r * Math.sin(a)];
}

/** 弧线 path（表盘弧段绘制）：r = 半径、a0/a1 = 起止角（12 点为 0°、顺时针）；
 * **整圈（≥359.5°）走两段半圆收口**——单段 360° 在 SVG 里会被归一化成 0° 而画不出来。 */
function arcPath(r: number, a0: number, a1: number): string {
  // 整圈（恰好 12h）：两段半圆收口（单段 360° 会被归一化成 0 而画不出来）
  if (a1 - a0 >= 359.5) {
    const [x0, y0] = polar(r, 0);
    const [x1, y1] = polar(r, 180);
    return `M${x0.toFixed(2)} ${y0.toFixed(2)} A${r} ${r} 0 1 1 ${x1.toFixed(2)} ${y1.toFixed(2)} A${r} ${r} 0 1 1 ${x0.toFixed(2)} ${y0.toFixed(2)}`;
  }
  const delta = (((a1 - a0) % 360) + 360) % 360;
  if (delta < 0.5) {
    return "";
  }
  const [x0, y0] = polar(r, a0);
  const [x1, y1] = polar(r, a0 + delta);
  const large = delta > 180 ? 1 : 0;
  return `M${x0.toFixed(2)} ${y0.toFixed(2)} A${r} ${r} 0 ${large} 1 ${x1.toFixed(2)} ${y1.toFixed(2)}`;
}

/** 当前班起点（毫秒）：**后端派生** `shift_start`（FIX006.4——原前端按段序列自推一份、
 * 与 StatsView 各算一次；现两处共用后端同一字段）；未在岗为 0（那时不画底轨）。 */
const shiftStartMs = computed(() => (props.day?.shift_start ?? 0) * 1000);

/** 本班预算（毫秒，PATCH005.3）：后端 `remaining_secs` = 今日上限 − 本班前已累计，**班内恒定**
 * （跨班累计扣减、加班步进即时生效）——表盘底轨跨度与电池分母同一口径。 */
const shiftBudgetMs = computed(() => (props.day?.remaining_secs ?? 0) * 1000);

/** 到顶置灰（PATCH005.3）：今日已累计到 12h 硬顶 → 上班钮点不开（后端同样拒打卡）。
 * FIX006.4：判定下沉后端（`cap_reached`），前端不再自算、也不重复硬顶常量。
 * 仅未上班时才可能点"上班"，在岗时按钮是"下班"，故置灰条件含 `!onDuty`。 */
const capReached = computed(() => props.day?.cap_reached ?? false);

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

/** dial 弧整层 SVG（design 1:1；PL026.4–.6：三圈 + 班间灰弧 + 底轨在岗/下班口径） */
const dialArcsSvg = computed(() => {
  const d = props.day;
  if (!d?.duty_started_at) {
    return "";
  }
  // FIX006.20c：时间源改用 **`secTs`（每秒档 ref）**——原用 `Date.now()` 而该 computed 只依赖
  // `props.day`（30 秒才刷）⇒ 进行中弧最多滞后 30 秒（注释所称"每秒重建"名不副实）
  const nowMs = secTs.value;
  const segs = daySegments(d.blocks);
  if (segs.length === 0) {
    return "";
  }
  // 底轨口径（PL026.6；PATCH003.2/.3）：在岗 = **当前班**打卡 → +N（蓝条只在岗时画）；
  // 已下班/过去日 = 不画蓝条，块弧也不按预测裁剪（终点 = 当天末块末尾）。
  // 判据两来源（PATCH003.3 定案 a）：`day.duty_ended_at == null` 负责**即时**（打卡/下班动作后
  // `day` 立即刷，实测 0–3ms）；`onDuty`（100ms 轮询）负责**纠偏**——万一某次 `day_detail`
  // 拉取失败，`onDuty` 一变即可在 ≤100ms 内把在岗态纠正回来。代价仅"下班后蓝条最多多留 100ms"。
  const onDutyNow = onDuty.value || d.duty_ended_at == null;
  const shiftStart = shiftStartMs.value;
  // PATCH005.3：底轨跨度 = **本班预算**（后端单一来源；本班打卡 + 今日剩余）——跨班累计扣减，
  // 预算用满/到顶时剩余为 0（底轨退化为 0 长度 = 已无预算）；块弧裁剪保底到 now，
  // 免"到点兜底执行前"那一瞬真实块弧整段消失
  const trackEnd = shiftStart + shiftBudgetMs.value;
  const clipEnd = onDutyNow ? Math.max(trackEnd, nowMs) : Number.POSITIVE_INFINITY;
  const parts: string[] = [];

  /** 逐片段按所属圈画弧（跨 12:00 / 跨零点自动切开；裁剪到 now 与当前班底轨末端） */
  const draw = (kind: string, t0: number, t1: number): void => {
    const end = Math.min(t1, nowMs, clipEnd);
    if (end <= t0) {
      return;
    }
    for (const frag of splitByRing(t0, end)) {
      const path = arcPath(DIAL_RINGS[frag.ring] ?? DIAL_RINGS[0]!, frag.a0, frag.a1);
      if (path) {
        parts.push(`<path class="dial-arc ${kind}" d="${path}" />`);
      }
    }
  };

  // ① 底轨（在岗才画；垫在块弧之下）
  if (onDutyNow) {
    for (const frag of splitByRing(shiftStart, trackEnd)) {
      const path = arcPath(DIAL_RINGS[frag.ring] ?? DIAL_RINGS[0]!, frag.a0, frag.a1);
      if (path) {
        parts.push(`<path class="dial-track" d="${path}" />`);
      }
    }
  }
  // ② 真实块弧（work / rest）
  for (const s of segs) {
    if (s.kind !== "gap") {
      draw(s.kind, s.t0, s.t1);
    }
  }
  // ③ 班间灰弧（PL026.5：中性灰；无 D / 无字母 / 无小圆）
  for (const s of segs) {
    if (s.kind === "gap") {
      draw("gap", s.t0, s.t1);
    }
  }
  return parts.join("");
});

/* ============================================================
   ③ flip-clock 翻转计时（design 1:1）：JS 生成 7 格 + flap 节点插拔
   ============================================================ */
const flipClockEl = ref<HTMLElement | null>(null);
let flipEls: HTMLElement[] = [];
let lastSecShown = ""; // 秒位现值（PATCH005.2：变化即触发冒号/点号灭一下）
const reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)");

/** 一次性建好翻转钟的 7 格 DOM（时:分:秒）并缓存节点引用（此后只改文本与类，不再重建）。 */
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

/** PATCH005.2：冒号/点号随"秒位跳字"**同步灭一下**——严格同步 = 在翻页更新同一帧触发，
 * 不跑自由相位的周期动画。休息 / 未上班（非 Running）不触发；reduced-motion 下不动。 */
function blinkSeparators(): void {
  if (reducedMotion.matches) {
    return;
  }
  const seps = flipClockEl.value?.querySelectorAll<HTMLElement>(".flip-sep");
  seps?.forEach((el) => {
    el.classList.remove("blink");
    void el.offsetWidth; // 强制回流：同一帧内 remove+add 不会重放动画
    el.classList.add("blink");
  });
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
  // PATCH005.2：秒位跳字即触发（严格同步）；休息 / 未上班不闪
  const secs = `${digits[4]}${digits[5]}`;
  if (secs !== lastSecShown) {
    lastSecShown = secs;
    if (isRunning.value) {
      blinkSeparators();
    }
  }
});

/* ============================================================
   ④ 沙漏（PATCH010.7 重做：水位 = 后端"本段时长"单一来源）
   —— w = 0 上满下空、w = 1 沉底；**沉底是常态**（paused / idle / 下班 / 刚打开 APP）
   —— running 时按"锚点 + 线性插值"：w = w0 + (seg − seg0) × (1 − w0) / (T − seg0)
      ⇒ 改阈值时水位连续（w0 取当时水位）、终点精确（seg = T ⇒ w = 1，与铃响同刻）
   —— 判据是**会话态**（不是"是否在岗"：休息也在岗但沉底），且**不可拿 segment = 0 当沉底**
      （暂停时 segment 也是 0，按公式会误推成"上满"）
   —— `.run` 类随本次重做退役：水位由 CSS 变量 `--hg-w` 直接驱动，其默认值即沉底
   ============================================================ */
const hourglassEl = ref<HTMLElement | null>(null);
const HG_FLIP = 0.6; // 秒，与 CSS --hg-flip 一致（新段翻转）
const HG_MODEL_T = "translate(13.75px, 9.25px)";
/** 水位锚点：seg0 = 本段毫秒、w0 = 该刻水位（0 上满 / 1 沉底） */
let hgSeg0 = 0;
let hgW0 = 1;
/** 上一拍实际写入的水位（阈值变更时作为新锚点 w0 ⇒ 水位连续不跳变） */
let hgWLast = 1;
let hgInited = false;
let hgLastRunning = false;
let hgLastThreshold = props.thresholdMin;
let hgFlowTimer = 0;

/** 当前水位：沉底是常态；只有 running 才按锚点线性插值到阈值（seg = T ⇒ w = 1） */
function hgWater(segmentMs: number): number {
  const threshold = props.thresholdMin;
  // FIX006.3：阈值未就绪（settings 未取到 / 读取失败）⇒ 恒沉底，不造假水位
  if (state.value !== "running" || threshold == null) {
    return 1;
  }
  const total = threshold * 60_000;
  if (segmentMs >= total) {
    return 1;
  }
  if (segmentMs <= hgSeg0 || total <= hgSeg0) {
    return hgW0;
  }
  const w = hgW0 + ((segmentMs - hgSeg0) * (1 - hgW0)) / (total - hgSeg0);
  return Math.min(1, Math.max(0, w));
}

/** 翻半圈：先无过渡钉 -180° 一帧，再清样式过渡回 0°（design 1:1；"沉底 → 上满"的入场） */
function hgFlipRestart(): void {
  const model = hourglassEl.value?.querySelector<HTMLElement>(".loader__model");
  if (!model) {
    return;
  }
  model.style.transition = "none";
  model.style.transform = `${HG_MODEL_T} rotate(-180deg)`;
  void model.getBoundingClientRect();
  model.style.transition = "";
  model.style.transform = "";
}

/** 沙漏状态机（100ms tick 内调用）：水位与沙流全由"本段时长 + 会话态"决定 */
function tickHourglass(segmentMs: number): void {
  const hg = hourglassEl.value;
  if (!hg) {
    return;
  }
  const running = isRunning.value;
  const threshold = props.thresholdMin;
  // FIX006.3：阈值未就绪（settings 未取到 / 读取失败）⇒ 不写水位与动画轴（不造假值）
  if (threshold == null) {
    return;
  }

  if (!hgInited) {
    // 启动 / 崩溃恢复：running ⇒ 按本段落位（"无缝续"）；否则沉底（常态）
    hgInited = true;
    hgSeg0 = segmentMs;
    hgW0 = running ? Math.min(1, segmentMs / (threshold * 60_000)) : 1;
  } else if (running && !hgLastRunning) {
    // 新段（打卡 / 继续）：翻半圈回满、从零开始（用户定案：只有新段才重来）
    hgSeg0 = 0;
    hgW0 = 0;
    hgFlipRestart();
    window.clearTimeout(hgFlowTimer);
    hgFlowTimer = window.setTimeout(() => {
      hgFlowTimer = 0;
      hourglassEl.value?.classList.add("flowing");
    }, HG_FLIP * 1000);
  } else if (running && threshold !== hgLastThreshold) {
    // 阈值变更（用户定案）：**不翻**、水位连续，只重锚定流速 ⇒ 终点仍落在 seg = T 那刻
    hgSeg0 = segmentMs;
    hgW0 = hgWLast;
  } else if (!running) {
    // 沉底（常态）：休息 / 未在岗 / 下班 —— 由 CSS 过渡平滑走完
    hgSeg0 = segmentMs;
    hgW0 = 1;
  }

  const w = hgWater(segmentMs);
  hg.style.setProperty("--hg-w", String(w));
  // 沙粒下移周期随阈值（只在阈值变更时写入，避免每拍重算动画时间轴）
  if (threshold !== hgLastThreshold) {
    hg.style.setProperty("--hg-dur", `${threshold * 60}s`);
  }
  // 沙流：工作中且未漏完才落沙；新段等翻转完（定时器）再落
  if (running && w < 1) {
    if (hgFlowTimer === 0 && !hg.classList.contains("flowing")) {
      hg.classList.add("flowing");
    }
  } else {
    window.clearTimeout(hgFlowTimer);
    hgFlowTimer = 0;
    hg.classList.remove("flowing");
  }
  hgWLast = w;
  hgLastRunning = running;
  hgLastThreshold = threshold;
}

/* ============================================================
   ⑤ 电池（PATCH006.4 定案）：**今日 12h 总量**进度——电量 = 1 − 今日在岗累计 ÷ 12h；
   黄 = max(30%, 1 − 基准/12)（"进入加班区就亮"，但不早于 30%）、红 = 25%（固定第 9h）；
   下班冻结。取代 PATCH005.3 的"分母 = 本班预算"（点加班不再回血、开新班不再回满）
   ============================================================ */
const batPct = ref(100);
const batMid = ref(false);
const batLow = ref(false);
let batFrozen: number | null = null;

function clamp01(n: number): number {
  return Math.min(1, Math.max(0, n));
}

/** PATCH006.4：取数时刻戳——电量分子 = 后端快照 `used_secs` + 自取数以来的墙钟增量（保住"每秒在走"） */
let dayStampMs = Date.now();
watch(
  () => props.day,
  () => {
    dayStampMs = Date.now();
  },
);

/** 今日在岗累计（毫秒；含班内休息、班间空档不计——与后端口径一致） */
function dayUsedMs(): number {
  return (props.day?.used_secs ?? 0) * 1000 + Math.max(0, Date.now() - dayStampMs);
}

function batteryRatio(): number {
  if (!onDuty.value && batFrozen != null) {
    return batFrozen;
  }
  if (!props.day?.duty_started_at) {
    return 1;
  }
  return clamp01(1 - dayUsedMs() / (BATTERY_TOTAL_HOURS * 3_600_000));
}

/** 黄阈值：进入加班区（今日累计 > 基准）即亮，但**不早于 30%**（基准 ≥9h 时固定 30%）。
 * FIX006.3：基准未就绪（settings 未取到 / 读取失败）⇒ 返回 null，**不判加班区**（不造假阈值）。 */
function yellowThreshold(): number | null {
  const basis = props.autoOutHours;
  if (basis == null) {
    return null;
  }
  return Math.max(BATTERY_YELLOW_FLOOR, 1 - basis / BATTERY_TOTAL_HOURS);
}

function tickBattery(): void {
  const ratio = batteryRatio();
  batPct.value = Math.round(ratio * 100);
  batLow.value = ratio <= BATTERY_RED_RATIO;
  const yellow = yellowThreshold();
  batMid.value = !batLow.value && yellow != null && ratio <= yellow;
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
   ⑥ 三段式控制（PATCH002 语义）：工作 = Running、休息 = Paused、重置 = 收段归零回休息；
   打卡即工作（后端 clock_in 内 start），故在岗初始高亮即「工作」。
   动作只发**意图**（session_set_run_mode），start/resume/pause 的裁决全在 Rust 侧——
   前端不再按自身缓存的会话态自选命令（那是"三段切不动"BUG 的根因之一）。
   ============================================================ */
const runMode = computed(() => (!onDuty.value ? "" : isRunning.value ? "work" : "rest"));

/** 工作：交给后端裁决（Idle→start / Paused→resume / Running→幂等） */
function onWork(): void {
  void act("session_set_run_mode", { mode: "work" });
}

/** 休息：Running→pause（Paused/Idle 幂等） */
function onRest(): void {
  void act("session_set_run_mode", { mode: "rest" });
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
    <!-- 文案条浮层插槽（PATCH005.11）：以本行（日期卡 + 表盘）为定位锚，双向居中覆盖——
         插槽内容由 App 提供；本行定位锚（position: relative）样式见 App.vue 胶水块 -->
    <slot name="row-overlay" />
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

  <!-- ④ 打卡 pill（上班/下班；确认框由父组件持有）。PATCH005.3 到顶置灰：今日累计满 12h 后
       上班钮点不开（与后端 DayBudgetExhausted 同一判据） -->
  <button
    class="pill-cast"
    type="button"
    :class="{ off: !onDuty && capReached }"
    :disabled="!onDuty && capReached"
    @click="emit('clock', onDuty ? 'out' : 'in')"
  >
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

  <!-- ⑤ 三段式控制：工作/休息互斥选中；重置 = 瞬时（按下回弹，不保持选中）。
       动作挂 label 的 @click（PATCH002）——不再依赖 display:none radio 的 change 转发；
       input 只做显示（:checked 驱动 :has(input:checked) 的按下样式） -->
  <div class="seg-control" :class="{ off: !onDuty }">
    <label class="seg" @click="onWork">
      <input
        type="radio"
        name="runseg"
        value="work"
        :disabled="!onDuty"
        :checked="runMode === 'work'"
      />
      <span class="text">工作</span>
    </label>
    <label class="seg" @click="onRest">
      <input
        type="radio"
        name="runseg"
        value="rest"
        :disabled="!onDuty"
        :checked="runMode === 'rest'"
      />
      <span class="text">休息</span>
    </label>
    <button class="seg seg-reset" type="button" :disabled="!onDuty" @click="act('session_restart')">
      <span class="text">重置</span>
    </button>
  </div>
</template>
