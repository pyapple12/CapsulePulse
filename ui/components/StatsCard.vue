<script setup lang="ts">
/** 统计行组件（纯展示：三值秒数由父组件拉取传入，业务聚合在 Rust 侧） */
defineProps<{
  todaySecs: number;
  weekSecs: number;
  allSecs: number;
}>();

/** 秒数 → 简洁时长（Xh Ym；不足 1 小时只显分钟） */
function fmt(total: number): string {
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  return h > 0 ? `${h}h ${m}m` : `${m}m`;
}
</script>

<template>
  <p class="stats">
    <span>今日 {{ fmt(todaySecs) }}</span>
    <span class="sep">｜</span>
    <span>本周 {{ fmt(weekSecs) }}</span>
    <span class="sep">｜</span>
    <span>累计 {{ fmt(allSecs) }}</span>
  </p>
</template>

<style scoped>
/* 统计行（内容层）：纯文字无盒，ink-2 层级弱于主数字 */
.stats {
  display: flex;
  align-items: center;
  gap: 8px;
  color: var(--ink-2);
  font-size: 13px;
  margin: 0;
}

.sep {
  opacity: 0.5;
}
</style>
