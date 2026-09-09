<script setup lang="ts">
import { ref } from "vue";

/** 提醒设置（镜像 Rust 侧 ReminderSettings serde 结构，单一来源在 Rust） */
interface ReminderSettings {
  threshold_min: number;
  sound_enabled: boolean;
  notify_enabled: boolean;
}

const props = defineProps<{ settings: ReminderSettings }>();
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
      <span>提示音</span>
      <input v-model="draft.sound_enabled" type="checkbox" />
    </label>
    <label class="row">
      <span>系统通知</span>
      <input v-model="draft.notify_enabled" type="checkbox" />
    </label>
    <button class="save" type="button" @click="onSave">保存</button>
  </section>
</template>

<style scoped>
.panel {
  display: flex;
  flex-direction: column;
  gap: 10px;
  width: 100%;
  padding: 12px;
  border: 1px solid rgba(128, 128, 128, 0.3);
  border-radius: 12px;
  background: rgba(128, 128, 128, 0.1);
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
  padding: 4px 6px;
  border: 1px solid rgba(128, 128, 128, 0.4);
  border-radius: 6px;
  background: transparent;
  color: inherit;
}

.save {
  align-self: center;
  padding: 6px 24px;
  border: 1px solid rgba(128, 128, 128, 0.4);
  border-radius: 8px;
  background: rgba(0, 122, 255, 0.75);
  border-color: transparent;
  color: #fff;
  font-size: 13px;
  cursor: pointer;
}

.save:hover {
  background: rgba(0, 122, 255, 0.9);
}
</style>
