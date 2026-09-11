<script setup lang="ts">
/** 玻璃确认框（PL005 打卡双向确认）：自定义 modal 替代原生 confirm。
 * 浮层形态（overlay + floating-sheet）与开合动效复用 App.vue 全局样式（PL006） */
defineProps<{
  open: boolean;
  title: string;
  message: string;
}>();

const emit = defineEmits<{ confirm: []; cancel: [] }>();
</script>

<template>
  <Transition name="sheet">
    <div v-if="open" class="overlay" @click.self="emit('cancel')">
      <section class="floating-sheet" role="alertdialog" :aria-label="title">
        <h2 class="sheet-title">{{ title }}</h2>
        <p class="sheet-message">{{ message }}</p>
        <div class="sheet-actions">
          <button class="btn" type="button" @click="emit('cancel')">取消</button>
          <button class="btn primary" type="button" @click="emit('confirm')">确认</button>
        </div>
      </section>
    </div>
  </Transition>
</template>

<style scoped>
.sheet-title {
  margin: 0;
  font-size: 16px;
  font-weight: 600;
  text-align: center;
}

.sheet-message {
  margin: 0;
  color: var(--ink-2);
  font-size: 13px;
  text-align: center;
}

.sheet-actions {
  display: flex;
  gap: 10px;
}

.sheet-actions .btn {
  flex: 1;
  padding: 8px 0;
  border: 1px solid rgba(128, 128, 128, 0.35);
  border-radius: var(--r-pill);
  background: rgba(128, 128, 128, 0.12);
  color: inherit;
  font-family: var(--font-stack);
  font-size: 13px;
  cursor: pointer;
  transition:
    background 0.15s ease,
    transform 0.15s ease;
}

.sheet-actions .btn:hover {
  background: rgba(128, 128, 128, 0.22);
}

.sheet-actions .btn:active {
  transform: scale(0.96);
}

.btn.primary {
  border-color: transparent;
  background: var(--accent);
  color: #fff;
}

.btn.primary:hover {
  background: color-mix(in srgb, var(--accent) 88%, #000);
}
</style>
