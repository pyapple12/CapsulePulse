<script setup lang="ts">
/** 设置板（design #settings-board 1:1 重装，PL019.2）：**七行**即时生效（PATCH009.2 下线一行、
 * PATCH009.3 合并一行）。常驻 DOM + .open 驱动显隐（boards.css visibility/scale 机制，v-if 会杀
 * 收板动画）；接线：阈值 change 夹取 / 三路开关 change（提醒通知一开关控两字段 / 置顶 / 贴边）/
 * 步进按钮 / 两行录键，全部即时上抛（设计无完成钮）。
 * 登记差异：① （PATCH009.2）设计稿"自动下班"开关**已下线**——后端自动下班本就是**常开固定
 * 功能**（core 零功能变更红线，z.plan 附录 PL016），故不再渲染该开关（原"常开禁用"占位一并移除）；
 * ② 「窗口置顶 / 贴边吸附」两行为
 * PL028.3 新增（设计稿无，有意补口；与托盘右键菜单双入口，prefs-changed 同步）；
 * ③ 「显隐 / 工作休息」快捷键两行为 PL028.2 新增（录键交互抄 CT：全局 capture + Esc 取消 +
 * 3 秒无操作自停；收板必须退出录制态，否则 capture 残留会吞掉整个应用按键）。 */
import { computed, onMounted, onUnmounted, ref, watch } from "vue";

import type { HotkeyAction, ReminderSettings } from "../types";

const props = defineProps<{
  /** 板开合（父持态，两板互斥）；.open 驱动 visibility + 自按钮中心 scale 飞出 */
  open: boolean;
  /** 当前生效设置（即时保存模型：每次变更即上抛，无草稿态） */
  settings: ReminderSettings;
  /** 保存失败文案（面板内可见反馈，FIX002.3 接线保留） */
  error?: string;
  /** 在岗中（PL025.5）：在岗期间禁止改自动下班小时数——本班基准不可中途改（账目口径唯一） */
  onDuty?: boolean;
  /** 今日已开启上班（PATCH005.3 整日锁定）：小时数**整日**禁改（含今天下班后），
   * 仅"第二天且未上班"可改——判据 = 今天有打卡（前端用 `day.duty_started_at != null`）。 */
  autoOutLocked?: boolean;
  /** 录键失败反馈（PL028.2；带动作归属——显示在对应行下，后端已回滚旧键） */
  hotkeyError?: { action: HotkeyAction; message: string } | null;
}>();

const emit = defineEmits<{
  save: [s: ReminderSettings];
  hotkey: [action: HotkeyAction, combo: string];
}>();

/** 布尔设置项键（三路：提醒通知 / 窗口置顶 / 贴边吸附）——回弹回写与切换上抛共用。
 * PATCH009.3 起"提醒声音 + 系统通知"合并为「提醒通知」一行一开关（控两字段），不再单列声音键。 */
type BooleanSettingKey = "notify_enabled" | "always_on_top" | "snap_to_edge";

const boardEl = ref<HTMLElement | null>(null);

/** 保存失败回弹：用户改动残留的控件视觉态（checkbox 点击/input 手写值）强制对齐真实设置。
 * vnode diff 对未发生变化的 checked/value 不做 DOM patch，光靠重渲染打不回去，必须手动回写；
 * error 清空（成功/重开面板）时控件本与 settings 一致，回写同值无害 */
watch(
  () => props.error,
  () => {
    const root = boardEl.value;
    if (root == null) {
      return;
    }
    const num = root.querySelector<HTMLInputElement>("input[type=number]");
    if (num != null) {
      num.value = String(props.settings.threshold_min);
    }
    // 开关按 data-pref 定位回写（PL028.3：弃下标索引防增删错位）
    // 提醒通知（PATCH009.3 合并）：一行一开关控两字段 ⇒ 回弹值取两者**与**（"要么都开要么都关"）
    const boxes: Array<[BooleanSettingKey, boolean]> = [
      ["notify_enabled", notifyOn.value],
      ["always_on_top", props.settings.always_on_top],
      ["snap_to_edge", props.settings.snap_to_edge],
    ];
    for (const [key, value] of boxes) {
      const box = root.querySelector<HTMLInputElement>(`.switch input[data-pref="${key}"]`);
      if (box != null) {
        box.checked = value;
      }
    }
  },
);

/** 阈值 change 夹取（design 守卫原样：空/0/NaN 回落 50，1~99 封顶）后即时上抛 */
function onThresholdChange(e: Event): void {
  const input = e.target as HTMLInputElement;
  const v = Math.min(99, Math.max(1, Number(input.value) || 50));
  input.value = String(v);
  emit("save", { ...props.settings, threshold_min: v });
}

/** 开关切换即时上抛（三路布尔字段各有后端字段；数值字段走专用守卫） */
function onSwitchChange(key: BooleanSettingKey, e: Event): void {
  const checked = (e.target as HTMLInputElement).checked;
  emit("save", { ...props.settings, [key]: checked });
}

/** 提醒通知（PATCH009.3 用户定案）：系统通知 + 提醒声音**合并为一行一开关**——**要么都开、
 * 要么都关**（本版不分，以后再分）。旧配置两值不一致时取**与**（都开才算开）。 */
const notifyOn = computed(() => props.settings.notify_enabled && props.settings.sound_enabled);

/** 提醒通知开关切换（PATCH009.3）：一次写入**同时置两字段**（后端字段与各自门控保持不动） */
function onNotifyChange(e: Event): void {
  const checked = (e.target as HTMLInputElement).checked;
  emit("save", { ...props.settings, notify_enabled: checked, sound_enabled: checked });
}

/** 步进 ±1（4~12 边界外静默：禁用态已挡，此处为键盘兜底，design stepAutoOut 同款守卫）；
 * 在岗期间整体禁改（PL025.5：本班基准不可中途改）——UI 已置灰，此处再兜一层 */
function stepAutoOut(delta: number): void {
  if (props.onDuty || props.autoOutLocked) {
    return;
  }
  const v = props.settings.workday_auto_out_hours + delta;
  if (v < 4 || v > 12) {
    return;
  }
  emit("save", { ...props.settings, workday_auto_out_hours: v });
}

// —— 全局热键录键（PL028.2；交互抄 CT 录键器）——
// 全局 capture 阶段拦截按键：Esc 取消 / 3 秒无操作自动结束（防吞按键）；
// **收板必须退出录制态**——capture 残留会让整个应用的键盘失灵（CT FIX004.2 教训）。

/** 正在录制的动作（null = 未录制；两行互斥，同一时刻只录一条） */
const recording = ref<HotkeyAction | null>(null);
let recordTimer = 0;

/** 退出录制态并清计时器（完成 / Esc / 收板 / 卸载都走此口）。
 * PATCH009.5 补记四：顺带**放弃录键按钮的焦点** —— 收尾后不留残留焦点（否则一个 Enter / Space
 * 就会重新进入录制态，而录制态会吞掉全部键盘输入，属风险态）。 */
function stopRecord(): void {
  recording.value = null;
  window.clearTimeout(recordTimer);
  const active = document.activeElement;
  if (active instanceof HTMLElement && active.classList.contains("hotkey-btn")) {
    active.blur();
  }
}

/** 录制 3 秒无操作自动结束：任何按键都算操作，重置计时。 */
function armRecordTimer(): void {
  window.clearTimeout(recordTimer);
  recordTimer = window.setTimeout(stopRecord, 3000);
}

/** 进入录制态：拦截下一个组合键。 */
function startRecord(action: HotkeyAction): void {
  recording.value = action;
  armRecordTimer();
}

/** 录制态按键分流：修饰键单独按下继续等待；主键（字母 / 数字 / F1–F12）完成组合；
 * Esc 取消；无修饰键的主键忽略（与后端 `hotkey::parse` 同规，防"录了就拒"）。 */
function onRecordKeydown(e: KeyboardEvent): void {
  const action = recording.value;
  if (action == null) {
    return;
  }
  e.preventDefault();
  e.stopPropagation();
  armRecordTimer();
  if (e.key === "Escape") {
    stopRecord();
    return;
  }
  const mods: string[] = [];
  if (e.ctrlKey) mods.push("Ctrl");
  if (e.altKey) mods.push("Alt");
  if (e.shiftKey) mods.push("Shift");
  if (e.metaKey) mods.push("Win");
  if (mods.length === 0) return; // 无修饰键：等修饰键先按下
  if (["Control", "Alt", "Shift", "Meta"].includes(e.key)) return; // 仍在按修饰键
  const mainKey = e.key.length === 1 ? e.key.toUpperCase() : e.key;
  if (!/^[A-Z0-9]$/.test(mainKey) && !/^F([1-9]|1[0-2])$/.test(mainKey)) return; // 不支持的主键忽略
  stopRecord();
  emit("hotkey", action, [...mods, mainKey].join("+"));
}

/** 热键显示串：后端存 `Super` 表 Win 键（插件解析器认 SUPER 不认 WIN），界面按 Windows 习惯显示 Win。 */
function displayHotkey(combo: string): string {
  return combo.replace(/\bSuper\b/g, "Win");
}

/** 录键按钮显示文案（录制态固定「录制中…」，其余取该条快捷键的显示串）——
 * 不可见量宽占位与 SVG 矢量文字两处共用同一函数，保证两者尺寸逐字一致。 */
function hotkeyLabel(action: HotkeyAction): string {
  if (recording.value === action) {
    return "录制中…";
  }
  const combo =
    action === "toggle_visible"
      ? props.settings.hotkey_toggle_visible
      : props.settings.hotkey_toggle_timer;
  return displayHotkey(combo);
}

onMounted(() => document.addEventListener("keydown", onRecordKeydown, true));
onUnmounted(() => {
  document.removeEventListener("keydown", onRecordKeydown, true);
  window.clearTimeout(recordTimer);
});

/** 收板即退出录制态（见上方"capture 残留吞按键"注记）。 */
watch(
  () => props.open,
  (open) => {
    if (!open) {
      stopRecord();
    }
  },
);
</script>

<template>
  <div id="settings-board" ref="boardEl" :class="{ open }">
    <div class="board-glass sheet">
      <div class="setting-row inline">
        <div>
          <p class="setting-name">提醒阈值（分钟）</p>
          <p class="setting-desc">连续工作达到阈值提醒休息</p>
        </div>
        <input
          type="number"
          :value="settings.threshold_min"
          min="1"
          max="99"
          @change="onThresholdChange"
        />
      </div>
      <div class="setting-row inline">
        <div>
          <p class="setting-name">提醒通知</p>
          <p class="setting-desc">达到阈值发送系统通知并播放提示音</p>
        </div>
        <label class="switch">
          <input
            type="checkbox"
            data-pref="notify_enabled"
            :checked="notifyOn"
            @change="onNotifyChange($event)"
          />
          <span class="slider"></span>
        </label>
      </div>
      <div class="setting-row inline">
        <div>
          <p class="setting-name">自动下班（小时）</p>
          <p class="setting-desc">
            {{ autoOutLocked ? "今日已开启上班，明日上班前可改" : "上限 4~12 小时" }}
          </p>
        </div>
        <div class="stepper">
          <button
            class="stepper-btn"
            aria-label="减少"
            type="button"
            :disabled="onDuty || autoOutLocked || settings.workday_auto_out_hours <= 4"
            @click="stepAutoOut(-1)"
          >
            <svg viewBox="0 0 26 22" aria-hidden="true">
              <rect class="st-frame" x="0.5" y="0.5" width="25" height="21" rx="6" />
              <path class="st-glyph" d="M10 11 H16" />
            </svg>
          </button>
          <span class="stepper-value">{{ settings.workday_auto_out_hours }}</span>
          <button
            class="stepper-btn"
            aria-label="增加"
            type="button"
            :disabled="onDuty || autoOutLocked || settings.workday_auto_out_hours >= 12"
            @click="stepAutoOut(1)"
          >
            <svg viewBox="0 0 26 22" aria-hidden="true">
              <rect class="st-frame" x="0.5" y="0.5" width="25" height="21" rx="6" />
              <path class="st-glyph" d="M13 8 V14 M10 11 H16" />
            </svg>
          </button>
        </div>
      </div>
      <!-- PL028.3：窗口偏好两行（设计稿无此两行，有意补口；与托盘右键菜单双入口） -->
      <div class="setting-row inline">
        <div>
          <p class="setting-name">窗口置顶</p>
          <p class="setting-desc">窗口恒在最前</p>
        </div>
        <label class="switch">
          <input
            type="checkbox"
            data-pref="always_on_top"
            :checked="settings.always_on_top"
            @change="onSwitchChange('always_on_top', $event)"
          />
          <span class="slider"></span>
        </label>
      </div>
      <div class="setting-row inline">
        <div>
          <p class="setting-name">贴边吸附</p>
          <p class="setting-desc">拖动松手自动贴边停驻</p>
        </div>
        <label class="switch">
          <input
            type="checkbox"
            data-pref="snap_to_edge"
            :checked="settings.snap_to_edge"
            @change="onSwitchChange('snap_to_edge', $event)"
          />
          <span class="slider"></span>
        </label>
      </div>
      <!-- PL028.2：两条全局热键（录键交互抄 CT；设计稿无此两行，有意补口） -->
      <div class="setting-row inline">
        <div>
          <p class="setting-name">显隐快捷键</p>
          <p class="setting-desc">
            {{
              recording === "toggle_visible" ? "按下新组合键（Esc 取消）" : "全局唤起 / 收起主窗"
            }}
          </p>
          <p v-if="hotkeyError?.action === 'toggle_visible'" class="hotkey-error" role="alert">
            {{ hotkeyError.message }}
          </p>
        </div>
        <button
          class="hotkey-btn"
          :class="{ recording: recording === 'toggle_visible' }"
          type="button"
          :aria-label="hotkeyLabel('toggle_visible')"
          @click="startRecord('toggle_visible')"
        >
          <!-- 不可见量宽占位：按钮尺寸仍由文字内容决定（观感零变化） -->
          <span class="hk-sizer" aria-hidden="true">{{ hotkeyLabel("toggle_visible") }}</span>
          <!-- 矢量层：框面 / 内环 / 录制环 / 文字同在**一个 SVG**（照 stepper）；
               按压缩放施加在 SVG 上 ⇒ 整体缩放，文字不再吃 HTML 文字的像素吸附（"按下下沉"根因） -->
          <svg class="hk-vec" aria-hidden="true">
            <rect class="hk-frame" />
            <rect class="hk-ring" />
            <text class="hk-label" x="50%" y="50%">{{ hotkeyLabel("toggle_visible") }}</text>
          </svg>
        </button>
      </div>
      <div class="setting-row inline">
        <div>
          <p class="setting-name">状态快捷键</p>
          <p class="setting-desc">
            {{ recording === "toggle_timer" ? "按下新组合键（Esc 取消）" : "全局切换工作 / 休息" }}
          </p>
          <p v-if="hotkeyError?.action === 'toggle_timer'" class="hotkey-error" role="alert">
            {{ hotkeyError.message }}
          </p>
        </div>
        <button
          class="hotkey-btn"
          :class="{ recording: recording === 'toggle_timer' }"
          type="button"
          :aria-label="hotkeyLabel('toggle_timer')"
          @click="startRecord('toggle_timer')"
        >
          <!-- 不可见量宽占位：按钮尺寸仍由文字内容决定（观感零变化） -->
          <span class="hk-sizer" aria-hidden="true">{{ hotkeyLabel("toggle_timer") }}</span>
          <svg class="hk-vec" aria-hidden="true">
            <rect class="hk-frame" />
            <rect class="hk-ring" />
            <text class="hk-label" x="50%" y="50%">{{ hotkeyLabel("toggle_timer") }}</text>
          </svg>
        </button>
      </div>
      <p v-if="error" class="save-error" role="alert">{{ error }}</p>
    </div>
  </div>
</template>

<style scoped>
/* PATCH009.4（用户 2026-10-16 定案）：七行为当前版本全部设置 ⇒ 行距**回到设计值 6px**
   （九行时代的 4px 挤压退役）；**每行 `flex: 1` 等分板内可用高**（铺满、节奏均匀；行内
   `align-items: center` 垂直居中由 `boards.css` 提供）；行高由等分自动决定（不另设 `min-height`）；
   **错误行内容超高时该行自行长高、其余行略缩**（flex 的 min-content 行为）；
   **设置行之间不再有任何分隔线**（`.setting-row + .setting-row` 的 border 规则已删）。 */
#settings-board .setting-row {
  flex: 1;
  padding: 6px 0;
}
/* 板内 flex 纵列：行等分可用高；`safe center` 兜底（内容超高时退化为顶排可滚、不裁顶） */
#settings-board .board-glass {
  display: flex;
  flex-direction: column;
  justify-content: center;
  justify-content: safe center;
}
/* 描述行收紧（8px 小字 + 行高 1.1 + 上边距 1px）——设计原样保留（PATCH009.4 只调行距/行高，
   不动行内字号） */
#settings-board .setting-desc {
  margin-top: 1px;
  line-height: 1.1;
}

/* 保存失败可见反馈（FIX002.3 接线保留；设计无此件，功能性补口非视觉裁定） */
.save-error {
  margin: 8px 0 0;
  font-size: 12px;
  color: #b42318;
}

/* 录键按钮与行内错误（PL028.2；设计无此件，功能性补口——观感对齐设置板控件语言：
   同面板数字输入框的 panel 配方，选中态描边用 accent）
   PATCH009.5（用户 2026-10-16 定案）：
   ① 行盒锁定：`line-height: 1` —— 录制态文案「录制中…」是 CJK，只能由回退字体（雅黑系）绘制，
      其 normal 行高比拉丁字体高 ⇒ 行盒被撑高、按钮比常态高一档（CT settings.css 同款注释与解法）；
   ② **全面 SVG 化（照 stepper：文字与框一体）**：按钮只剩透明点击壳 + 一个不可见量宽占位，
      整个视觉（框面 / 内环 / 录制环 / 文字）都在**同一个 SVG** 里；按压缩放施加在 SVG 上 ⇒
      矢量单元整体缩放 —— 文字不再吃 HTML 文字在形变时被吸附到整设备像素的落位（"按下文字
      下沉"的根因；125% 缩放下 1 设备像素 = 0.8 逻辑 px，吸附点一变就是那"沉一下"）；
   ③ 动效：hover 提亮一档 + 录制态**背景呼吸**（CT 原样节奏与形态，改为动框面 `fill`）
      + 按下双速（CP stepper 同款：按下 `0.07s ease-out` 缩 `0.94`、松手 `0.22s
      cubic-bezier(0.23, 1, 0.32, 1)` 弹性回位；数值全同 stepper，不引新值）；
   ④ 投影**留在 `<svg>` 元素自身的 `box-shadow`**（不进 SVG 内容）：SVG 内的 drop-shadow 按
      填充透明度衰减 —— 框面是半透明（暗主题 0.14）⇒ 投影几乎不可见（stepper 现即此状态）；
      留在元素上则与改造前**逐像素一致**，且随该元素一起缩放（仍是一体）。
   按钮尺寸仍由文字内容决定（`min-width: 96px` + `padding: 6px 10px` 不动）。 */
.hotkey-btn {
  position: relative;
  min-width: 96px;
  padding: 6px 10px;
  border: none;
  background: none; /* 视觉全部由内层 SVG 承担（框面 / 内环 / 录制环 / 文字） */
  color: var(--ink);
  font-family: inherit;
  font-size: 12px;
  line-height: 1; /* 锁行盒：防回退字体度量撑高（CT 同款惯例） */
  cursor: pointer;
}
/* 焦点环：已随"焦点环全局退役"（PATCH009.6，`glass.css` 的 `:focus, :focus-visible { outline: none }`）
   一并撤除 —— 录键按钮不再有 accent 蓝环、也不再露 UA 黑框（用户定案：全应用不留任何焦点环）。 */
/* 不可见量宽占位：按钮宽高仍由文字内容决定（与矢量文字同字号同字体 ⇒ 尺寸逐字一致） */
.hk-sizer {
  visibility: hidden;
}
/* 矢量层：铺满按钮（padding box）；投影与按压缩放都落在**这一个元素**上 */
.hk-vec {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  overflow: visible;
  pointer-events: none;
  border-radius: 9px; /* 与框同形，供投影取形 */
  box-shadow: var(--panel-shadow); /* 与改造前逐像素一致（见上方 ④） */
  transition: transform 0.22s cubic-bezier(0.23, 1, 0.32, 1); /* 松手弹性回位 */
}
.hotkey-btn:active .hk-vec {
  transform: scale(0.94); /* 按下瞬时缩一档（stepper 同款双速；数值同 stepper） */
  transition-duration: 0.07s;
  transition-timing-function: ease-out;
}
/* 框面 + 内环：1px 描边**内贴**（x/y 0.5px + calc(100% - 1px)；rx 8.5 = 现有圆角 9 − 0.5） */
.hk-frame {
  x: 0.5px;
  y: 0.5px;
  width: calc(100% - 1px);
  height: calc(100% - 1px);
  rx: 8.5px;
  fill: var(--panel-bg);
  stroke: var(--panel-ring);
  stroke-width: 1;
  transition: fill 0.15s ease;
}
.hotkey-btn:hover .hk-frame {
  fill: color-mix(in srgb, var(--panel-bg) 88%, white 12%);
}
/* accent 录制环：常态 0 宽（出现/消失可过渡）；再内缩 0.25px ⇒ 1.5px 环完整落在框内 */
.hk-ring {
  x: 0.75px;
  y: 0.75px;
  width: calc(100% - 1.5px);
  height: calc(100% - 1.5px);
  rx: 8.25px;
  fill: none;
  stroke: var(--accent);
  stroke-width: 0;
  transition: stroke-width 0.15s ease;
}
.hotkey-btn.recording .hk-ring {
  stroke-width: 1.5;
}
/* 矢量文字：fill 跟按钮 color（--ink）；`text-shadow` 走继承（与改造前同一条链 ⇒ 暗主题那层
   文字投影不变）；垂直定位按 em 盒居中（`dominant-baseline: central`，改完目测微调） */
.hk-label {
  fill: currentColor;
  font-family: var(--font-stack);
  font-size: 12px;
  text-anchor: middle;
  dominant-baseline: central;
}
/* 录制态背景呼吸（照 CT：常态亮度 ↔ 提亮一档，0.8s 往返）——改动作在框面 `fill` 上 */
.hotkey-btn.recording .hk-frame {
  animation: hotkey-pulse 0.8s ease-in-out infinite alternate;
}
@keyframes hotkey-pulse {
  from {
    fill: var(--panel-bg);
  }

  to {
    fill: color-mix(in srgb, var(--panel-bg) 84%, white 16%);
  }
}
/* reduced-motion 守卫（PL009 纪律）：关过渡与呼吸；录制态仍靠 accent 描边静态可辨 */
@media (prefers-reduced-motion: reduce) {
  .hk-vec,
  .hk-frame,
  .hk-ring {
    transition: none;
  }

  .hotkey-btn.recording .hk-frame {
    animation: none;
  }
}
.hotkey-error {
  margin: 2px 0 0;
  font-size: 10px;
  color: #b42318;
}
</style>
