<script setup lang="ts">
/** 玻璃确认框（PL005 打卡双向确认）：自定义 modal 替代原生 confirm，深浅色均适配玻璃卡片 */
defineProps<{
  open: boolean;
  title: string;
  message: string;
}>();

const emit = defineEmits<{ confirm: []; cancel: [] }>();
</script>

<template>
  <div v-if="open" class="overlay" @click.self="emit('cancel')">
    <section class="modal" role="alertdialog" :aria-label="title">
      <h2 class="modal-title">{{ title }}</h2>
      <p class="modal-message">{{ message }}</p>
      <div class="modal-actions">
        <button class="btn" type="button" @click="emit('cancel')">取消</button>
        <button class="btn primary" type="button" @click="emit('confirm')">确认</button>
      </div>
    </section>
  </div>
</template>

<style scoped>
.overlay {
  position: fixed;
  inset: 0;
  z-index: 20;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(0, 0, 0, 0.25);
  backdrop-filter: blur(6px);
}

/* 确认框本体：玻璃卡片风格，半透明底 + 模糊叠加 */
.modal {
  display: flex;
  flex-direction: column;
  gap: 14px;
  width: 240px;
  padding: 20px;
  border: 1px solid rgba(128, 128, 128, 0.35);
  border-radius: 16px;
  background: rgba(255, 255, 255, 0.55);
  backdrop-filter: blur(24px);
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.2);
  user-select: none;
  color: #1f2328;
}

.modal-title {
  margin: 0;
  font-size: 16px;
  font-weight: 600;
  text-align: center;
}

.modal-message {
  margin: 0;
  font-size: 13px;
  opacity: 0.8;
  text-align: center;
}

.modal-actions {
  display: flex;
  gap: 10px;
}

.modal-actions .btn {
  flex: 1;
  padding: 8px 0;
  border: 1px solid rgba(128, 128, 128, 0.4);
  border-radius: 10px;
  background: rgba(128, 128, 128, 0.12);
  color: inherit;
  font-size: 13px;
  cursor: pointer;
}

.btn.primary {
  border-color: transparent;
  background: rgba(0, 122, 255, 0.75);
  color: #fff;
}

.btn.primary:hover {
  background: rgba(0, 122, 255, 0.9);
}

@media (prefers-color-scheme: dark) {
  .modal {
    background: rgba(40, 40, 40, 0.55);
    color: #e8e8e8;
  }
}
</style>
