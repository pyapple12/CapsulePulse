/**
 * 展示格式化共享助手（纯函数、零 IPC 依赖）：时间与时长 → 界面文本。
 * 单一来源收敛跨组件重复（FIX002.13：hhmm/fmt/pad 曾在 4 处逐字复制）。
 */

/** 数字补零到两位 */
export function pad(n: number): string {
  return String(n).padStart(2, "0");
}

/** Unix 秒 → 本地 HH:MM（图谱轴标、段明细、自动下班文案条） */
export function hhmm(secs: number): string {
  const d = new Date(secs * 1000);
  return `${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

/** 分钟数 → 时长三段 `{h, m}`（**不做任何字符补位**）：数字列对齐由 CSS 承担 —— `.num`
 * `min-width: 2ch; text-align: right`（PATCH007.12 **方案 B**，用户 2026-10-16 定案：比字符补位
 * 更确定，不依赖字体是否有 U+2007 等宽空格 ✓）。 */
export function splitDuration(minutes: number): { h: number; m: number } {
  const total = Math.max(0, minutes);
  return { h: Math.floor(total / 60), m: Math.round(total) % 60 };
}
