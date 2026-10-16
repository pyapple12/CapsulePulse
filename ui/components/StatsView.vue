<script setup lang="ts">
/** 统计板（PL018 design 换装）：日期导航 + 时间图谱（分段/RWD 节点/彗星发射流）+ 三值卡
 * + 周卡（堆叠柱/均线/选中环闪现）+ 翻面今日明细。纯展示组件：数据来自 day_detail/week_detail
 * （Rust 侧归约），渲染算法 1:1 自 design index.html 内联脚本翻译（数据源换真实命令）。 */
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";

import type { DaySummary, WeekDay, WorkdayTotal } from "../types";
import { hhmm, pad, splitDuration } from "../format";
import { daySegments, type DaySegment } from "../src/day-segments";
import { useGlassBar } from "../src/use-glass-bar";
const props = defineProps<{
  refreshKey: number;
  /** 当前数据日偏移（PATCH005.4，来自 App 的 `session_status`）：统计的锚点与默认页 */
  dataDayOffset: number;
  open: boolean;
  /** 有浮层盖住统计板（确认框遮罩等）：玻璃浮钮随之淡出（PL026.3） */
  veiled?: boolean;
}>();

// 默认页 = 当前数据日（PATCH005.4：跨零点夜班没下班就停在前一天）；锚点变化由下面的 watch 跟随
const offset = ref(props.dataDayOffset);
const day = ref<DaySummary | null>(null);
// 周视图（PL022）：查看日所在自然周（周一~周日，恒 7 行），Rust 侧归约，本组件零聚合
const week = ref<WeekDay[]>([]);
// 总日均（PL022）：全历史工作时长总和 ÷ 有数据天数（后端聚合，前端只做除法展示）
const total = ref<WorkdayTotal>({
  duty_secs: 0,
  work_secs: 0,
  rest_secs: 0,
  days: 0,
  first_offset: null,
});
/** 拉取失败标记（PATCH010.5 用户定案）：统计板**不出现任何报错**（失败只进 console）；
 * 本标记仅用于**抑制"本日无打卡记录"空态**——拉失败时不能谎报"无记录"。 */
const loadFailed = ref(false);

async function refresh(): Promise<void> {
  try {
    day.value = await invoke<DaySummary>("day_detail", { offset: offset.value });
    loadFailed.value = false;
  } catch (err) {
    loadFailed.value = true;
    console.error("day_detail 调用失败", err);
  }
  try {
    total.value = await invoke<WorkdayTotal>("workday_total");
  } catch (err) {
    loadFailed.value = true;
    console.error("workday_total 调用失败", err);
  }
  try {
    week.value = await invoke<WeekDay[]>("week_detail", { offset: offset.value });
  } catch (err) {
    loadFailed.value = true;
    console.error("week_detail 调用失败", err);
  }
}

// 开板即刷（design：开板时强制重绘）；关板复位到统计面（design setStats(false) 移除 flipped）
watch(
  () => props.open,
  (open) => {
    if (open) {
      void refresh();
    } else {
      flipped.value = false;
    }
  },
);

/* ============================================================
   日期导航与标签（design day-label 语义：M月D日 · 周X · 在岗中/已下班）
   ============================================================ */
const WEEKDAY_CN = ["周一", "周二", "周三", "周四", "周五", "周六", "周日"];

/** 日期卡状态文案（PL025.4 四态 + PL026.2 第五态）：过去无打卡 = 休假日；今天无打卡 = 未上班；
 * 有打卡：今天未下班 = 在岗中、今天已下班 = 暂下班（多段工作可再打卡，故当天只算"暂时"下班）、
 * 过去日 = 已下班 */
function dayState(): string {
  const duty = day.value?.duty_started_at ?? null;
  const ended = day.value?.duty_ended_at ?? null;
  // PATCH005.4：分界与"在岗中"判据都相对**当前数据日**（不再相对日历今日）
  if (duty == null) {
    return offset.value < props.dataDayOffset ? "休假日" : "未上班";
  }
  if (offset.value === props.dataDayOffset) {
    return ended == null ? "在岗中" : "暂下班";
  }
  return "已下班";
}

const dayLabelText = computed(() => {
  const d = new Date(Date.now() + offset.value * 86_400_000);
  return `${d.getMonth() + 1}月${d.getDate()}日 · ${WEEKDAY_CN[(d.getDay() + 6) % 7]} · ${dayState()}`;
});

/** 往前翻的下界（PL025.1）：最早记录日偏移；空库/未加载 = 0（今天）→ 不许往前翻 */
const firstOffset = computed(() => total.value.first_offset ?? 0);

/** 前一日：最早记录日即最后一条（箭头 disabled 之外再兜一层，防程序化改 offset） */
function prevDay(): void {
  if (offset.value <= firstOffset.value) {
    return;
  }
  offset.value -= 1;
}

/* ============================================================
   时间图谱（design renderGraph 1:1）：bars/nodes 属性更新 + comet 白纹发射
   ============================================================ */
const graphSvg = ref<SVGSVGElement | null>(null);
const axisStart = ref("");
const axisEnd = ref("");

/** 段序列类型（FIX006.4：并入共享构建器 `daySegments` 的 `DaySegment`，原本地结构重复已消除） */
type GraphSeg = DaySegment;

/** graphSvg 内的子节点引用（bars 复用更新 / comet 白纹容器） */
const barsEl = ref<SVGGElement | null>(null);
const cometEl = ref<SVGGElement | null>(null);
/** PATCH006.3：duty（蓝）填色层——排在彗星之下（白纹只落在蓝段上） */
const dutyEl = ref<SVGGElement | null>(null);
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

/** 图谱空态（PL024.5，用户目测定案）：无打卡日 / 今日未上班——
 * 清段条与节点、停并清彗星、轴标置 na:na；避免残留上一日画面造成误读。 */
function renderGraphEmpty(): void {
  if (barsEl.value) {
    barsEl.value.innerHTML = "";
  }
  const nodesG = graphSvg.value?.querySelector<SVGGElement>("#graphNodes");
  if (nodesG) {
    nodesG.innerHTML = "";
  }
  cometWalk = 0;
  const comet = cometEl.value;
  if (comet) {
    comet.style.display = "none";
    for (const s of [...comet.children]) {
      s.remove();
    }
  }
  axisStart.value = "上班 na:na";
  axisEnd.value = "下班 na:na";
}

/** 图谱整帧渲染（design renderGraph 1:1；数据 = 真实 day.blocks，秒 → ms） */
function renderGraph(): void {
  const svg = graphSvg.value;
  const bars = barsEl.value;
  const d = day.value;
  if (!svg || !bars) {
    return;
  }
  if (!d?.duty_started_at || d.blocks.length === 0) {
    renderGraphEmpty();
    return;
  }
  const clockIn = d.duty_started_at * 1000;
  // PATCH005.4：判据由"日历今日（offset >= 0）"改为"**是不是当前数据日**"——跨零点夜班的数据日
  // 是打卡那一天（offset −1），视图仍按"在岗"渲染（蓝条 / 在岗尾巴 / 轴末 / 彗星都跟着它）
  const isCurrentDataDay = offset.value === props.dataDayOffset;
  const onDutyToday = isCurrentDataDay && d.duty_ended_at == null;
  const nowMs = Date.now();
  // PATCH004.3：**单一坐标**（照抄 design/index.html）——画布本身就是时间轴：0 = 上班、W = 轴末；
  // 底槽（透明条）与两个裁剪层保持满画布 [0, W]（rx = 6），两端圆角由它们裁出。填色、在岗尾巴、
  // 节点、彗星走廊**全走同一条映射**，颜色分界与节点圆心因此严丝合缝；端头节点的圆心由下面的
  // "夹进 [6, W−6]"处理（否则时刻落在画布边缘的节点会被裁成半颗）。

  // 段源（FIX006.4）：共享 `daySegments` 补班间空档（原与 TimerCard 各写一份，已收敛）；
  // 在岗时末段延伸到"墙钟现在"（活尾巴）——异态分界由下次刷新的块数据承担
  const segs: GraphSeg[] = daySegments(
    d.blocks,
    onDutyToday ? Math.floor(nowMs / 1000) * 1000 : undefined,
  );

  // PL025.10 轴末口径 (a)：今日在岗 → 轴末 = **当前在岗那一班**的打卡 + 自动下班小时
  // （当前班起点 = 最后一个 gap 之后的第一块起点；无 gap 则首块起点）——修掉"再打卡后第二班
  // 超出轴末被裁断"（旧口径恒按今天第一班打卡算）；今日已下班 / 过去日 = 末块末尾（落定重画）
  // FIX006.4：当前班起点改读后端 `shift_start`（与 TimerCard 底轨同一事实源；原前端自推一份）
  const currentShiftStart = (d.shift_start ?? 0) * 1000;
  // PATCH005.3：在岗时轴末 = 本班打卡 + **今日剩余**（后端单一来源，前端零估算）；暂下班 = 末段末尾。
  // 轴末保底到 now：预算已耗尽（到点兜底尚未执行）时不把真实块弧裁掉
  const endTs = onDutyToday
    ? Math.max(currentShiftStart + d.remaining_secs * 1000, nowMs)
    : segs[segs.length - 1]!.t1;
  const nowTs = isCurrentDataDay ? nowMs : endTs;
  const W = svg.clientWidth;
  if (W > 0 && svg.viewBox.baseVal.width !== W) {
    svg.setAttribute("viewBox", `0 0 ${W} 12`);
  }
  // PATCH003.5：底槽与两个裁剪层保持模板的满画布（width="100%" + rx="6"）——首帧/空态不再跳宽。
  // PATCH004.3：满画布口径（复原设计稿）——PATCH003 曾把填色/在岗尾巴/走廊/白纹行程连带收窄
  // 6px，端头因此留下一小段"素帽"；现全部回到 0…W，两端圆角由裁剪层 rx=6 裁出。
  // PATCH005.3：轴宽保底 1ms——"在岗但本班预算已耗尽"（到点兜底尚未执行）时轴末 = 本班打卡，
  // 零宽会让 frac 除零产出 NaN（SVG translate(NaN) 刷错误）；保底不改变正常态取值。
  const span = Math.max(1, endTs - clockIn);
  const frac = (ts: number): number => (ts - clockIn) / span;
  /** 时间 → 画布 x（满画布 0…W；圆角由裁剪层 rx=6 负责，这里不内缩）。 */
  const xpx = (ts: number): number => frac(ts) * W;

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
    if (i === 0 || st !== segs[i - 1]!.kind) {
      // PL025.10 节点语义 = 进入状态那一刻：进入工作 W / 进入休息 R / 进灰（= 上一班末）D；
      // 首段起点也打点（旧 `i > 0` 抑制是设计单班时代遗留，多班后表现为"只有第一班缺 W"）
      const ch = st === "gap" ? "D" : st === "rest" ? "R" : "W";
      nodes.push({ ts: t0, ch, kind: st === "gap" ? "end" : st });
    }
  });
  if (nowTs < endTs) {
    // 在岗尾巴：画到画布右缘（W = 轴末，与填色同一条映射；圆角由裁剪层裁出）
    list.push({ st: "duty", x: xpx(nowTs), w: W - xpx(nowTs), active: false });
  }
  nodes.push({ ts: endTs, ch: "D", kind: "end" });
  // PATCH012.7（用户定案）：**在岗状态下**，末尾这个 `D` 是"**预计下班**"占位（`endTs` = 本班预算到点，
  // 未来时刻 ⇒ 它是全图 ts 最大者），并非真实最新节点 ⇒ 让它的 z 序**让位**：先画（落最底），
  // 使数组最后一个 = 原倒数第二 = **最新的非 D 节点** ⇒ 它最后 append ⇒ 永远置顶。
  // 未在岗时不动：那时末尾 `D` = 真实下班时刻，本就该在最上（与"纯时间顺序"一致）。
  if (onDutyToday) {
    const di = nodes.findIndex((n) => n.ch === "D" && n.ts === endTs);
    if (di >= 0) {
      nodes.unshift(nodes.splice(di, 1)[0]!);
    }
  }

  // PATCH006.3：按 kind 分两组渲染（层序见模板）——duty（蓝）在彗星之下、work/rest/gap 在彗星之上；
  // 组内沿用 design 的"数量对齐 + 改属性"复用策略（抽 syncGroup 两次调用，DRY）
  const syncGroup = (g: SVGGElement | null, items: Bar[]): void => {
    if (!g) {
      return;
    }
    while (g.children.length < items.length) {
      g.append(document.createElementNS(SVG_NS, "rect"));
    }
    while (g.children.length > items.length) {
      g.lastElementChild?.remove();
    }
    [...g.children].forEach((rect, i) => {
      const b = items[i]!;
      rect.setAttribute("class", `graph-seg ${b.st}${b.active ? " active" : ""}`);
      rect.setAttribute("x", b.x.toFixed(2));
      rect.setAttribute("width", Math.max(0, b.w).toFixed(2));
      rect.setAttribute("y", "0");
      rect.setAttribute("height", "12");
    });
  };
  syncGroup(
    dutyEl.value,
    list.filter((b) => b.st === "duty"),
  );
  syncGroup(
    bars,
    list.filter((b) => b.st !== "duty"),
  );

  // nodes：数量变化整层重建（design：避免复用跨色渐变伪影）
  const nodesG = svg.querySelector<SVGGElement>("#graphNodes");
  if (nodesG) {
    nodesG.innerHTML = "";
    nodes.forEach((n) => {
      // PATCH003.4 定案 a：节点圆心 = 它的真实时刻（= 颜色分界线），覆盖顺序因此 = 纯时间顺序
      // （DOM 追加序 = 时间序；SVG 同层后画在上）。
      // PATCH004.3：圆心横向**夹进 [6, W−6]**（圆角帽心区间）——内部节点不受影响（圆心恰在时刻上）；
      // 轴两端（时刻落在画布边缘 0 / W）被夹进 6px，整颗圆（r 5.4 < 圆角 6）落在圆角帽内不被裁掉。
      const center = Math.min(Math.max(xpx(n.ts), 6), Math.max(6, W - 6));
      const g = document.createElementNS(SVG_NS, "g");
      g.setAttribute("class", `graph-node ${n.kind}${n.ts <= nowTs ? " lit" : ""}`);
      g.setAttribute("transform", `translate(${(center - 6).toFixed(2)} 0)`);
      g.innerHTML =
        '<circle cx="6" cy="6" r="5.4" /><text x="6" y="6" dy="-0.5" text-anchor="middle" dominant-baseline="central"></text>';
      (g.querySelector("text") as SVGTextElement).textContent = n.ch;
      nodesG.append(g);
    });
  }

  // 轴标（FIX006.18：复用 `format.hhmm`——**注意单位换算**：本处毫秒、format 收秒）
  axisStart.value = `上班 ${hhmm(clockIn / 1000)}`;
  axisEnd.value = `下班 ${hhmm(endTs / 1000)}`;

  // 彗星走廊（design 1:1）：今天 = birthX 起走廊；过去日 walk=0 静默
  const comet = cometEl.value;
  const clipRect = cometClipRect.value;
  if (!comet || !clipRect) {
    return;
  }
  const birthX = xpx(segs[segs.length - 1]!.t0);
  // PATCH004.3：行程与走廊同源——走廊右端 = 画布右缘 W（设计稿原口径；斜纹由此被裁剪层
  // rx=6 裁成半圆，端头圆角里也能看到斜纹），故 walk = 走廊宽 + 24（24 = 斜纹自身宽度：
  // 走完即完全退出走廊，回收时不留硬切）
  // PATCH006.2：白纹（斜条）= "在岗中"指示 ⇒ 判据用 onDutyToday（原 isCurrentDataDay 在
  // "已下班但仍停在当前数据日"时照旧发射，出现"暂下班后斜纹仍满条飞"）
  cometWalk = onDutyToday ? W - birthX + 24 : 0;
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
  if (onDutyToday && cometWalk > 24) {
    const wasHidden = comet.style.display === "none";
    comet.style.display = "";
    comet.setAttribute("transform", `translate(${birthX.toFixed(2)} 0)`);
    // 彗星走廊右端 = 画布右缘 W（与 cometWalk 同源；PATCH004.3 起不再止于帽心——设计稿
    // 原本就是 W，右端那半圆由裁剪层 rx=6 自己裁出来）
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

/** 生成一颗彗星条纹（design 1:1）：`born` = 出生时刻、`age` = 已存活毫秒 ⇒ 由两者算出动画
 * 起点（负 delay 对齐相位）与 `animationend` 自摘；存活期外由调用方不再生成。 */
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
const flipped = ref(false);
let selTimer = 0;
let destroyed = false;
let lastViewDay = 0;
let weekSig = "";

/** 周卡七格单字标签（行序 = 周一~周日，PL022） */
const WEEKDAY_SINGLE = ["一", "二", "三", "四", "五", "六", "日"];

/** 当前**数据日**的 MM-DD（"今"标记判定 = 周卡行 date 与之一致；PATCH005.4：跨零点后
 * 在岗日不变 ⇒ "今"仍落在前一天，与"在岗中"一致） */
const todayMmdd = computed(() => {
  const d = new Date(Date.now() + props.dataDayOffset * 86_400_000);
  return `${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
});

/** 总日均（分钟）：全历史工作总和 ÷ 有数据天数（PL022；无数据为零） */
const avgWorkMin = computed(() =>
  total.value.days > 0 ? total.value.work_secs / 60 / total.value.days : 0,
);

function roundedTopRect(x: number, y: number, w: number, h: number, r: number): string {
  const rr = Math.min(r, h / 2, w / 2);
  return `M${x},${y + h} L${x},${y + rr} Q${x},${y} ${x + rr},${y} L${x + w - rr},${y} Q${x + w},${y} ${x + w},${y + rr} L${x + w},${y + h} Z`;
}

function fmtCN(min: number): string {
  // FIX006.1：进位口径统一走 splitDuration（原自算 `round(min) % 60` 会丢整点进位）
  const { h, m } = splitDuration(min);
  return `${h}小时${m}分钟`;
}

/** 分钟 → "XhYYm"（design fmtHM 1:1：三值卡数字制式，分钟两位补零；周卡日均仍用 fmtCN） */
function fmtHM(min: number): string {
  const { h, m } = splitDuration(min);
  return `${h}h${pad(m)}m`;
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
  // PL026.1 尺度下限：分母取"当周峰值"与"总日均"的较大者——当周偏弱时柱整体变矮（相对总日均
  // 的真实表达），均线也不再被夹到 100%（此前 min(100,…) 会让它顶格当"最高点"，看不出高出多少）
  const weekPeak = Math.max(...days.map((d) => d.work + d.rest), 1);
  const avgWork = avgWorkMin.value;
  const scaleMax = Math.max(weekPeak, avgWork, 1);

  const W = svg.clientWidth;
  const H = svg.clientHeight;
  if (W > 0 && H > 0 && (svg.viewBox.baseVal.width !== W || svg.viewBox.baseVal.height !== H)) {
    svg.setAttribute("viewBox", `0 0 ${W} ${H}`);
  }
  const slot = W / 7;
  const plotH = H - 3; // 上下各留 1.5px 描边余量

  let inner = "";
  days.forEach((d, i) => {
    const barTotal = d.work + d.rest;
    const barH = barTotal ? (barTotal / scaleMax) * plotH : 0;
    const restH = barTotal ? (d.rest / barTotal) * barH : 0;
    const workH = barTotal ? (d.work / barTotal) * barH : 0;
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
  // 内容签名：查看周 + 逐柱总高变化才重建（避免每秒整层重建）
  const sig = `${offset.value}|${days.map((d) => Math.round(d.work + d.rest)).join(",")}`;
  if (sig !== weekSig) {
    weekSig = sig;
    bars.innerHTML = inner;
  }

  // 均线（design 1:1；高度 = 总日均占尺度分母的比例——PL026.1 起分母 = max(当周峰值, 总日均)，
  // 故不再需要越顶夹取；当周偏弱时均线自然成为最高点且高度真实可读）
  if (wcAvgLine.value) {
    wcAvgLine.value.style.bottom = `${((avgWork / scaleMax) * 100).toFixed(1)}%`;
  }

  // 选中环闪现：查看日偏移变化时触发（design flashSel 1:1）——
  // PL022：查看日所在格由真实日期定位（自然周下不再是"末列 + offset"）
  if (offset.value !== lastViewDay) {
    const viewed = new Date(Date.now() + offset.value * 86_400_000);
    const key = `${String(viewed.getMonth() + 1).padStart(2, "0")}-${String(viewed.getDate()).padStart(2, "0")}`;
    const i = week.value.findIndex((d) => d.date === key);
    if (i < 0) {
      return; // 周数据未跟上（跨周刷新途中）：不记 lastViewDay，待新数据到位再闪现
    }
    lastViewDay = offset.value;
    const barTotal = days[i]!.work + days[i]!.rest;
    const barH = barTotal ? (barTotal / scaleMax) * plotH : 0;
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

/** 分钟轴 → `HH:MM`（段明细时间列：分钟是**相对班起点**的偏移，故先加到班起点再格式化）。 */
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
    // FIX006.2：判据改"是不是当前数据日"——原 `offset === 0` 是"日历今天"，跨零点夜班的数据日
    // 为 −1 时该页判据为假 ⇒ 在岗那一页永远不显示「至今」（同文件图谱与节拍早已用 dataDayOffset）
    const isDataDay = offset.value === props.dataDayOffset;
    const ongoing = isDataDay && d.duty_ended_at == null && i === d.blocks.length - 1;
    const endSec = isDataDay ? Math.min(b.end, nowSec) : b.end;
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
  // FIX006.2：数据日即"今日"（与周卡"今"标、在岗判据同口径）
  if (offset.value === props.dataDayOffset) {
    return "今日明细";
  }
  const d = new Date(Date.now() + offset.value * 86_400_000);
  return `${d.getMonth() + 1}月${d.getDate()}日明细`;
}

/** 秒 → 明细行的时长文本（`XhYYm` / `XXm` 两档；与三值卡的 fmtHM 同制式语义但更短）。 */
function durText(secs: number): string {
  return `${Math.round(secs / 60)}分`;
}

/** 段条宽度百分比：以**最长已落定段**为分母（进行中段不计入分母，免整行被压扁）。 */
function blkWidth(secs: number): string {
  return `${Math.min((secs / detailMax.value) * 100, 100).toFixed(1)}%`;
}

/** 三值卡进度条宽度（design tc-percent：占在岗份额；PL024.6 起仅进度条沿用，文字改用 deltaView） */
function pct(part: number): number {
  const duty = day.value?.duty_secs ?? 0;
  return duty > 0 ? Math.round((part / duty) * 100) : 0;
}

/** 三值各自的全历史平均（秒；分母 = 有数据天数，与总日均同口径） */
const avgDuty = computed(() =>
  total.value.days > 0 ? total.value.duty_secs / total.value.days : 0,
);
const avgWork = computed(() =>
  total.value.days > 0 ? total.value.work_secs / total.value.days : 0,
);
/** 休息的历史平均（秒；与 `avgWork` 同口径——分母 = 有数据天数） */
const avgRest = computed(() =>
  total.value.days > 0 ? total.value.rest_secs / total.value.days : 0,
);

/** 指示符三态（PL024.6 用户定案）：up 红 / down 绿 / eq 中性灰 / na 无数据 */
type DeltaState = "up" | "down" | "eq" | "na";

/** 单值相对其历史平均的偏离（整数百分比，不带正负号；方向由箭头与颜色表达） */
function deltaView(valueSecs: number, avgSecs: number): { text: string; state: DeltaState } {
  if (day.value?.duty_started_at == null || avgSecs <= 0) {
    return { text: "na", state: "na" };
  }
  const pctVal = Math.round(((valueSecs - avgSecs) / avgSecs) * 100);
  if (pctVal > 0) {
    return { text: `${pctVal}%`, state: "up" };
  }
  if (pctVal < 0) {
    return { text: `${Math.abs(pctVal)}%`, state: "down" };
  }
  return { text: "0%", state: "eq" };
}

/** 指示符图形（上 / 下 / 等号；na 无图形） */
function deltaIcon(state: DeltaState): string {
  if (state === "up") {
    return "m18 15-6-6-6 6";
  }
  if (state === "down") {
    return "m6 9 6 6 6-6";
  }
  return "M6 12h12";
}

const dutyDelta = computed(() => deltaView(day.value?.duty_secs ?? 0, avgDuty.value));
const workDelta = computed(() => deltaView(day.value?.work_secs ?? 0, avgWork.value));
const restDelta = computed(() => deltaView(day.value?.rest_secs ?? 0, avgRest.value));

/** 明细进行中段的时长增长由每秒 renderDetail 重算覆盖（design sig 语义同效） */

/* ============================================================
   PL026.3 玻璃滚动浮钮（照抄 CT）：原生滑杆隐藏，缝里浮一枚 4×16 透明玻璃钮
   ============================================================ */
const frontFaceEl = ref<HTMLElement | null>(null);
const backFaceEl = ref<HTMLElement | null>(null);
let glassBar: {
  sync: () => void;
  setVeiled: (veiled: boolean) => void;
  destroy: () => void;
} | null = null;

onMounted(() => {
  cometClipRect.value = document.querySelector<SVGRectElement>("#graphClipCometRect");
  // PL026.3 玻璃滚动浮钮（照抄 CT）：锚在底板层（#win），落在统计板右缘与窗口右缘之间的缝里；
  // 两个翻转面都参与（隐藏原生滑杆 + 滚动同步），几何取当前活动面
  const faces = [frontFaceEl.value, backFaceEl.value].filter((el): el is HTMLElement => el != null);
  if (faces.length > 0) {
    glassBar = useGlassBar(faces, {
      right: 4.5,
      inset: true,
      active: () => (flipped.value ? backFaceEl.value : frontFaceEl.value),
    });
    glassBar.setVeiled(props.veiled === true);
  }
  void refresh();
});

// 翻面 / 开合不派发 scroll（几何变了但没滚动）→ 手动同步浮钮
watch([flipped, () => props.open], () => {
  void nextTick(() => glassBar?.sync());
});
watch(
  () => props.veiled,
  (veiled) => glassBar?.setVeiled(veiled === true),
);

watch(
  () => props.refreshKey,
  () => {
    void refresh();
  },
);
watch(offset, () => {
  void refresh();
});
// PATCH005.4：锚点跟随当前数据日——用户没翻走（仍停在旧锚点）时锚点变化即跟随（下班 / 自动下班
// 后偏移回 0，视图随之切回"今日未上班"）；用户手动翻页时不动（尊重其浏览）
watch(
  () => props.dataDayOffset,
  (next, prev) => {
    if (offset.value === prev) {
      offset.value = next;
    }
  },
);
// 渲染节拍：数据到位后重建 + 每秒重算（design 1s 节流同效）
watch([day, week], () => {
  renderGraph();
  renderWeek();
  renderDetail();
});
// FIX006.19：**板关时本拍零工作**（原实现不论板开关都每秒重建图谱节点层 / 周卡 / 明细 ⇒ 常态空转；
// 关板复位到统计面后这些 DOM 本来也不可见）。板开后的首帧由上面的 watch 立即补齐 ✓。
const secTimer = window.setInterval(() => {
  if (!props.open) {
    return;
  }
  renderGraph();
  renderWeek();
  renderDetail();
}, 1000);

// PATCH006.1：在岗节拍——"板开 && 在岗中"时每 5 秒**只重取当日明细**（橙红块与三值随时间增长）；
// 关板 / 未上班 / 已下班一律停表（两个门都在 ⇒ 不空转）；周卡与总日均仍只在动作路径 `refresh()` 取。
// 背景：纯时间推移原本不触发重取，而"在岗尾巴"右端在后端取数时已被 `min(now)` 钉死 ⇒ 块不会长。
let dutyTick: number | undefined;
function stopDutyTick(): void {
  if (dutyTick !== undefined) {
    window.clearInterval(dutyTick);
    dutyTick = undefined;
  }
}
/** 轻量重取：只刷当日明细（不含周卡 / 总日均） */
async function refreshDayOnly(): Promise<void> {
  try {
    day.value = await invoke<DaySummary>("day_detail", { offset: offset.value });
    loadFailed.value = false;
  } catch (err) {
    loadFailed.value = true;
    console.error("day_detail 调用失败", err);
  }
}
/** 按"板开 && 在岗中"启停节拍 */
function syncDutyTick(): void {
  const d = day.value;
  const onDuty =
    d?.duty_started_at != null && d.duty_ended_at == null && offset.value === props.dataDayOffset;
  if (props.open && onDuty) {
    if (dutyTick === undefined) {
      dutyTick = window.setInterval(() => void refreshDayOnly(), 5_000);
    }
  } else {
    stopDutyTick();
  }
}
watch([() => props.open, day, offset, () => props.dataDayOffset], () => {
  syncDutyTick();
});
onUnmounted(stopDutyTick);

onUnmounted(() => {
  destroyed = true;
  glassBar?.destroy();
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
        <section ref="frontFaceEl" class="stats sheet flip-face front">
          <div class="daynav">
            <button
              class="arrow"
              type="button"
              title="前一日"
              :disabled="offset <= firstOffset"
              @click="prevDay"
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

          <div class="graph-block">
            <div class="graph">
              <svg id="graphSvg" ref="graphSvg" height="12" width="100%" preserveAspectRatio="none">
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
                <!-- PATCH006.3：填色分两层——duty（蓝：预计/在岗带）排在彗星**之下**（白纹只落在蓝段上），
                     work/rest/gap 排在彗星**之上**（随时间右长即自然盖掉白纹） -->
                <g id="graphDuty" ref="dutyEl" clip-path="url(#graphClip)"></g>
                <!-- 裁剪挂不动的外壳（clipPath 随引用元素坐标系走，挂平移组会被走廊带走） -->
                <g clip-path="url(#graphClipComet)">
                  <g ref="cometEl"></g>
                </g>
                <g id="graphBars" ref="barsEl" clip-path="url(#graphClip)"></g>
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
              <div class="tc-head" :class="{ tight: dutyDelta.text.length >= 4 }">
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
                <span class="tc-percent" :class="dutyDelta.state"
                  ><svg
                    v-if="dutyDelta.state !== 'na'"
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    stroke-width="3"
                    stroke-linecap="round"
                    stroke-linejoin="round"
                  >
                    <path :d="deltaIcon(dutyDelta.state)" />
                  </svg>
                  {{ dutyDelta.text }}</span
                >
              </div>
              <b>{{ fmtHM((day?.duty_secs ?? 0) / 60) }}</b>
              <span class="tc-hint">今日明细</span>
            </div>
            <div class="gcard t-card t-work">
              <div class="tc-head" :class="{ tight: workDelta.text.length >= 4 }">
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
                <span class="tc-percent" :class="workDelta.state"
                  ><svg
                    v-if="workDelta.state !== 'na'"
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    stroke-width="3"
                    stroke-linecap="round"
                    stroke-linejoin="round"
                  >
                    <path :d="deltaIcon(workDelta.state)" />
                  </svg>
                  {{ workDelta.text }}</span
                >
              </div>
              <b>{{ fmtHM((day?.work_secs ?? 0) / 60) }}</b>
              <div class="tc-range">
                <i class="fill" :style="{ width: `${pct(day?.work_secs ?? 0)}%` }"></i>
              </div>
            </div>
            <div class="gcard t-card t-rest">
              <div class="tc-head" :class="{ tight: restDelta.text.length >= 4 }">
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
                <span class="tc-percent" :class="restDelta.state"
                  ><svg
                    v-if="restDelta.state !== 'na'"
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    stroke-width="3"
                    stroke-linecap="round"
                    stroke-linejoin="round"
                  >
                    <path :d="deltaIcon(restDelta.state)" />
                  </svg>
                  {{ restDelta.text }}</span
                >
              </div>
              <b>{{ fmtHM((day?.rest_secs ?? 0) / 60) }}</b>
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
                <span v-for="(d, i) in week" :key="i" :class="{ today: d.date === todayMmdd }">{{
                  d.date === todayMmdd ? "今" : WEEKDAY_SINGLE[i]
                }}</span>
              </div>
            </div>
          </div>
        </section>

        <!-- 背面：今日/当日明细 -->
        <section ref="backFaceEl" class="detail-face sheet flip-face back">
          <h3>{{ detailTitle() }}</h3>
          <div class="detail">
            <div v-for="(r, i) in detailRows" :key="i" class="dl">
              <!-- PATCH012.5：进行中行的「至今」由行尾标签移入时间列（形如 23:24–至今，颜色不变）；
                   两字拆两个子元素 + 容器等宽 ⇒ 被 flex 撑到列首尾、右缘与时间串（如 23:38）对齐 -->
              <span class="time"
                >{{ fmtT(r.startMin) }}–<span v-if="r.ongoing" class="ongoing"
                  ><span>至</span><span>今</span></span
                ><template v-else>{{ fmtT(r.endMin) }}</template></span
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
            </div>
            <p v-if="detailRows.length === 0 && !loadFailed" class="empty">本日无打卡记录</p>
          </div>
          <button class="flip-back" type="button" @click="flipped = false">返回统计</button>
        </section>
      </div>
    </div>
  </div>
</template>
