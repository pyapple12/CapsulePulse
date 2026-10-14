<script setup lang="ts">
/** 设置板（design #settings-board 1:1 重装，PL019.2）：九行即时生效。
 * 常驻 DOM + .open 驱动显隐（boards.css visibility/scale 机制，v-if 会杀收板动画）；
 * 接线：阈值 change 夹取 / 四开关 change / 步进按钮 / 两行录键，全部即时上抛（设计无完成钮）。
 * 登记差异：① 第五行"自动下班"开关 demo 侧零接线（静态 checked）、后端自动下班常开且
 * core 零功能变更红线（z.plan 附录 PL016 明确不做）——渲染为常开禁用态，观感与设计一致
 * （boards.css 无 :disabled 样式），语义上不假装可切换；② 「窗口置顶 / 贴边吸附」两行为
 * PL028.3 新增（设计稿无，有意补口；与托盘右键菜单双入口，prefs-changed 同步）；
 * ③ 「显隐 / 工作休息」快捷键两行为 PL028.2 新增（录键交互抄 CT：全局 capture + Esc 取消 +
 * 3 秒无操作自停；收板必须退出录制态，否则 capture 残留会吞掉整个应用按键）。 */
import { onMounted, onUnmounted, ref, watch } from "vue";

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

/** 布尔设置项键（四路：声音 / 通知 / 窗口置顶 / 贴边吸附）——回弹回写与切换上抛共用 */
type BooleanSettingKey = "sound_enabled" | "notify_enabled" | "always_on_top" | "snap_to_edge";

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
    // 开关按 data-pref 定位回写（PL028.3：开关增至四路，弃下标索引防增删错位）
    const boxes: Array<[BooleanSettingKey, boolean]> = [
      ["sound_enabled", props.settings.sound_enabled],
      ["notify_enabled", props.settings.notify_enabled],
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

/** 开关切换即时上抛（四路布尔字段各有后端字段；数值字段走专用守卫） */
function onSwitchChange(key: BooleanSettingKey, e: Event): void {
  const checked = (e.target as HTMLInputElement).checked;
  emit("save", { ...props.settings, [key]: checked });
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

/** 退出录制态并清计时器（完成 / Esc / 收板 / 卸载都走此口）。 */
function stopRecord(): void {
  recording.value = null;
  window.clearTimeout(recordTimer);
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
          <p class="setting-name">提醒声音</p>
          <p class="setting-desc">达到阈值播放提示音</p>
        </div>
        <label class="switch">
          <input
            type="checkbox"
            data-pref="sound_enabled"
            :checked="settings.sound_enabled"
            @change="onSwitchChange('sound_enabled', $event)"
          />
          <span class="slider"></span>
        </label>
      </div>
      <div class="setting-row inline">
        <div>
          <p class="setting-name">系统通知</p>
          <p class="setting-desc">达到阈值发送系统通知</p>
        </div>
        <label class="switch">
          <input
            type="checkbox"
            data-pref="notify_enabled"
            :checked="settings.notify_enabled"
            @change="onSwitchChange('notify_enabled', $event)"
          />
          <span class="slider"></span>
        </label>
      </div>
      <div class="setting-row inline">
        <div>
          <p class="setting-name">自动下班</p>
          <p class="setting-desc">达到时长自动下班</p>
        </div>
        <!-- 常开禁用：见文件头"登记差异" -->
        <label class="switch" title="自动下班常开">
          <input type="checkbox" checked disabled />
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
          @click="startRecord('toggle_visible')"
        >
          {{
            recording === "toggle_visible"
              ? "录制中…"
              : displayHotkey(settings.hotkey_toggle_visible)
          }}
        </button>
      </div>
      <div class="setting-row inline">
        <div>
          <p class="setting-name">工作/休息快捷键</p>
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
          @click="startRecord('toggle_timer')"
        >
          {{
            recording === "toggle_timer" ? "录制中…" : displayHotkey(settings.hotkey_toggle_timer)
          }}
        </button>
      </div>
      <p v-if="error" class="save-error" role="alert">{{ error }}</p>
    </div>
  </div>
</template>

<style scoped>
/* PL028.2 / PL028.3 新增四行后，九行在 300×400 板内需要更紧的行距（设计 6px → 4px，
   全板统一、观感几无差别；设计稿无此密度，功能性补口——登记于 z.plan 附录 PL028） */
#settings-board .setting-row {
  padding: 4px 0;
}
/* 同上：描述行收紧（8px 小字 + 行高 1.1 + 上边距 1px）——九行才装得下 */
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
   同面板数字输入框的 panel 配方，选中态描边用 accent） */
.hotkey-btn {
  min-width: 96px;
  padding: 6px 10px;
  border: none;
  border-radius: 9px;
  background: var(--panel-bg);
  box-shadow: var(--panel-stroke), var(--panel-shadow);
  color: var(--ink);
  font-family: inherit;
  font-size: 12px;
  cursor: pointer;
}
.hotkey-btn.recording {
  outline: 1.5px solid var(--accent);
}
.hotkey-error {
  margin: 2px 0 0;
  font-size: 10px;
  color: #b42318;
}
</style>
