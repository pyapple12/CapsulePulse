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

/** 秒数 → 简洁时长（Xh Ym；不足 1 小时只显分钟） */
export function fmtDuration(total: number): string {
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  return h > 0 ? `${h}h ${m}m` : `${m}m`;
}
