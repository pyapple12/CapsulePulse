<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";

import ConfirmModal from "./components/ConfirmModal.vue";
import SettingsPanel from "./components/SettingsPanel.vue";
import StatsView from "./components/StatsView.vue";
import TimerCard from "./components/TimerCard.vue";
import type { RecoveryNotice } from "./types";
// IPC DTO 镜像类型统一收敛在 types.ts（单一来源 = Rust serde 结构，防多处声明漂移）
import type {
  DaySummary,
  HotkeyAction,
  PrefsSnapshot,
  ReminderSettings,
  SessionStatus,
} from "./types";
// 展示格式化共享助手（FIX002.13 收敛）
import { hhmm } from "./format";
// 标题粒子化视觉件（design initTitleParticles 1:1，reduced-motion 内部退避静态文字）
import { initTitleParticles } from "./src/title-particles";
// 标题阈值拖拽把手（PATCH001.2，移植 CT useThresholdDrag：单击/双击不吞、按住才拖窗）
import { useThresholdDrag } from "./src/use-threshold-drag";
// 提示音经 vite 打包（哈希进 dist）——不用 public/ 目录（publicDir 默认在根，曾有 404 教训）
import chimeUrl from "../assets/house_alarm-clock_loud.mp3";

// 统计聚合/提醒判定在 Rust；本组件只做展示与命令转发

// 统计为低频数据：挂载 + 动作后（TimerCard changed 事件）+ 30s 兜底，不进 100ms tick
const STATS_TICK_MS = 30_000;

// 当日明细（PL017 dial 弧段 + 电池上班时刻的数据源；30s 兜底 + 动作后即刷）
const day = ref<DaySummary | null>(null);
/** 当前数据日偏移（PATCH005.4）：时钟与统计的取数锚点（`session_status.data_day_offset`） */
const dataDayOffset = ref(0);
/** 会话提示（PATCH010.8）：TimerCard 每分钟/变化时上报——设置板阈值行下限与锁定判据 */
const sessionHint = ref({ running: false, segmentMin: 0, reminderFired: false });
const settings = ref<ReminderSettings | null>(null);
const panelVisible = ref(false);
const reminderVisible = ref(false);
// 触发阈值取自 reminder-due 事件 payload（PATCH012.8 起 payload = **完整文案字符串**，
// Rust 侧单一来源；首次带阈值分钟数、重发换句式不带数值），不做前端默认值兜底
const reminderText = ref("");
// 设置保存失败的可见反馈（面板内展示，成功或重开面板时清除）
const saveError = ref("");
/** 录键失败反馈（PL028.2；带动作归属 ⇒ 显示在对应行下——后端已回滚旧键，界面仍显旧值） */
const hotkeyError = ref<{ action: HotkeyAction; message: string } | null>(null);
// 动作/打卡失败的可见反馈（FIX002.3：命令失败不再只进 console）；动作成功即清除
const actionError = ref("");
const chimeRef = ref<HTMLAudioElement | null>(null);
// PL005：打卡双向确认框（pill 在 TimerCard，方向经 clock 事件上抛，不直接执行）
const confirmMode = ref<"in" | "out" | null>(null);
// 两板互斥（design：统计板/设置板飞出，开一关一）
const statsOpen = ref(false);
const autoOutVisible = ref(false);
const autoOutAt = ref(0);
// PL025.8 自动下班待决：到点前 60s 弹确认框（倒计时 + 下班/加班）——到点由后端执行，前端只展示
const autoOutDue = ref<number | null>(null);
/** 启动对账提示（PL027.8；崩溃 / 正常退出皆走）：一次性——仅"判下班"分支有值（无缝续静默），取过即不再显示 */
const recoveryNotice = ref<number | null>(null);
const autoOutLeft = ref(0);
let autoOutCountdown: number | undefined;
let unlistenAutoOutPending: (() => void) | undefined;
// 统计视图刷新信号：计时/打卡动作后自增，StatsView watch 重拉（保持 Rust 不推送定案）
const statsRefreshKey = ref(0);
let statsTimer: number | undefined;
// —— 文案条浮层（PATCH005.11）——
// 四条条带（提醒 / 自动下班 / 恢复 / 错误）不参与文档流：以「日期卡 + 表盘」行（TimerCard
// .timer-top）为中心双向居中覆盖，被盖行自雾化；自隐 = 错误条 10s、其余 5s，任意点击亦立即隐藏
/** UI 提示时长（PATCH010.5 用户定案，**全应用统一**）：错误提示（红字）**10s** 自动消失、
 * 非红字提示 **5s** 自动消失——文案条与板内红字（板底保存失败 / 行内录键失败）共用同一口径。 */
const HINT_HIDE_MS = 5_000;
const ERROR_HIDE_MS = 10_000;

/** 数据刷新失败的可见提示文案（FIX010.10 方案 A）：与"成功即清"的判据共用同一常量，
 * 避免两处字符串漂移 */
const STATS_FAIL_MSG = "数据刷新失败，正在自动重试";
/** 连续失败计数 + "本轮已提示过"标记（FIX010.10）：`refreshStats` 是低频兜底口，
 * 失败**不得常驻提示**（PATCH010.5 口径）⇒ 同一轮故障只提示一次，成功即复位 */
let statsFailCount = 0;
let statsFailWarned = false;
let bannerHideTimer: number | undefined;
/** 录键失败短标签的自动恢复计时（PATCH010.4 用户定案：10s 后该行换回正常说明文字） */
let hotkeyErrorTimer: number | undefined;
/** 板底保存失败文案的自动消失计时（PATCH010.5 用户定案：10s） */
let saveErrorTimer: number | undefined;
/** 是否有任一条文案条可见（浮层 open 类 = 自雾化开关） */
const anyBannerVisible = computed(
  () =>
    reminderVisible.value ||
    autoOutVisible.value ||
    recoveryNotice.value != null ||
    actionError.value !== "",
);
let unlistenReminder: (() => void) | undefined;
let unlistenAutoOut: (() => void) | undefined;
// 标题粒子视觉件清理句柄（组件卸载时解绑 window 监听并停帧）
let destroyTitleParticles: (() => void) | undefined;
// 标题阈值拖拽解绑句柄（PATCH001.2：卸载时摘把手监听并收尾未决手势）
let destroyThresholdDrag: (() => void) | undefined;
// 分态纱焦点态（CT 定案）：失焦 30% 纱、聚焦 0% 纱桌面直透——Rust 转发 window-focus 驱动
// focused class；初值经 isFocused 查询兜底，错过启动期事件也不滞留错误纱态
const windowFocused = ref(false);
let unlistenFocus: (() => void) | undefined;
// 窗口偏好同步解绑句柄（PL028.3：托盘右键菜单改偏好 → 设置面板开关回显）
let unlistenPrefs: (() => void) | undefined;
// 拖拽结束事件解绑句柄（FIX008.17：补上——原实现漏存句柄，该订阅无法退订）
let unlistenDragEnd: (() => void) | undefined;

// —— PL010.1 拖拽修复：data-tauri-drag-region 只在"被点中元素自身"带属性时生效，
// 弹性布局铺满后 main 无裸区可点（回归 bug）——改为全局 mousedown 接线：
// 交互元素白名单命中不抢，其余一律启动窗口拖拽（点按语义不受影响）；
// 浮层件全部在列（PL019：板区不拖、确认遮罩不拖，design 无拖拽语义；**收板**另由下方
// NO_CLOSE_ON_PRESS 管 —— PATCH009.7 起两者刻意分开）；
// .topbar 自 PATCH001.2 起在列——标题改走阈值拖拽把手（否则"按下即拖"会吞掉双击）；
// label 自 PATCH002.2 起在列——三段控件的「工作/休息」是 <label>，不在白名单时按下即被
// startDragging 抢走鼠标捕获、click 永不派发（"按了没反应"根因；凡交互控件一律在列）
// .glass-thumb 自 FIX008.5 起在列——玻璃滚动浮钮的把手锚在 `#win`（**板外**节点、非板内子元素），
// 不入列则按下会走到 `markWindowDragStart()` 变成拖窗口、它自己的拖拽滚动永远起不来
const DRAG_INTERACTIVE =
  "button, input, textarea, select, a, label, .sideButton, #stats-board, #settings-board, .overlay, .topbar, .glass-thumb";

/** 按下（mousedown）时**不收板**的区域（PATCH009.7 / PATCH009.8 用户定案）：板内交互、确认
 * 遮罩、两个开板钮（各自负责开合，点另一侧钮仍由钮自己切到那一板）+ **标题**。与
 * `DRAG_INTERACTIVE` **刻意分开**：标题必须在拖拽白名单里（阈值把手：按下只记起点、超 5px
 * 才启动拖拽）—— 原先两者共用一个常量，标题命中白名单即 return，导致"板开着点标题没反应"。
 * 标题的收板改挂 `click`（PATCH009.8）：拖动时 `startDragging()` 会吞掉 click ⇒ 拖拽与单击
 * 天然分流（CT `useThresholdDrag` 注释同史），故标题不进"按下即收"这条路。
 * `.glass-bar`（FIX008.5）：玻璃滚动浮钮锚在 `#win`（统计板的**兄弟**节点、板外）⇒ 不入本列时
 * 按下会命中"按下即收"直接把板关掉，浮钮的拖拽滚动永远用不上；故必须显式入列。 */
const NO_CLOSE_ON_PRESS =
  "#stats-board, #settings-board, .overlay, .sideButton, .topbar, .glass-bar";

// —— design flyBoard（PL019.2）：把开板按钮中心换算成板内坐标写入 transform-origin，
// 开板即自该按钮处缩放飞出；BOARD_TOP/LEFT = boards.css 板位常量（design 同名）——
const BOARD_TOP = 54;
const BOARD_LEFT = 12;
const winEl = ref<HTMLElement | null>(null);
// 顶栏把手（PATCH001.2：阈值拖拽挂点，同时是双击缩回托盘的点击目标）
const topbarEl = ref<HTMLElement | null>(null);
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

/** 窗口级按下分流（PATCH009.7 起**收板判定与拖拽判定分离**）：先收板（`NO_CLOSE_ON_PRESS`
 * 之外的位置按下即收两板，收板消费本次按下、不抢拖拽，元素自身 click 照常派发）→
 * 再按拖拽白名单分流 → 其余启动窗口拖拽。标题不在收板之列（改由 `onTitleClick` 走 click）。 */
function onWindowDown(e: MouseEvent): void {
  if (e.button !== 0) {
    return;
  }
  // 文案条浮层（PATCH005.11）：任意位置按下即隐（先于收板 / 白名单分支，保证"任意位置"语义）
  hideAllBanners();
  const target = e.target as HTMLElement | null;
  // 收板：除"板内 / 遮罩 / 开板钮 / 标题"外，任何位置（内容区、dock、空白…）按下即收两板
  if ((statsOpen.value || panelVisible.value) && target?.closest(NO_CLOSE_ON_PRESS) == null) {
    statsOpen.value = false;
    panelVisible.value = false;
    return;
  }
  if (target?.closest(DRAG_INTERACTIVE)) {
    return;
  }
  markWindowDragStart(); // PATCH011：拖拽期间屏蔽焦点抖动（原生拖拽循环会成对抖 false/true）
  void getCurrentWindow().startDragging();
}

/** 标题单击收两板（PATCH009.8 用户定案）：板开着时点标题即收两板；**拖拽不触发** —— 阈值把手
 * 在位移超 5px 时启动系统拖拽，拖拽循环吞掉后续 click（CT 实测同款分流），故本回调只在
 * "真单击"时到达；板没开时是空操作。 */
function onTitleClick(): void {
  if (statsOpen.value || panelVisible.value) {
    statsOpen.value = false;
    panelVisible.value = false;
  }
}

/** 双击标题缩回托盘（PATCH001.2，CT 同款交互）：与 Alt+F4 / 托盘显隐同语义（隐藏到托盘，
 * 真退仍走托盘菜单）；失败经既有错误条可见（严格抛错主线，不静默） */
async function onTitleDblClick(): Promise<void> {
  actionError.value = "";
  try {
    await invoke("main_hide_to_tray");
  } catch (err) {
    actionError.value = `缩回托盘失败：${String(err)}`;
    console.error("main_hide_to_tray 调用失败", err);
  }
}

/** 拉取当日明细（dial 弧段 + 电池数据源；统计的详细刷新在 StatsView 自持）。
 * PATCH005.4：取数锚定**当前数据日**（`session_status.data_day_offset`）——跨零点夜班没下班就
 * 不切换、锚在打卡日；下班 / 自动下班后偏移回 0，视图随之切回"今日未上班"。 */
async function refreshStats(): Promise<void> {
  try {
    const status = await invoke<SessionStatus>("session_status");
    dataDayOffset.value = status.data_day_offset;
    day.value = await invoke<DaySummary>("day_detail", { offset: status.data_day_offset });
    // FIX010.10：本轮恢复正常 ⇒ 复位失败计数，并撤掉刚才那条失败提示（自愈后不挂残留红字）
    statsFailCount = 0;
    statsFailWarned = false;
    if (actionError.value === STATS_FAIL_MSG) {
      actionError.value = "";
    }
  } catch (err) {
    // FIX010.10（方案 A）：失败**主动可见**（此前只 console）；错误详情仍只进 console，UI 只放短文案。
    // 限流：连续 ≥2 次才提示，且**同一轮故障只提示一次**（禁止常驻提示——PATCH010.5 口径；
    // 提示条自隐走 ERROR_HIDE_MS = 10s，见下面的 banner 计时 watch）
    console.error("session_status / day_detail 调用失败", err);
    statsFailCount += 1;
    if (statsFailCount >= 2 && !statsFailWarned) {
      statsFailWarned = true;
      actionError.value = STATS_FAIL_MSG;
    }
  }
}

/** 拉取提醒设置 */
async function refreshSettings(): Promise<void> {
  try {
    settings.value = await invoke<ReminderSettings>("get_settings");
  } catch (err) {
    // FIX006.3（方案 C）：读取失败**主动可见报错**（不再静默）；前端也不再造 50/8 默认值，
    // 未就绪期间沙漏不画水位、电池不判加班区（TimerCard 见 undefined 即按"未知"处理）
    console.error("get_settings 调用失败", err);
    actionError.value = "设置加载失败，请重启应用重试";
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
 * （PL014 定案，点板外收板），成功仅清错误；失败错误态传入面板可见反馈。
 * PATCH010.5：板底错误文案 **10s 自动消失**（用户定案：UI 所有错误提示一律 10s）。 */
async function onSaveSettings(s: ReminderSettings): Promise<void> {
  try {
    await invoke("set_settings", { settings: s });
    settings.value = s;
    clearSaveError();
  } catch (err) {
    // CommandError 经 IPC 序列化为文案字符串；PATCH010.4 起板底**只显固定文案**
    // （「设置保存失败」，不换行），完整原因进 console（用户定案）
    // FIX006.20a：**先清空、等一拍、再置文案**——同文案连续两次失败时值没变 ⇒ 面板侧
    // `watch(() => props.error)` 不触发 ⇒ 控件"回弹"不发生（用户失败后又点了开关 ⇒ 残留假象）
    saveError.value = "";
    await nextTick();
    saveError.value = "设置保存失败";
    console.error("set_settings 调用失败", err);
    window.clearTimeout(saveErrorTimer);
    saveErrorTimer = window.setTimeout(clearSaveError, ERROR_HIDE_MS);
  }
}

/** 清板底保存失败态（成功 / 10s 到点 / 开板 / 卸载共用） */
function clearSaveError(): void {
  window.clearTimeout(saveErrorTimer);
  saveError.value = "";
}

/** 录键失败**短标签**（PATCH010.4 用户定案）：设置板那一行只显示**分类**，完整后端文案进 console——
 * 该行宽仅 ~140px（= 板内容 248 − 录键钮 ≤98 − 行 gap 10），长文案必换行。
 * 分类靠**子串匹配**后端文案（`CommandError::Hotkey` / `Poisoned` / `HotkeyRegister` 的既有措辞）——
 * **后端改这几条文案时必须同步本表**，未命中一律落兜底「保存失败」（不会崩，只是标签变粗）。 */
function hotkeyFailLabel(raw: string): string {
  const table: Array<[string, string]> = [
    ["热键组合为空", "热键组合为空"],
    ["缺少修饰键", "缺少修饰键"],
    ["缺少主键", "缺少主键"],
    ["不支持的按键", "不支持的按键"],
    ["重复的按键段", "重复的按键段"],
    ["按键过多", "按键过多"],
    ["锁已中毒", "设置锁已中毒"],
    ["注册失败", "快捷键注册失败"],
  ];
  return table.find(([needle]) => raw.includes(needle))?.[1] ?? "保存失败";
}

/** 录键完成（PL028.2）：交后端校验 + 重注册（冲突/非法后端已回滚旧键并上抛 ⇒ 行内红字；
 * 成功则回填规范化串，界面与落库同源）。PATCH010.4：行内只显**短标签**（10s 后自动换回说明文字），
 * 完整文案进 console。 */
async function onSetHotkey(action: HotkeyAction, combo: string): Promise<void> {
  try {
    const normalized = await invoke<string>("set_hotkey", { action, combo });
    if (settings.value != null) {
      settings.value =
        action === "toggle_visible"
          ? { ...settings.value, hotkey_toggle_visible: normalized }
          : { ...settings.value, hotkey_toggle_timer: normalized };
    }
    clearHotkeyError();
  } catch (err) {
    hotkeyError.value = {
      action,
      message: `快捷键保存失败：${hotkeyFailLabel(String(err))}`,
    };
    console.error("set_hotkey 调用失败", err);
    // 10s 后自动换回正常说明文字（用户定案）；开板 / 卸载亦清除
    window.clearTimeout(hotkeyErrorTimer);
    hotkeyErrorTimer = window.setTimeout(clearHotkeyError, ERROR_HIDE_MS);
  }
}

/** 清录键失败态（成功 / 10s 到点 / 开板 / 卸载共用） */
function clearHotkeyError(): void {
  window.clearTimeout(hotkeyErrorTimer);
  hotkeyError.value = null;
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
    clearSaveError();
    // PATCH010.4：开板即清录键失败态（10s 内关板再开板 ⇒ 立刻换回正常说明文字，用户定案）
    clearHotkeyError();
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

/** 会话提示（PATCH010.8）：TimerCard 顺手上报（分钟粒度、变化才发）⇒ 设置板阈值行的
 * 下限与锁定判据。**不另开 App 侧轮询**（用户定案：单一来源、零时差、零额外 IPC）。 */
function onSessionHint(running: boolean, segmentMin: number, reminderFired: boolean): void {
  sessionHint.value = { running, segmentMin, reminderFired };
}

// —— PL029 板开合驱动的系统背板：板开 && 聚焦 ⇒ 挂；收板 / 失焦 ⇒ 摘 ——
/** 开板**挂**背板的延时（ms）：开板动画（`scale(0) → scale(1)`）共 400ms，
 * 取"**结束前 200ms**" = 200ms —— 动画进行中挂上（板已基本展开），起手不突变（用户 2026-10-16 定案）。 */
const BACKDROP_ON_DELAY_MS = 200;
/** 收板**摘**背板的延时（ms）：收板动画共 350ms，取"**开始后 200ms**" —— 板已明显缩小再摘（同定案）。 */
const BACKDROP_OFF_DELAY_MS = 200;
let backdropOnTimer: number | undefined;
let backdropOffTimer: number | undefined;
/** 当前已下发的背板态（去重：避免重复 invoke） */
let backdropOn = false;

/** 挂背板（唯一出口）：清两定时器 + 下发 */
function turnBackdropOn(): void {
  window.clearTimeout(backdropOnTimer);
  window.clearTimeout(backdropOffTimer);
  backdropOnTimer = undefined;
  backdropOffTimer = undefined;
  backdropOn = true;
  void invoke("set_board_backdrop", { open: true }).catch((err) =>
    console.error("set_board_backdrop 调用失败", err),
  );
}

/** 摘背板（唯一出口）：清两定时器 + 下发 */
function turnBackdropOff(): void {
  window.clearTimeout(backdropOnTimer);
  window.clearTimeout(backdropOffTimer);
  backdropOnTimer = undefined;
  backdropOffTimer = undefined;
  backdropOn = false;
  void invoke("set_board_backdrop", { open: false }).catch((err) =>
    console.error("set_board_backdrop 调用失败", err),
  );
}

/** 拖拽中标记（PATCH011）：原生拖拽循环会给出成对 `Focused(false/true)` 抖动，
 * 拖拽期间把它视作"仍聚焦"，避免背板被误摘（修"板开着拖标题 ⇒ 磨砂摘了又挂上"）。 */
const windowDragging = ref(false);
/** 安全阀（ms）：`drag-end` 若因故未到（事件丢失 / 异常路径），最多忽略焦点这么久，防永久屏蔽 */
const DRAG_SAFETY_MS = 10_000;
let dragSafetyTimer: number | undefined;

/** 标记"开始拖拽"（两处拖拽入口共用：标题阈值把手 + 全局 mousedown 路径） */
function markWindowDragStart(): void {
  windowDragging.value = true;
  window.clearTimeout(dragSafetyTimer);
  dragSafetyTimer = window.setTimeout(() => {
    dragSafetyTimer = undefined;
    windowDragging.value = false;
  }, DRAG_SAFETY_MS);
}

/** 解除"拖拽中"（Rust `drag-end` 事件；watch 随之按真实焦点重算一次） */
function clearWindowDragging(): void {
  window.clearTimeout(dragSafetyTimer);
  dragSafetyTimer = undefined;
  windowDragging.value = false;
}

/** 板开合 / 焦点 / 拖拽变化 ⇒ 同步系统背板（PL029）：开板延时 200ms 挂、收板延时 200ms 摘、
 * 失焦立即摘（**拖拽期间视作聚焦**，见 PATCH011） */
function syncBoardBackdrop(): void {
  const want =
    (panelVisible.value || statsOpen.value) && (windowFocused.value || windowDragging.value);
  if (want) {
    window.clearTimeout(backdropOffTimer);
    backdropOffTimer = undefined;
    if (!backdropOn && backdropOnTimer === undefined) {
      backdropOnTimer = window.setTimeout(turnBackdropOn, BACKDROP_ON_DELAY_MS);
    }
    return;
  }
  // 不再需要：撤掉"待挂"（开板动画中途又收板 ⇒ 干脆不挂）
  window.clearTimeout(backdropOnTimer);
  backdropOnTimer = undefined;
  if (!backdropOn || backdropOffTimer !== undefined) {
    return;
  }
  if (!windowFocused.value) {
    turnBackdropOff(); // 失焦：立即摘（防 DWM 灰板）
    return;
  }
  backdropOffTimer = window.setTimeout(turnBackdropOff, BACKDROP_OFF_DELAY_MS);
}

watch([panelVisible, statsOpen, windowFocused, windowDragging], syncBoardBackdrop);

// —— PL029 失焦自动收板（2026-10-16 用户定案）：板开着 + 窗口失焦**持续满 15s** ⇒ 自动收两板 ——
// 语义 = "失焦相当于 app 没操作"（失焦期间收不到用户操作，故"没操作"实现为"失焦持续"）。
// 重新聚焦或中途手动收板即取消计时；缩回托盘（窗口隐藏）同走失焦 ⇒ 15s 后自动收板。
/** 失焦自动收板的阈值（ms；用户 2026-10-16 由 30s 收紧到 15s） */
const BOARD_AUTOCLOSE_MS = 15_000;
let boardAutocloseTimer: number | undefined;

watch([windowFocused, panelVisible, statsOpen], () => {
  window.clearTimeout(boardAutocloseTimer);
  boardAutocloseTimer = undefined;
  const boardOpen = panelVisible.value || statsOpen.value;
  if (!boardOpen || windowFocused.value) {
    return;
  }
  boardAutocloseTimer = window.setTimeout(() => {
    boardAutocloseTimer = undefined;
    panelVisible.value = false;
    statsOpen.value = false;
  }, BOARD_AUTOCLOSE_MS);
});

/** 打卡 pill（TimerCard 上抛方向）→ 弹对应方向确认框（双向确认，不直接执行） */
function onPillClick(direction: "in" | "out"): void {
  confirmMode.value = direction;
}

/** 确认框取消：仅收起 */
function onConfirmCancel(): void {
  confirmMode.value = null;
}

/** 收起自动下班条（FIX005.1：展示层生命周期与打卡流对称；自隐与点击统一走 hideAllBanners） */
function clearAutoOutBanner(): void {
  autoOutVisible.value = false;
}

/** 隐藏全部文案条（PATCH005.11 唯一出口：到点自隐与"任意位置点击"共用） */
function hideAllBanners(): void {
  reminderVisible.value = false;
  autoOutVisible.value = false;
  recoveryNotice.value = null;
  actionError.value = "";
  if (bannerHideTimer !== undefined) {
    window.clearTimeout(bannerHideTimer);
    bannerHideTimer = undefined;
  }
}

/** 起/续文案条自隐计时（PATCH005.11）：有错误条取 10s、否则 5s；无可隐内容时清表 */
function armBannerAutoHide(): void {
  if (bannerHideTimer !== undefined) {
    window.clearTimeout(bannerHideTimer);
    bannerHideTimer = undefined;
  }
  if (!anyBannerVisible.value) {
    return;
  }
  bannerHideTimer = window.setTimeout(
    hideAllBanners,
    actionError.value !== "" ? ERROR_HIDE_MS : HINT_HIDE_MS,
  );
}

// 文案条可见性变化（出现 / 切换 / 消失）→ 起自隐计时（定位由插槽天然居中，无需测量）
watch([reminderVisible, autoOutVisible, recoveryNotice, actionError], () => {
  armBannerAutoHide();
});

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

/** PL025.8 自动下班待决：记到点时刻并起 1s 倒计时（后端已把窗口拉到前台，前端只展示与选择） */
function showAutoOutPending(dueAt: number): void {
  autoOutDue.value = dueAt;
  tickAutoOutLeft();
  if (autoOutCountdown === undefined) {
    autoOutCountdown = window.setInterval(tickAutoOutLeft, 1000);
  }
}

/** 倒计时刷新（到点即 0；到点后后端执行并回 workday-auto-out → 关框）。
 * FIX006.24：框开着时顺带**每秒重刷 `day`**——`can_overtime` 的跨零点翻转由后端重算，
 * 否则它跟 `day` 一起走（最多滞后 30 秒）；只在该框打开的 ~60 秒内生效，零新 API。 */
function tickAutoOutLeft(): void {
  const due = autoOutDue.value;
  autoOutLeft.value = due == null ? 0 : Math.max(0, Math.ceil(due - Date.now() / 1000));
  if (due != null) {
    void refreshStats();
  }
}

/** 关自动下班确认框（下班/加班/后端已执行都走这里），并清倒计时句柄 */
function closeAutoOutPending(): void {
  autoOutDue.value = null;
  autoOutLeft.value = 0;
  if (autoOutCountdown !== undefined) {
    window.clearInterval(autoOutCountdown);
    autoOutCountdown = undefined;
  }
}

/** 加班钮可见性：**后端派生** `can_overtime`（FIX006.4——"到顶"与"跨日"两条业务规则下沉后端，
 * 前端不再自算日历日比较；后端 `overtime` 命令仍是权威门禁，跨日再点会得到明确错误）。
 * 依赖 autoOutLeft（框开着时每秒刷新）保证到点与跨零点判定随之更新。 */
const canOvertime = computed(() => {
  void autoOutLeft.value;
  return day.value?.can_overtime ?? false;
});

/** 自动下班框「下班」：立即打卡下班（后端按基准时刻记账） */
async function onAutoOutClockOut(): Promise<void> {
  actionError.value = "";
  try {
    await invoke("clock_out");
  } catch (err) {
    actionError.value = `下班失败：${String(err)}`;
    console.error("clock_out 调用失败", err);
  }
  closeAutoOutPending();
  reminderVisible.value = false;
  void refreshStats();
  statsRefreshKey.value++;
}

/** 自动下班框「加班」：把本班基准拉满 12h（计时继续）；失败经错误条可见 */
async function onAutoOutOvertime(): Promise<void> {
  actionError.value = "";
  try {
    await invoke("overtime");
  } catch (err) {
    actionError.value = `加班失败：${String(err)}`;
    console.error("overtime 调用失败", err);
  }
  closeAutoOutPending();
  // PATCH005.8：加班改了预算 / 轴末 / 电池分母 ⇒ 立即取数（与 onAutoOutClockOut 同族收尾；
  // 后端 `overtime` 只落流水 + 重排兜底线程，不发事件，不刷就得等 30s 轮询）
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
  // 标题阈值拖拽把手（PATCH001.2）：按下不立即拖，位移超 5px 才拖窗——单击/双击照常派发
  if (topbarEl.value != null) {
    destroyThresholdDrag = useThresholdDrag(topbarEl.value, { onDragStart: markWindowDragStart });
  }
  // 焦点态初值兜底：错过启动期事件也不至于滞留错误纱态（查询失败仅记录，退回常纱）
  getCurrentWindow()
    .isFocused()
    .then((focused) => {
      windowFocused.value = focused;
    })
    .catch((err) => console.error("isFocused 查询失败", err));
  // window-focus：Rust 焦点态转发（payload = 聚焦与否），驱动分态纱 focused class
  listen<boolean>("window-focus", (event) => {
    windowFocused.value = event.payload;
    // PATCH005.9：重新获焦时顺带取一次数——窗口若被托盘/热键隐藏较久，WebView 定时器可能被
    // 节流，唤起瞬间视图可能陈旧；这里补一次即刷（零新监听、零后端改动）
    if (event.payload) {
      void refreshStats();
      statsRefreshKey.value++;
    }
  })
    .then((un) => {
      unlistenFocus = un;
    })
    .catch((err) => console.error("window-focus 监听注册失败", err));
  // drag-end（PATCH011）：Rust 在"真拖动过 + 左键已松 + 静默满阈值"时发出 ⇒ 解除拖拽中标记
  listen("drag-end", () => {
    clearWindowDragging();
  })
    .then((un) => {
      unlistenDragEnd = un;
    })
    .catch((err) => console.error("drag-end 监听注册失败", err));
  // prefs-changed（PL028.3）：托盘右键菜单改了窗口偏好 → 设置面板开关回显同步。
  // 面板自改那一路已即时写入本地态（onSaveSettings），此处只补"另一入口"的回显
  listen<PrefsSnapshot>("prefs-changed", (event) => {
    if (settings.value != null) {
      settings.value = { ...settings.value, ...event.payload };
    }
  })
    .then((un) => {
      unlistenPrefs = un;
    })
    .catch((err) => console.error("prefs-changed 监听注册失败", err));
  // reminder-due：Rust 侧评估触发（payload = 触发时的真实阈值分钟数，直显文案条）；注册失败必须可见
  listen<string>("reminder-due", (event) => {
    reminderText.value = event.payload;
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
  // PL027.8 启动对账提示（一次性；崩溃 / 正常退出皆走）：对账结果自后端取一次——仅"判下班"分支有值，弹文案条；
  // 无缝续分支返回 null（静默，不打扰）
  invoke<RecoveryNotice | null>("take_recovery_notice")
    .then((notice) => {
      if (notice?.closed_at != null) {
        recoveryNotice.value = notice.closed_at;
      }
    })
    .catch((err) => console.error("take_recovery_notice 调用失败", err));
  // PATCH005.13 启动期"自动下班"补拉：后端在 webview 装载完成前 emit 的事件会被丢
  // （如"停机期间到期"的重开）⇒ 挂载时补拉一次（同 PL027.8 的"存槽 + 挂载拉取"范式）
  invoke<number | null>("take_auto_out_notice")
    .then((at) => {
      if (at != null) {
        autoOutAt.value = at;
        autoOutVisible.value = true;
      }
    })
    .catch((err) => console.error("take_auto_out_notice 调用失败", err));
  // workday-auto-out：payload = 回填下班时刻（上班 + N），文案条告知 + 界面即刷；
  // 自隐改由文案条浮层统一管理（PATCH005.11：5s + 任意点击，见 watch / hideAllBanners）
  listen<number>("workday-auto-out", (event) => {
    autoOutAt.value = event.payload;
    autoOutVisible.value = true;
    // 后端已执行（tick 或兜底线程）→ 待决确认框随之关闭（PL025.8）
    closeAutoOutPending();
    void refreshStats();
    statsRefreshKey.value++;
  })
    .then((un) => {
      unlistenAutoOut = un;
    })
    .catch((err) => console.error("workday-auto-out 监听注册失败", err));
  // PL025.8 workday-auto-out-pending：payload = 到点时刻——弹确认框（倒计时 + 下班/加班）；
  // 后端同时已把窗口拉到前台（show/unminimize/set_focus），前端只管展示与选择
  listen<number>("workday-auto-out-pending", (event) => {
    showAutoOutPending(event.payload);
  })
    .then((un) => {
      unlistenAutoOutPending = un;
    })
    .catch((err) => {
      actionError.value = `自动下班提示监听注册失败：${String(err)}`;
      console.error("workday-auto-out-pending 监听注册失败", err);
    });
});

onUnmounted(() => {
  if (statsTimer !== undefined) {
    window.clearInterval(statsTimer);
  }
  if (bannerHideTimer !== undefined) {
    window.clearTimeout(bannerHideTimer);
  }
  window.clearTimeout(hotkeyErrorTimer);
  window.clearTimeout(saveErrorTimer);
  window.clearTimeout(backdropOnTimer);
  window.clearTimeout(backdropOffTimer);
  window.clearTimeout(boardAutocloseTimer);
  window.clearTimeout(dragSafetyTimer);
  if (autoOutCountdown !== undefined) {
    window.clearInterval(autoOutCountdown);
  }
  window.removeEventListener("mousedown", onWindowDown);
  unlistenReminder?.();
  unlistenAutoOut?.();
  unlistenAutoOutPending?.();
  unlistenFocus?.();
  unlistenPrefs?.();
  unlistenDragEnd?.();
  destroyTitleParticles?.();
  destroyThresholdDrag?.();
});
</script>

<template>
  <main ref="winEl" class="window" id="win" :class="{ focused: windowFocused }">
    <div class="content">
      <header ref="topbarEl" class="topbar" @click="onTitleClick">
        <h1 class="title" @dblclick="onTitleDblClick">
          CapsulePulse<canvas class="title-canvas" aria-hidden="true"></canvas>
        </h1>
      </header>

      <!-- 计时页（设计单页常驻；统计/设置 = 侧角钮飞出板，浮层件均在 .window 直下与 .content 平级） -->
      <section class="stage" id="page-timer">
        <TimerCard
          :day="day"
          :threshold-min="settings?.threshold_min"
          :auto-out-hours="settings?.workday_auto_out_hours"
          @changed="onTimerChanged"
          @error="actionError = $event"
          @session="onSessionHint"
          @clock="onPillClick"
        >
          <!-- 文案条浮层（PATCH005.11）：插进 TimerCard 的「日期卡 + 表盘」行，以该行为定位锚
               ⇒ 双向居中覆盖、零测量零魔术数；绝对定位 ⇒ 不参与文档流（出现时下方内容零位移）；
               被盖行自雾化、浮层自身清晰；显隐走 v-if + 设计稿 .show（PATCH005.10）；
               自隐 5s / 错误条 10s，窗口内任意位置点击亦立即隐藏 -->
          <template #row-overlay>
            <div class="banner-layer" :class="{ open: anyBannerVisible }">
              <div v-if="reminderVisible" class="banner remind show">
                <span class="dot"></span><span>{{ reminderText }}</span>
              </div>
              <div v-if="autoOutVisible" class="banner auto show">
                <span class="dot"></span><span>已于 {{ hhmm(autoOutAt) }} 自动下班</span>
              </div>
              <div v-if="recoveryNotice != null" class="banner auto show">
                <span class="dot"></span
                ><span
                  >上次未打卡下班就离开了，已按
                  {{ hhmm(recoveryNotice) }} 结束工作记账；要继续请重新打卡</span
                >
              </div>
              <div v-if="actionError" class="banner error show" role="alert">
                <span class="dot"></span><span>{{ actionError }}</span>
              </div>
            </div>
          </template>
        </TimerCard>
      </section>
    </div>

    <!-- 统计板（design：自统计钮飞出的玻璃板，翻面 = 今日明细；DOM 序先于侧钮，照抄 design） -->
    <StatsView
      ref="statsViewEl"
      :open="statsOpen"
      :refresh-key="statsRefreshKey"
      :data-day-offset="dataDayOffset"
      :veiled="confirmMode != null || autoOutDue != null"
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
    <!-- PATCH012.5：标题居中 + 文案整定（用户定案） -->
    <ConfirmModal
      :open="confirmMode != null"
      :title="confirmMode === 'in' ? '确认上班打卡' : '打卡下班喽！'"
      :message="confirmMode === 'in' ? '(・∀・) EXCITED 上班啦！' : '＼(＾▽＾)／ YAY, Party!!!'"
      title-center
      @confirm="onConfirmOk"
      @cancel="onConfirmCancel"
    />

    <!-- PL025.8 自动下班确认框（到点前 60s 弹出，复用同一形态）：倒计时 + 下班/加班
         FIX006.3：文案的"满 N 小时"只取后端 `budget_hours`（框仅在岗时开 ⇒ 该值必已填），
         不再兜 `settings` / 8 -->
    <ConfirmModal
      :open="autoOutDue != null"
      title="即将自动下班"
      :message="`今日在岗累计满 ${day?.budget_hours ?? 0} 小时，到点将自动下班。`"
      :note="autoOutLeft > 0 ? `剩余 ${autoOutLeft} 秒` : '正在自动下班…'"
      ok-text="下班"
      cancel-text="加班"
      :cancel-visible="canOvertime"
      @confirm="onAutoOutClockOut"
      @cancel="onAutoOutOvertime"
    />

    <!-- 设置板（design：自齿轮中心缩放飞出，覆于内容之上；settings 就绪才挂载） -->
    <SettingsPanel
      v-if="settings"
      ref="settingsPanelEl"
      :open="panelVisible"
      :settings="settings"
      :error="saveError"
      :on-duty="day != null && day.duty_started_at != null && day.duty_ended_at == null"
      :auto-out-locked="day != null && day.duty_started_at != null"
      :session-running="sessionHint.running"
      :segment-min="sessionHint.segmentMin"
      :reminder-fired="sessionHint.reminderFired"
      :hotkey-error="hotkeyError"
      @save="onSaveSettings"
      @hotkey="onSetHotkey"
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
@import "./src/styles/glass-bar.css";

/* —— 最小功能胶水（非设计定稿样式）：文案条浮层 + 动效退避 —— */
/* 错误条：正文**不改色**（承白 ink，与其余三条一致——PATCH005.12 用户定案，原红字为历史胶水） */
.banner-layer .banner.error .dot {
  background: #ff6b6b; /* 错误条圆点红（PATCH005.12 用户定案） */
}
/* 文案条浮层（PATCH005.11）：插在「日期卡 + 表盘」行（TimerCard .timer-top）内，以该行为定位锚
   ⇒ 天然双向居中（横向 align-items:center + 纵向 top:50% 配 translateY(-50%)），零测量零魔术数；
   绝对定位 ⇒ 不参与文档流（出现时下方内容零位移）；pointer-events:none 让点击穿透
   （"任意位置点击即隐"与全局拖拽不受影响） */
.window .timer-top {
  position: relative; /* 浮层定位锚（PATCH005.11） */
}
.banner-layer {
  position: absolute;
  left: 0;
  right: 0;
  top: 50%;
  z-index: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 6px;
  transform: translateY(-50%);
  pointer-events: none;
}
.banner-layer .banner {
  width: 100%;
  margin: 0; /* 定位交给浮层，去掉设计稿的流内纵向落位 */
  /* 被盖行是亮色件（日期卡 / 表盘）时白字发虚（用户定案 2026-10-15 逐步加码：
     先 1px/3px 试读 → 2px/4px 更浓更大 → **白纱改黑纱**）：
     底由设计稿的 var(--panel-bg)（白纱）改为 **CT 黑纱系**（浮板同源），文字影保留 */
  background: rgba(0, 0, 0, 0.5);
  text-shadow: 2px 2px 4px rgba(0, 0, 0, 0.75);
}
/* 前面的圆点套同款影（点是背景色块，用 box-shadow 对齐文字影口径） */
.banner-layer .banner .dot {
  box-shadow: 2px 2px 4px rgba(0, 0, 0, 0.75);
}
/* 被盖行自雾化（与浮板同款配方：glass.css 的 filter blur）——只糊该行的内容件
   （日期卡 / 表盘），浮层自身保持清晰 */
.window .timer-top > :not(.banner-layer) {
  transition: filter 0.35s ease;
}
.window:has(.banner-layer.open) .timer-top > :not(.banner-layer) {
  filter: blur(4px);
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
