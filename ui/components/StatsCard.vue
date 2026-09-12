<script setup lang="ts">
/** 统计行组件（纯展示：三值秒数由父组件拉取传入，业务聚合在 Rust 侧） */
import { fmtDuration } from "../format";

defineProps<{
  todaySecs: number;
  weekSecs: number;
  allSecs: number;
}>();
</script>

<template>
  <p class="stats">
    <span>今日 {{ fmtDuration(todaySecs) }}</span>
    <span class="sep">｜</span>
    <span>本周 {{ fmtDuration(weekSecs) }}</span>
    <span class="sep">｜</span>
    <span>累计 {{ fmtDuration(allSecs) }}</span>
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
