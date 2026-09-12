<script setup lang="ts">
/** 底部 dock 导航（PL008.3）：图标 + 文字双页切换，玻璃胶囊条定底。
 * 纯展示组件：页签状态由父组件持有（v-show 保活接线原样留在 App.vue），此处只发 change 事件 */
import { ChartColumn, Timer } from "lucide-vue-next";

defineProps<{ active: "timer" | "stats" }>();
const emit = defineEmits<{ change: [tab: "timer" | "stats"] }>();
</script>

<template>
  <nav class="dock" aria-label="页面导航">
    <button
      class="dock-item"
      :class="{ active: active === 'timer' }"
      type="button"
      role="tab"
      :aria-selected="active === 'timer'"
      @click="emit('change', 'timer')"
    >
      <Timer :size="18" :stroke-width="2.2" aria-hidden="true" />
      <span>计时</span>
    </button>
    <button
      class="dock-item"
      :class="{ active: active === 'stats' }"
      type="button"
      role="tab"
      :aria-selected="active === 'stats'"
      @click="emit('change', 'stats')"
    >
      <ChartColumn :size="18" :stroke-width="2.2" aria-hidden="true" />
      <span>统计</span>
    </button>
  </nav>
</template>

<style scoped>
/* dock 本体：玻璃胶囊条（chip 底 + rim 光 + 糖果投影），宽度随卡片、定底由 margin-top:auto 承担 */
.dock {
  display: grid;
  grid-template-columns: 1fr 1fr;
  width: calc(100% - 24px);
  margin-top: auto;
  padding: 5px;
  border-radius: var(--r-pill);
  background: var(--chip-bg);
  box-shadow: var(--rim-light), var(--glass-highlight), var(--shadow-candy);
}

/* 页签项：图标 + 文字纵排；激活 = 紫渐变胶囊 + 白 icon + 彩色边缘泛光 */
.dock-item {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 3px;
  padding: 6px 0 7px;
  border: none;
  border-radius: var(--r-pill);
  background: transparent;
  color: var(--ink-2);
  font-size: 11px;
  font-weight: 600;
  cursor: pointer;
  transition:
    background 0.2s var(--ease-spring),
    color 0.2s ease,
    transform 0.15s ease;
}

.dock-item:active {
  transform: scale(0.96);
}

.dock-item.active {
  background: var(--grad-primary);
  box-shadow: var(--rim-light), var(--edge-glow);
  color: #fff;
}
</style>
