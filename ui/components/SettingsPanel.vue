<script setup lang="ts">
import { ref } from "vue";

// IPC DTO 镜像类型统一在 types.ts（单一来源 = Rust serde 结构）
import type { ReminderSettings } from "../types";

const props = defineProps<{ settings: ReminderSettings; error?: string }>();
const emit = defineEmits<{ save: [s: ReminderSettings] }>();

// 面板打开时快照一份本地草稿，保存时才上抛（未保存修改不污染父态）
const draft = ref<ReminderSettings>({ ...props.settings });

function onSave(): void {
  emit("save", { ...draft.value });
}
</script>

<template>
  <section class="panel">
    <label class="row">
      <span>提醒阈值（分钟）</span>
      <input v-model.number="draft.threshold_min" type="number" min="1" max="240" step="1" />
    </label>
    <label class="row">
      <span>自动下班（小时）</span>
      <input
        v-model.number="draft.workday_auto_out_hours"
        type="number"
        min="1"
        max="72"
        step="1"
      />
    </label>
    <label class="row">
      <span>提示音</span>
      <input v-model="draft.sound_enabled" type="checkbox" />
    </label>
    <label class="row">
      <span>系统通知</span>
      <input v-model="draft.notify_enabled" type="checkbox" />
    </label>
    <button class="save" type="button" @click="onSave">保存</button>
    <p v-if="error" class="error" role="alert">{{ error }}</p>
  </section>
</template>

<style scoped>
/* 设置内容（PL006.5 浮层化）：自身去边框去底色，玻璃外壳由 App.vue 的 .floating-sheet 承担 */
.panel {
  display: flex;
  flex-direction: column;
  gap: 10px;
  width: 100%;
  box-sizing: border-box;
}

.row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  font-size: 13px;
}

.row input[type="number"] {
  width: 72px;
  padding: 4px 8px;
  border: 1px solid rgba(128, 128, 128, 0.35);
  border-radius: 8px;
  background: transparent;
  color: inherit;
  font-family: var(--font-stack);
}

.save {
  align-self: center;
  padding: 7px 28px;
  border: none;
  border-radius: var(--r-pill);
  background: var(--accent);
  color: #fff;
  font-family: var(--font-stack);
  font-size: 13px;
  cursor: pointer;
  transition:
    background 0.15s ease,
    transform 0.15s ease;
}

.save:hover {
  background: color-mix(in srgb, var(--accent) 88%, #000);
}

.save:active {
  transform: scale(0.96);
}

/* 保存失败提示：双主题可读的错误红 */
.error {
  margin: 0;
  font-size: 12px;
  color: #b3261e;
}

@media (prefers-color-scheme: dark) {
  .error {
    color: #ff8a80;
  }
}
</style>
