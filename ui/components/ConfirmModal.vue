<script setup lang="ts">
/** 打卡双向确认框（design .overlay .sheet 1:1 重装，PL019.2）：display 直切零过渡（设计无动画）。
 * 结构与文案照抄 design openConfirm（h4 + p + .acts 取消/确认）；必须放置于 .window 内——
 * glass.css :has(.overlay) 内容自雾化依赖该祖先关系（WebView2 透明窗 backdrop-filter 不渲染，
 * CT 技法：被盖内容 filter blur + 板上黑纱由 glass.css 差异档承担） */
defineProps<{
  open: boolean;
  title: string;
  message: string;
}>();

const emit = defineEmits<{ confirm: []; cancel: [] }>();
</script>

<template>
  <div v-if="open" class="overlay" role="alertdialog" :aria-label="title">
    <div class="sheet">
      <h4>{{ title }}</h4>
      <p>{{ message }}</p>
      <div class="acts">
        <button type="button" @click="emit('cancel')">取消</button>
        <button class="ok" type="button" @click="emit('confirm')">确认</button>
      </div>
    </div>
  </div>
</template>
