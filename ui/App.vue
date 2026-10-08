<script setup lang="ts">
import { onMounted, onUnmounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";

import ConfirmModal from "./components/ConfirmModal.vue";
import SettingsPanel from "./components/SettingsPanel.vue";
import StatsView from "./components/StatsView.vue";
import TimerCard from "./components/TimerCard.vue";
// IPC DTO 镜像类型统一收敛在 types.ts（单一来源 = Rust serde 结构，防多处声明漂移）
import type { DaySummary, ReminderSettings } from "./types";
// 展示格式化共享助手（FIX002.13 收敛）
import { hhmm } from "./format";
// 标题粒子化视觉件（design initTitleParticles 1:1，reduced-motion 内部退避静态文字）
import { initTitleParticles } from "./src/title-particles";
// 提示音经 vite 打包（哈希进 dist）——不用 public/ 目录（publicDir 默认在根，曾有 404 教训）
import chimeUrl from "../assets/house_alarm-clock_loud.mp3";

// 统计聚合/提醒判定在 Rust；本组件只做展示与命令转发

// 统计为低频数据：挂载 + 动作后（TimerCard changed 事件）+ 30s 兜底，不进 100ms tick
const STATS_TICK_MS = 30_000;

// 当日明细（PL017 dial 弧段 + 电池上班时刻的数据源；30s 兜底 + 动作后即刷）
const day = ref<DaySummary | null>(null);
const settings = ref<ReminderSettings | null>(null);
const panelVisible = ref(false);
const reminderVisible = ref(false);
// 触发阈值取自 reminder-due 事件 payload（Rust 侧评估时的真实设置），不做前端默认值兜底
const reminderThreshold = ref(0);
// 设置保存失败的可见反馈（面板内展示，成功或重开面板时清除）
const saveError = ref("");
// 动作/打卡失败的可见反馈（FIX002.3：命令失败不再只进 console）；动作成功即清除
const actionError = ref("");
const chimeRef = ref<HTMLAudioElement | null>(null);
// PL005：打卡双向确认框（pill 在 TimerCard，方向经 clock 事件上抛，不直接执行）
const confirmMode = ref<"in" | "out" | null>(null);
// 两板互斥（design：统计板/设置板飞出，开一关一）
const statsOpen = ref(false);
const autoOutVisible = ref(false);
const autoOutAt = ref(0);
// 统计视图刷新信号：计时/打卡动作后自增，StatsView watch 重拉（保持 Rust 不推送定案）
const statsRefreshKey = ref(0);
let statsTimer: number | undefined;
// 自动下班条自隐句柄（FIX005.1：横幅不再滞留至重启；新触发先清旧防叠加）
let autoOutHideTimer: number | undefined;
let unlistenReminder: (() => void) | undefined;
let unlistenAutoOut: (() => void) | undefined;
// 标题粒子视觉件清理句柄（组件卸载时解绑 window 监听并停帧）
let destroyTitleParticles: (() => void) | undefined;

// —— PL010.1 拖拽修复：data-tauri-drag-region 只在"被点中元素自身"带属性时生效，
// 弹性布局铺满后 main 无裸区可点（回归 bug）——改为全局 mousedown 接线：
// 交互元素白名单命中不抢，其余一律启动窗口拖拽（点按语义不受影响）；
// 浮层件全部在列（PL019：板区不拖不收、确认遮罩不拖，design 无拖拽语义）
const DRAG_INTERACTIVE =
  "button, input, textarea, select, a, .sideButton, #stats-board, #settings-board, .overlay";

// —— design flyBoard（PL019.2）：把开板按钮中心换算成板内坐标写入 transform-origin，
// 开板即自该按钮处缩放飞出；BOARD_TOP/LEFT = boards.css 板位常量（design 同名）——
const BOARD_TOP = 54;
const BOARD_LEFT = 12;
const winEl = ref<HTMLElement | null>(null);
const statsBtnEl = ref<HTMLElement | null>(null);
const gearBtnEl = ref<HTMLElement | null>(null);
const statsViewEl = ref<InstanceType<typeof StatsView> | null>(null);
const settingsPanelEl = ref<InstanceType<typeof SettingsPanel> | null>(null);

/** 开板飞出原点：按钮中心 → 板内坐标，取整写 --origin-x/y（design flyBoard 1:1） */
function flyBoard(board: HTMLElement | null, btn: HTMLElement | null): void {
  const win = winEl.value;
  if (win == null || board == null || btn == null) {
    return;
  }
  const cr = win.getBoundingClientRect();
  const br = btn.getBoundingClientRect();
  board.style.setProperty(
    "--origin-x",
    `${Math.round(br.left + br.width / 2 - cr.left - BOARD_LEFT)}px`,
  );
  board.style.setProperty(
    "--origin-y",
    `${Math.round(br.top + br.height / 2 - cr.top - BOARD_TOP)}px`,
  );
}

/** 非交互区按下：板开着先收板（design 点板外收板，PL019.6；板区/开合钮在白名单不收），
 * 收板动作消费本次按下不抢拖拽；无板时启动窗口拖拽 */
function onWindowDown(e: MouseEvent): void {
  if (e.button !== 0) {
    return;
  }
  const target = e.target as HTMLElement | null;
  if (target?.closest(DRAG_INTERACTIVE)) {
    return;
  }
  if (statsOpen.value || panelVisible.value) {
    statsOpen.value = false;
    panelVisible.value = false;
    return;
  }
  void getCurrentWindow().startDragging();
}

/** 拉取当日明细（dial 弧段 + 电池数据源；统计的详细刷新在 StatsView 自持） */
async function refreshStats(): Promise<void> {
  try {
    day.value = await invoke<DaySummary>("day_detail", { offset: 0 });
  } catch (err) {
    console.error("day_detail 调用失败", err);
  }
}

/** 拉取提醒设置 */
async function refreshSettings(): Promise<void> {
  try {
    settings.value = await invoke<ReminderSettings>("get_settings");
  } catch (err) {
    console.error("get_settings 调用失败", err);
  }
}

/** 提示音（sound_enabled 门控；播放失败降级仅通知——容错白名单） */
function playChime(): void {
  if (settings.value && !settings.value.sound_enabled) {
    return;
  }
  chimeRef.value?.play().catch((err) => {
    console.warn("提示音播放失败（降级仅通知）", err);
  });
}

/** 保存设置（Rust 侧校验 + 持久化 + 即时生效）：设计模型 = 即时生效且板保持常开
 * （PL014 定案，点板外收板），成功仅清错误；失败错误态传入面板可见反馈 */
async function onSaveSettings(s: ReminderSettings): Promise<void> {
  try {
    await invoke("set_settings", { settings: s });
    settings.value = s;
    saveError.value = "";
  } catch (err) {
    // CommandError 经 IPC 序列化为文案字符串；面板内展示 + console 留痕
    saveError.value = `设置保存失败：${String(err)}`;
    console.error("set_settings 调用失败", err);
  }
}

/** ⚙ 开合设置板（与统计板互斥）：开板自齿轮中心飞出（flyBoard）；settings 未就绪时可见反馈不静默（FIX002.3） */
function togglePanel(): void {
  if (settings.value == null) {
    actionError.value = "设置加载失败，请重启应用重试";
    return;
  }
  panelVisible.value = !panelVisible.value;
  if (panelVisible.value) {
    statsOpen.value = false;
    saveError.value = "";
    const board = settingsPanelEl.value?.$el as HTMLElement | undefined;
    flyBoard(board ?? null, gearBtnEl.value);
  }
}

/** 统计板开合（与设置板互斥）：开板自统计钮中心飞出（flyBoard） */
function toggleStats(): void {
  statsOpen.value = !statsOpen.value;
  if (statsOpen.value) {
    panelVisible.value = false;
    const board = statsViewEl.value?.$el as HTMLElement | undefined;
    flyBoard(board ?? null, statsBtnEl.value);
  }
}

/** TimerCard 动作后：统计即刷；动作即处理提醒（暂停/重开 = 新段），文案条与错误条随之隐藏 */
function onTimerChanged(): void {
  void refreshStats();
  statsRefreshKey.value++;
  reminderVisible.value = false;
  actionError.value = "";
}

/** 打卡 pill（TimerCard 上抛方向）→ 弹对应方向确认框（双向确认，不直接执行） */
function onPillClick(direction: "in" | "out"): void {
  confirmMode.value = direction;
}

/** 确认框取消：仅收起 */
function onConfirmCancel(): void {
  confirmMode.value = null;
}

/** 收起自动下班条并清自隐句柄（FIX005.1：展示层生命周期与打卡流对称） */
function clearAutoOutBanner(): void {
  if (autoOutHideTimer !== undefined) {
    window.clearTimeout(autoOutHideTimer);
    autoOutHideTimer = undefined;
  }
  autoOutVisible.value = false;
}

/** 确认框确认：执行打卡 → 刷新在岗态与统计；失败经错误条可见（FIX002.3） */
async function onConfirmOk(): Promise<void> {
  const mode = confirmMode.value;
  confirmMode.value = null;
  if (mode == null) {
    return;
  }
  actionError.value = "";
  try {
    await invoke(mode === "in" ? "clock_in" : "clock_out");
  } catch (err) {
    actionError.value = `打卡失败：${String(err)}`;
    console.error("打卡命令调用失败", err);
  }
  // 下班成功即清提醒条（FIX002.2：提醒触发后直接下班不再滞留）
  if (mode === "out" && !actionError.value) {
    reminderVisible.value = false;
  }
  // 上班成功清双横幅（FIX005.1：新工作日开始，昨日提醒条/已自动下班条语境失效）
  if (mode === "in" && !actionError.value) {
    reminderVisible.value = false;
    clearAutoOutBanner();
  }
  void refreshStats();
  statsRefreshKey.value++;
}

onMounted(() => {
  void refreshStats();
  void refreshSettings();
  statsTimer = window.setInterval(() => void refreshStats(), STATS_TICK_MS);
  window.addEventListener("mousedown", onWindowDown);
  // 标题粒子视觉件（reduced-motion 或缺件时内部返回 undefined，静态文字兜底）
  if (winEl.value != null) {
    destroyTitleParticles = initTitleParticles(winEl.value);
  }
  // reminder-due：Rust 侧评估触发（payload = 触发时的真实阈值分钟数，直显文案条）；注册失败必须可见
  listen<number>("reminder-due", (event) => {
    reminderThreshold.value = event.payload;
    reminderVisible.value = true;
    playChime();
  })
    .then((un) => {
      unlistenReminder = un;
    })
    .catch((err) => {
      // 注册失败必须可见（FIX005.3：release 无控制台，ACL 静默拒是登记过的现实风险）
      actionError.value = `提醒监听注册失败：${String(err)}`;
      console.error("reminder-due 监听注册失败", err);
    });
  // workday-auto-out：payload = 回填下班时刻（上班 + N），文案条告知 + 界面即刷；
  // 30s 后自隐（FIX005.1：此前无任何清理点会滞留至重启），新触发重置计时
  listen<number>("workday-auto-out", (event) => {
    autoOutAt.value = event.payload;
    autoOutVisible.value = true;
    if (autoOutHideTimer !== undefined) {
      window.clearTimeout(autoOutHideTimer);
    }
    autoOutHideTimer = window.setTimeout(() => {
      autoOutVisible.value = false;
      autoOutHideTimer = undefined;
    }, 30_000);
    void refreshStats();
    statsRefreshKey.value++;
  })
    .then((un) => {
      unlistenAutoOut = un;
    })
    .catch((err) => console.error("workday-auto-out 监听注册失败", err));
});

onUnmounted(() => {
  if (statsTimer !== undefined) {
    window.clearInterval(statsTimer);
  }
  if (autoOutHideTimer !== undefined) {
    window.clearTimeout(autoOutHideTimer);
  }
  window.removeEventListener("mousedown", onWindowDown);
  unlistenReminder?.();
  unlistenAutoOut?.();
  destroyTitleParticles?.();
});
</script>

<template>
  <main ref="winEl" class="window" id="win">
    <!-- 位移折射滤镜（design 1:1）：真实窗口 backdrop 链不渲染（README 可行性 ❌ → 聚焦磨砂归 DWM），保留 DOM 对位 -->
    <svg width="0" height="0" style="position: absolute" aria-hidden="true">
      <filter id="rf-window" filterUnits="objectBoundingBox" x="0" y="0" width="1" height="1">
        <feImage x="0" y="0" width="1" height="1" preserveAspectRatio="none" result="m" />
        <feDisplacementMap
          in="SourceGraphic"
          in2="m"
          scale="12"
          xChannelSelector="R"
          yChannelSelector="G"
        />
      </filter>
    </svg>
    <div class="fx mottle"></div>
    <div class="fx grain"></div>
    <div class="fx sweep"></div>

    <div class="content">
      <header class="topbar">
        <h1 class="title">CapsulePulse<canvas class="title-canvas" aria-hidden="true"></canvas></h1>
      </header>

      <!-- 文案条（提醒 / 自动下班 / 动作错误）：配方 = controls.css .banner 1:1，error 红字为 ui 功能胶水 -->
      <div v-if="reminderVisible" class="banner remind">
        <span class="dot"></span><span>已连续工作 {{ reminderThreshold }} 分钟，休息一下吧</span>
      </div>
      <div v-if="autoOutVisible" class="banner auto">
        <span class="dot"></span><span>已于 {{ hhmm(autoOutAt) }} 自动下班</span>
      </div>
      <div v-if="actionError" class="banner error" role="alert">{{ actionError }}</div>

      <!-- 计时页（设计单页常驻；统计/设置 = 侧角钮飞出板，浮层件均在 .window 直下与 .content 平级） -->
      <section class="stage" id="page-timer">
        <TimerCard
          :day="day"
          :threshold-min="settings?.threshold_min ?? 50"
          :target-hours="settings?.workday_auto_out_hours ?? null"
          @changed="onTimerChanged"
          @error="actionError = $event"
          @clock="onPillClick"
        />
      </section>
    </div>

    <!-- 统计板（design：自统计钮飞出的玻璃板，翻面 = 今日明细；DOM 序先于侧钮，照抄 design） -->
    <StatsView
      ref="statsViewEl"
      :open="statsOpen"
      :refresh-key="statsRefreshKey"
      :auto-out-hours="settings?.workday_auto_out_hours ?? null"
    />

    <!-- 侧角按钮（左统计 · 右设置，对称落位）：两板互斥飞出 -->
    <button
      ref="statsBtnEl"
      class="sideButton statsButton"
      :class="{ open: statsOpen }"
      type="button"
      aria-label="统计"
      @click="toggleStats"
    >
      <svg viewBox="0 0 24 24" aria-hidden="true">
        <path d="M4 13h4v7H4zM10 8h4v12h-4zM16 4h4v16h-4z" />
      </svg>
      <span class="tooltip">统计</span>
    </button>
    <button
      ref="gearBtnEl"
      class="sideButton settingsButton"
      :class="{ open: panelVisible }"
      type="button"
      aria-label="设置"
      @click="togglePanel"
    >
      <svg viewBox="0 -960 960 960" aria-hidden="true">
        <path
          d="m370-80-16-128q-13-5-24.5-12T307-235l-119 50L78-375l103-78q-1-7-1-13.5v-27q0-6.5 1-13.5L78-585l110-190 119 50q11-8 23-15t24-12l16-128h220l16 128q13 5 24.5 12t22.5 15l119-50 110 190-103 78q1 7 1 13.5v27q0 6.5-2 13.5l103 78-110 190-118-50q-11 8-23 15t-24 12L590-80H370Zm70-80h79l14-106q31-8 57.5-23.5T639-327l99 41 39-68-86-65q5-14 7-29.5t2-31.5q0-16-2-31.5t-7-29.5l86-65-39-68-99 42q-22-23-48.5-38.5T533-694l-13-106h-79l-14 106q-31 8-57.5 23.5T321-633l-99-41-39 68 86 64q-5 15-7 30t-2 32q0 16 2 31t7 30l-86 65 39 68 99-42q22 23 48.5 38.5T427-266l13 106Zm42-180q58 0 99-41t41-99q0-58-41-99t-99-41q-59 0-99.5 41T342-480q0 58 40.5 99t99.5 41Zm-2-140Z"
        />
      </svg>
      <span class="tooltip">设置</span>
    </button>

    <!-- 打卡双向确认框（design .overlay .sheet；DOM 序照抄：侧钮后、设置板前） -->
    <ConfirmModal
      :open="confirmMode != null"
      :title="confirmMode === 'in' ? '确认上班打卡' : '确认下班打卡'"
      :message="
        confirmMode === 'in'
          ? '开始记录今日在岗时长；班内计时 = 工作、空隙 = 休息。'
          : '班内计时将收段落库，并结算今日在岗 / 工作 / 休息三值。'
      "
      @confirm="onConfirmOk"
      @cancel="onConfirmCancel"
    />

    <!-- 设置板（design：自齿轮中心缩放飞出，覆于内容之上；settings 就绪才挂载） -->
    <SettingsPanel
      v-if="settings"
      ref="settingsPanelEl"
      :open="panelVisible"
      :settings="settings"
      :error="saveError"
      @save="onSaveSettings"
    />
  </main>
  <audio ref="chimeRef" :src="chimeUrl" preload="auto"></audio>
</template>

<style>
/* PL016.1/PL017.1/PL018.1 令牌与分类样式唯一来源：design 侧 1:1 副本（差异登记见各文件头）。
   旧糖果令牌（PL006/PL007）已退役——单态外观为设计定案（有意分叉，登记 z.plan 附录 PL016） */
@import "./src/styles/glass.css";
@import "./src/styles/topbar.css";
@import "./src/styles/timer.css";
@import "./src/styles/hourglass.css";
@import "./src/styles/controls.css";
@import "./src/styles/stats.css";
@import "./src/styles/graph.css";
@import "./src/styles/boards.css";

/* —— 最小功能胶水（非设计定稿样式）：错误条红字 + 动效退避，仅此两件 —— */
/* 文案条错误条红字（remind/auto 配方在 controls.css） */
.banner.error {
  color: #b42318;
}
/* 动效退避（可达性基建，非设计范围；design 侧退避随 devkit 不迁，
   分类样式内声明式退避随各 PL 落位） */
@media (prefers-reduced-motion: reduce) {
  *,
  *::before,
  *::after {
    transition-duration: 0.01ms !important;
    animation-duration: 0.01ms !important;
  }
}
</style>
