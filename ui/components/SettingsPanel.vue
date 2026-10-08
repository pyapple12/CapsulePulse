<script setup lang="ts">
/** 设置板（design #settings-board 1:1 重装，PL019.2）：五行即时生效。
 * 常驻 DOM + .open 驱动显隐（boards.css visibility/scale 机制，v-if 会杀收板动画）；
 * 接线：阈值 change 夹取 / 两开关 change / 步进按钮，全部即时 emit save（设计无完成钮）。
 * 登记差异：第五行"自动下班"开关 demo 侧零接线（静态 checked）、后端自动下班常开且
 * core 零功能变更红线（z.plan 附录 PL016 明确不做）——渲染为常开禁用态，观感与设计一致
 * （boards.css 无 :disabled 样式），语义上不假装可切换。 */
import { ref, watch } from "vue";

import type { ReminderSettings } from "../types";

const props = defineProps<{
  /** 板开合（父持态，两板互斥）；.open 驱动 visibility + 自按钮中心 scale 飞出 */
  open: boolean;
  /** 当前生效设置（即时保存模型：每次变更即上抛，无草稿态） */
  settings: ReminderSettings;
  /** 保存失败文案（面板内可见反馈，FIX002.3 接线保留） */
  error?: string;
}>();

const emit = defineEmits<{ save: [s: ReminderSettings] }>();

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
    const boxes = root.querySelectorAll<HTMLInputElement>(".switch input");
    if (boxes[0] != null) {
      boxes[0].checked = props.settings.sound_enabled;
    }
    if (boxes[1] != null) {
      boxes[1].checked = props.settings.notify_enabled;
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

/** 开关切换即时上抛（仅声音/通知两路有后端字段） */
function onSwitchChange(key: "sound_enabled" | "notify_enabled", e: Event): void {
  const checked = (e.target as HTMLInputElement).checked;
  emit("save", { ...props.settings, [key]: checked });
}

/** 步进 ±1（4~12 边界外静默：禁用态已挡，此处为键盘兜底，design stepAutoOut 同款守卫） */
function stepAutoOut(delta: number): void {
  const v = props.settings.workday_auto_out_hours + delta;
  if (v < 4 || v > 12) {
    return;
  }
  emit("save", { ...props.settings, workday_auto_out_hours: v });
}
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
          <p class="setting-desc">上限 4~12 小时</p>
        </div>
        <div class="stepper">
          <button
            class="stepper-btn"
            aria-label="减少"
            type="button"
            :disabled="settings.workday_auto_out_hours <= 4"
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
            :disabled="settings.workday_auto_out_hours >= 12"
            @click="stepAutoOut(1)"
          >
            <svg viewBox="0 0 26 22" aria-hidden="true">
              <rect class="st-frame" x="0.5" y="0.5" width="25" height="21" rx="6" />
              <path class="st-glyph" d="M13 8 V14 M10 11 H16" />
            </svg>
          </button>
        </div>
      </div>
      <p v-if="error" class="save-error" role="alert">{{ error }}</p>
    </div>
  </div>
</template>

<style scoped>
/* 保存失败可见反馈（FIX002.3 接线保留；设计无此件，功能性补口非视觉裁定） */
.save-error {
  margin: 8px 0 0;
  font-size: 12px;
  color: #b42318;
}
</style>
