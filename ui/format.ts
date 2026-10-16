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
  // FIX006.1：**先整体取整成整数分钟**，再由它派生时与分——否则"分钟四舍五入到 60"的进位会被丢掉
  // （旧写法 `floor(total/60)` + `round(total)%60` 对同一小数用了两种取整），每个整点前 30 秒
  // 会显示成 (h−1)h00m（如 59.6 分 → `0h00m`，应为 `1h00m`；119.6 分 → `1h00m`，应为 `2h00m`）。
  const total = Math.max(0, Math.round(minutes));
  return { h: Math.floor(total / 60), m: total % 60 };
}
