<script setup lang="ts">
/** 工作日进度环（PL008.4，定稿线框·胶囊表盘）：糖果玻璃轨道 + 紫渐变进度弧 + 数字居环中。
 * 纯展示组件：进度 = 当日工作秒 ÷ (自动下班小时 × 3600)（workday 口径，数据由父组件下发），
 * 本组件零业务聚合；未上班 = 虚线置灰轨道不显弧（数字由插槽方配纯色回退） */
import { computed } from "vue";

const props = defineProps<{
  /** 是否在岗：未上班显虚线置灰轨道 */
  onDuty: boolean;
  /** 当日工作秒数（day_detail.work_secs，day_detail 刷新时更新） */
  workSecs: number;
  /** 自动下班小时数（settings 未就绪时为 null，环不显弧） */
  targetHours: number | null;
}>();

// 环几何（定稿线框）：外径 264、描边 12 → 半径 126；周长用于 dasharray 换算
const R = 126;
const CIRC = 2 * Math.PI * R;

/** 进度比例：工作秒 / (目标小时 × 3600)，上限 1（超时定格满环） */
const ratio = computed(() => {
  if (!props.onDuty || props.targetHours == null) {
    return 0;
  }
  return Math.min(props.workSecs / (props.targetHours * 3_600), 1);
});
</script>

<template>
  <div class="ring-wrap">
    <svg
      class="ring"
      viewBox="0 0 264 264"
      role="img"
      :aria-label="onDuty ? `今日工作进度 ${Math.round(ratio * 100)}%` : '未上班'"
    >
      <defs>
        <!-- 弧线渐变自 12 点顺时针（svg 整体旋转 -90°），色相经 accent 令牌派生（深浅色自动跟随） -->
        <linearGradient id="ring-grad" x1="0" y1="0" x2="0.32" y2="1">
          <stop offset="0" class="stop-a" />
          <stop offset="1" class="stop-b" />
        </linearGradient>
      </defs>
      <circle class="track" :class="{ off: !onDuty }" cx="132" cy="132" :r="R" />
      <circle
        v-if="onDuty"
        class="arc"
        cx="132"
        cy="132"
        :r="R"
        :stroke-dasharray="`${ratio * CIRC} ${CIRC}`"
      />
    </svg>
    <div class="ring-center">
      <slot />
    </div>
  </div>
</template>

<style scoped>
.ring-wrap {
  position: relative;
  width: 264px;
  height: 264px;
}

/* svg 整体旋转：dasharray 起点转到 12 点方向（环心数字在独立层，不受旋转影响） */
.ring {
  display: block;
  width: 100%;
  height: 100%;
  transform: rotate(-90deg);
}

/* 轨道 = 玻璃感中性环（ink 低透明度，双主题自适应）；未上班虚线置灰 */
.track {
  fill: none;
  stroke: color-mix(in srgb, var(--ink) 12%, transparent);
  stroke-width: 12;
}

.track.off {
  stroke-dasharray: 3 9;
  opacity: 0.55;
}

/* 进度弧：圆头端帽 + accent 派生渐变；dasharray 由父态驱动，变化带缓动 */
.arc {
  fill: none;
  stroke: url(#ring-grad);
  stroke-width: 12;
  stroke-linecap: round;
  transition: stroke-dasharray 0.3s ease;
}

.stop-a {
  stop-color: color-mix(in srgb, var(--accent) 55%, #fff);
}

.stop-b {
  stop-color: var(--accent);
}

.ring-center {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
}
</style>
