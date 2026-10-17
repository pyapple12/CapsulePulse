/**
 * 子窗口量内容的共享管线（FIX007.12）：悬浮卡（PL028.1）与自绘右键菜单窗（PATCH008）此前各持
 * 一份**逐字重复**的几何数学（`snap4` + "量内容层尺寸 + 从 CSS 读 padding/border/margin 作 chrome"）
 * ⇒ 收敛到本模块：**取整规则与量法只有一处**，两个窗口只保留各自的窗口语义（见调用点注释）。
 *
 * 纯函数、零 IPC 依赖 ⇒ 可直接单测（`ui/tests/measure-window.spec.ts`）。
 */

/** 把目标尺寸**向上**取到 **4 的倍数**（PATCH007.24 用户定案）：125% 下 4 逻辑 = 5 物理 ⇒
 * **逻辑与物理同时为整数** ⇒ 系统取整与 WebView 视口取整不产生差（可视区 = 窗 = 卡片）；
 * 100% / 150% / 175% / 200% 下同样成立。**只向上**（只多不少 ⇒ 内容永不裁）。 */
export function snap4(value: number): number {
  return Math.ceil(value / 4) * 4;
}

/** 量到的一格几何：内容层尺寸 + 卡片 chrome（内边距 + 边框 + 外边距）。 */
export interface ChromeMeasure {
  contentW: number;
  contentH: number;
  chromeX: number;
  chromeY: number;
}

/**
 * 量**内容层** `body` 的尺寸 + 从 `card` 的计算样式读 chrome（**不手抄常量**：卡片加
 * padding / border / margin 后无需改代码，窗随样式走）。
 *
 * # 参数
 * - `body`：量尺寸的对象——**必须传内容层**（卡片是 `height: 100%` 铺满窗，量卡片会自反馈）
 * - `card`：读 chrome 的对象（卡片本身）
 *
 * # 返回
 * 任一元素缺失（未挂载 / 已卸载）返回 `null`；调用方据此跳过本轮（不设窗、不重锚）。
 */
export function measureChrome(
  body: HTMLElement | null,
  card: HTMLElement | null,
): ChromeMeasure | null {
  if (body == null || card == null) {
    return null;
  }
  const rect = body.getBoundingClientRect();
  const cs = getComputedStyle(card);
  const chromeX =
    parseFloat(cs.paddingLeft) +
    parseFloat(cs.paddingRight) +
    parseFloat(cs.borderLeftWidth) +
    parseFloat(cs.borderRightWidth) +
    parseFloat(cs.marginLeft) +
    parseFloat(cs.marginRight);
  const chromeY =
    parseFloat(cs.paddingTop) +
    parseFloat(cs.paddingBottom) +
    parseFloat(cs.borderTopWidth) +
    parseFloat(cs.borderBottomWidth) +
    parseFloat(cs.marginTop) +
    parseFloat(cs.marginBottom);
  return { contentW: rect.width, contentH: rect.height, chromeX, chromeY };
}
