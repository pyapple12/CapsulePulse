<script setup lang="ts">
import { onMounted, onUnmounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
// PL008.3：dock 时代 ⚙ 换 lucide 线性图标（与 DockNav 同源图标库）
import { Settings } from "lucide-vue-next";

import ConfirmModal from "./components/ConfirmModal.vue";
import DockNav from "./components/DockNav.vue";
import SettingsPanel from "./components/SettingsPanel.vue";
import StatsCard from "./components/StatsCard.vue";
import StatsView from "./components/StatsView.vue";
import TimerCard from "./components/TimerCard.vue";
// IPC DTO 镜像类型统一收敛在 types.ts（单一来源 = Rust serde 结构，防多处声明漂移）
import type { DaySummary, ReminderSettings, SessionStats } from "./types";
// 展示格式化共享助手（FIX002.13 收敛）
import { hhmm } from "./format";
// 提示音经 vite 打包（哈希进 dist）——不用 public/ 目录（publicDir 默认在根，曾有 404 教训）
import chimeUrl from "../assets/house_alarm-clock_loud.mp3";

// 玻璃卡片 + 拖动区沿用 PL001 阶段 B 判定形态；计时在 TimerCard，统计聚合/提醒判定在 Rust

// 统计为低频数据：挂载 + 动作后（TimerCard changed 事件）+ 30s 兜底，不进 100ms tick
const STATS_TICK_MS = 30_000;

const todaySecs = ref(0);
const weekSecs = ref(0);
const allSecs = ref(0);
// 今日工作秒（PL008.4 环口径：day_detail.work_secs，daywork 口径，随统计刷新节奏更新）
const todayWorkSecs = ref(0);
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
// PL005：打卡确认框 + 双标签视图（pill 移入 TimerCard 后，在岗态由其自有轮询驱动）
const confirmMode = ref<"in" | "out" | null>(null);
const activeTab = ref<"timer" | "stats">("timer");
const autoOutVisible = ref(false);
const autoOutAt = ref(0);
// 统计视图刷新信号：计时/打卡动作后自增，StatsView watch 重拉（保持 Rust 不推送定案）
const statsRefreshKey = ref(0);
let statsTimer: number | undefined;
// 自动下班条自隐句柄（FIX005.1：横幅不再滞留至重启；新触发先清旧防叠加）
let autoOutHideTimer: number | undefined;
let unlistenReminder: (() => void) | undefined;
let unlistenAutoOut: (() => void) | undefined;
// PL011 分态纱浓度：聚焦磨砂态 0% 纱（磨砂已足够）、失焦透明态 30% 纱（保可读）——
// 初值经 isFocused 查询兜底，此后随 Rust 的 window-focus 事件翻转
const windowFocused = ref(false);
let unlistenFocus: (() => void) | undefined;

// —— PL010.1 拖拽修复：data-tauri-drag-region 只在"被点中元素自身"带属性时生效，
// 弹性布局铺满后 main 无裸区可点（回归 bug）——改为全局 mousedown 接线：
// 交互元素白名单命中不抢，其余一律启动窗口拖拽（点按语义不受影响）；
// .overlay 不可拖（FIX003.1）：遮罩 @click.self 点外关闭依赖 click，拖拽循环会吞掉它——
const DRAG_INTERACTIVE =
  "button, input, textarea, select, a, .dock, .floating-sheet, .pill, .detail-panel, .overlay";

/** 非交互区按下即启动窗口拖拽 */
function onWindowDown(e: MouseEvent): void {
  if (e.button !== 0) {
    return;
  }
  const target = e.target as HTMLElement | null;
  if (target?.closest(DRAG_INTERACTIVE)) {
    return;
  }
  void getCurrentWindow().startDragging();
}

/** 拉取统计快照 + 今日工作秒（环口径：day_detail.work_secs，与统计同节奏更新） */
async function refreshStats(): Promise<void> {
  try {
    const s = await invoke<SessionStats>("session_stats");
    todaySecs.value = s.today_secs;
    weekSecs.value = s.week_secs;
    allSecs.value = s.all_secs;
  } catch (err) {
    console.error("session_stats 调用失败", err);
  }
  try {
    const day = await invoke<DaySummary>("day_detail", { offset: 0 });
    todayWorkSecs.value = day.work_secs;
  } catch (err) {
    console.error("day_detail 调用失败（环口径沿用上次取值）", err);
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

/** 保存设置（Rust 侧校验 + 持久化 + 即时生效）：成功收起面板；失败错误态传入面板可见反馈 */
async function onSaveSettings(s: ReminderSettings): Promise<void> {
  try {
    await invoke("set_settings", { settings: s });
    settings.value = s;
    saveError.value = "";
    panelVisible.value = false;
  } catch (err) {
    // CommandError 经 IPC 序列化为文案字符串；面板内展示 + console 留痕
    saveError.value = `设置保存失败：${String(err)}`;
    console.error("set_settings 调用失败", err);
  }
}

/** ⚙ 开合面板；打开时清掉上一轮保存失败的错误提示；settings 未就绪时可见反馈不静默（FIX002.3） */
function togglePanel(): void {
  if (settings.value == null) {
    actionError.value = "设置加载失败，请重启应用重试";
    return;
  }
  panelVisible.value = !panelVisible.value;
  if (panelVisible.value) {
    saveError.value = "";
  }
}

/** TimerCard 动作后：统计即刷；动作即处理提醒（暂停/重开 = 新段），文案条与错误条随之隐藏 */
function onTimerChanged(): void {
  void refreshStats();
  statsRefreshKey.value++;
  reminderVisible.value = false;
  actionError.value = "";
}

/** 切到统计页：单日明细即时重拉（动作后的快照可能已是旧账，如开始计时当秒的工作块） */
function onTabClick(tab: "timer" | "stats"): void {
  activeTab.value = tab;
  if (tab === "stats") {
    statsRefreshKey.value++;
  }
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
  // 焦点态初值兜底：错过启动期事件也不至于滞留错误纱浓度（查询失败仅记录，退回默认纱态）
  getCurrentWindow()
    .isFocused()
    .then((focused) => {
      windowFocused.value = focused;
    })
    .catch((err) => console.error("isFocused 查询失败", err));
  // window-focus：Rust Focused 事件转发（payload = 聚焦与否），驱动分态纱与 focused class
  listen<boolean>("window-focus", (event) => {
    windowFocused.value = event.payload;
  })
    .then((un) => {
      unlistenFocus = un;
    })
    .catch((err) => console.error("window-focus 监听注册失败", err));
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
  unlistenFocus?.();
});
</script>

<template>
  <main class="window" id="win" :class="{ focused: windowFocused }">
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
      <button
        class="sideButton settingsButton"
        type="button"
        aria-label="设置"
        @click="togglePanel"
      >
        <Settings :size="17" :stroke-width="2.2" aria-hidden="true" />
        <span class="tooltip">设置</span>
      </button>

      <!-- 文案条（提醒 / 自动下班 / 动作错误）：骨架占位，PL017 换 design .banner 配方 -->
      <div v-if="reminderVisible" class="banner remind">
        <span class="dot"></span><span>已连续工作 {{ reminderThreshold }} 分钟，休息一下吧</span>
      </div>
      <div v-if="autoOutVisible" class="banner auto">
        <span class="dot"></span><span>已于 {{ hhmm(autoOutAt) }} 自动下班</span>
      </div>
      <div v-if="actionError" class="banner error" role="alert">{{ actionError }}</div>

      <!-- 双页 v-show 保活：TimerCard 的 100ms tick 是提醒/自动下班评估口，切页不得中断 -->
      <section v-show="activeTab === 'timer'" class="stage" id="page-timer">
        <TimerCard
          :work-secs="todayWorkSecs"
          :target-hours="settings?.workday_auto_out_hours ?? null"
          @changed="onTimerChanged"
          @error="actionError = $event"
          @clock="onPillClick"
        />
      </section>
      <section v-show="activeTab === 'stats'" class="stage" id="page-stats">
        <StatsView :refresh-key="statsRefreshKey" />
      </section>

      <!-- 过渡期遗留件（PL018/PL019 换装迁走）：功能保持、观感待换 -->
      <StatsCard :today-secs="todaySecs" :week-secs="weekSecs" :all-secs="allSecs" />
      <DockNav :active="activeTab" @change="onTabClick" />

      <Transition name="sheet">
        <div v-if="panelVisible && settings" class="overlay" @click.self="panelVisible = false">
          <section class="floating-sheet" role="dialog" aria-label="设置">
            <SettingsPanel :settings="settings" :error="saveError" @save="onSaveSettings" />
          </section>
        </div>
      </Transition>
    </div>
  </main>
  <ConfirmModal
    :open="confirmMode != null"
    :title="confirmMode === 'in' ? '上班打卡' : '下班打卡'"
    :message="confirmMode === 'in' ? '开始一天工作吗？' : '结束一天工作吗？'"
    @confirm="onConfirmOk"
    @cancel="onConfirmCancel"
  />
  <audio ref="chimeRef" :src="chimeUrl" preload="auto"></audio>
</template>

<style>
/* PL016.1 令牌唯一来源：design/glass.css 1:1 副本（差异登记见文件头）
   + 骨架件 topbar.css 1:1（.topbar/.title/.sideButton/.tooltip/.stage 布局）。
   旧糖果令牌（PL006/PL007）已退役——单态外观为设计定案（有意分叉，登记 z.plan 附录 PL016） */
@import "./src/styles/glass.css";
@import "./src/styles/topbar.css";

/* —— 过渡期最小功能胶水（非设计定稿样式）：仅保可运行；PL017–PL019 换装 design
   分类样式时逐一替换 —— */
/* 文案条骨架（PL017 换 design .banner 配方） */
.banner {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 13px;
}
.banner.error {
  color: #b42318;
}
/* 浮层结构骨架（确认框/设置板定位；糖果蒙皮已退役，PL019 换 design sheet 配方） */
.overlay {
  position: fixed;
  inset: 0;
  z-index: 20;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(0, 0, 0, 0.25);
}
.floating-sheet {
  display: flex;
  flex-direction: column;
  gap: 14px;
  width: 240px;
  padding: 20px;
  border-radius: 24px;
  background: rgba(245, 250, 255, 0.92);
  color: var(--ink);
  font-family: var(--font-stack);
  user-select: none;
}
/* 按钮最小功能形态（PL019 换 design 配方） */
.btn-primary,
.btn-ghost {
  border-radius: 999px;
  padding: 8px 18px;
  font-family: var(--font-stack);
}
.btn-primary {
  background: var(--accent);
  color: #fff;
}
.btn-primary:disabled,
.btn-ghost:disabled {
  opacity: 0.45;
  cursor: not-allowed;
}
/* 设置板开合过渡（曲线字面量替代已退役的 --ease-spring，节奏不变） */
.sheet-enter-active,
.sheet-leave-active {
  transition: opacity 0.15s ease;
}
.sheet-enter-active .floating-sheet {
  transition: transform 0.15s cubic-bezier(0.34, 1.56, 0.64, 1);
}
.sheet-enter-from,
.sheet-leave-to {
  opacity: 0;
}
.sheet-enter-from .floating-sheet {
  transform: scale(0.92);
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

<style scoped>
/* PL016.3 骨架胶水：.window/.content 的定妆在 glass.css、.topbar/.stage 在 topbar.css，
   此处仅保留过渡期旧组件的排布胶水；各页正式样式随 PL017–PL019 落位后本块收敛删除 */
.content > .banner {
  margin: 0;
}

/* 过渡期遗留件（StatsCard/DockNav，PL018/PL019 迁走）：压缩自身，不与 stage 抢空间 */
.content > :is(.stats, .dock) {
  flex: 0 0 auto;
}
</style>
