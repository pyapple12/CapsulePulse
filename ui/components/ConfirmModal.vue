<script setup lang="ts">
/** 打卡双向确认框（design .overlay .sheet 1:1 重装，PL019.2）：display 直切零过渡（设计无动画）。
 * 结构与文案照抄 design openConfirm（h4 + p + .acts 取消/确认）；必须放置于 .window 内——
 * glass.css :has(.overlay) 内容自雾化依赖该祖先关系（WebView2 透明窗 backdrop-filter 不渲染，
 * CT 技法：被盖内容 filter blur + 板上黑纱由 glass.css 差异档承担）。
 * PL025.8：文案与附注可覆盖（自动下班确认框复用同形态：倒计时 + 下班/加班）。 */
// PL028.4：`cancelVisible` **必须给缺省值**——Vue 对 Boolean 型 prop 的缺省铸造是 `false`，
// 不传即 false ⇒ `v-if="cancelVisible !== false"` 恒假、取消按钮永不渲染（与本注释"缺省可见"相反）
withDefaults(
  defineProps<{
    open: boolean;
    title: string;
    message: string;
    /** 确认按钮文案（缺省「确认」）。 */
    okText?: string;
    /** 取消按钮文案（缺省「取消」）。 */
    cancelText?: string;
    /** 附注行（如倒计时；缺省不渲染）。 */
    note?: string;
    /** 取消按钮可见性（**缺省可见**；自动下班框在"加班无意义"时隐藏）。 */
    cancelVisible?: boolean;
    /** 标题居中（PATCH012.5：打卡框标题居中；缺省左对齐——自动下班框沿用）。 */
    titleCenter?: boolean;
  }>(),
  { cancelVisible: true },
);

const emit = defineEmits<{ confirm: []; cancel: [] }>();
</script>

<template>
  <div v-if="open" class="overlay" role="alertdialog" :aria-label="title">
    <div class="sheet">
      <h4 :class="{ center: titleCenter }">{{ title }}</h4>
      <p>{{ message }}</p>
      <p v-if="note">{{ note }}</p>
      <!-- PL028.4：按钮顺序按用户定案 = **确认在左、取消在右**（有意偏离 design 的"取消/确认"） -->
      <div class="acts">
        <button class="ok" type="button" @click="emit('confirm')">{{ okText ?? "确认" }}</button>
        <button v-if="cancelVisible !== false" type="button" @click="emit('cancel')">
          {{ cancelText ?? "取消" }}
        </button>
      </div>
    </div>
  </div>
</template>
