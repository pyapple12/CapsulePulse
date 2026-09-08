<script setup lang="ts">
import TimerCard from "./components/TimerCard.vue";
// 玻璃卡片 + 拖动区沿用 PL001 阶段 B 判定通过的形态（G1–G4）；计时与按钮逻辑在 TimerCard
</script>

<template>
  <main class="glass-card" data-tauri-drag-region>
    <h1 class="title" data-tauri-drag-region>CapsulePulse</h1>
    <TimerCard />
  </main>
</template>

<style scoped>
/* 玻璃卡片：四周留 12px 露出 Acrylic 底（G1 透明可见），卡片本体半透明 + backdrop-filter 叠加模糊（G2） */
.glass-card {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 16px;
  height: calc(100vh - 24px);
  margin: 12px;
  border-radius: 16px;
  background: rgba(255, 255, 255, 0.35);
  backdrop-filter: blur(24px);
  user-select: none;
  color: #1f2328;
}

.title {
  font-size: 22px;
  font-weight: 600;
  margin: 0;
  opacity: 0.75;
  cursor: default;
}

/* 深浅色跟随系统（G3）：双主题下文字与卡片底均保持可读 */
@media (prefers-color-scheme: dark) {
  .glass-card {
    background: rgba(30, 30, 30, 0.35);
    color: #e8e8e8;
  }
}
</style>
