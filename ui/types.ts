/**
 * IPC DTO 镜像类型（TS 侧单一收敛点）：与 Rust serde 结构逐字段对齐，
 * Rust 侧为契约单一来源——改结构时只改这里与对应 .rs，禁止在组件内重复声明。
 */

/** status 命令返回体（镜像 core/src/commands/session.rs SessionStatus） */
export interface SessionStatus {
  state: "idle" | "running" | "paused";
  total_ms: number;
  on_duty: boolean;
  /** 当前数据日偏移（PATCH005.4）：在岗 → 该班打卡日相对日历今日的天数偏移（0 / −1）；未在岗 → 0。
   * 时钟与统计据此锚定"进行中的数据日"（跨零点夜班没下班就不切换）。 */
  data_day_offset: number;
}

/** 加班硬顶小时数（镜像 core/src/workday.rs `OVERTIME_CAP_HOURS`，单一来源在后端）：
 * 今日上限封顶于此值——到顶后加班钮隐藏、上班钮置灰（PATCH005.3）。 */
export const OVERTIME_CAP_HOURS = 12;
/** PATCH006.4 电池分母：固定 12h = 100%（人一天能干的总量，与硬顶同源） */
export const BATTERY_TOTAL_HOURS = OVERTIME_CAP_HOURS;
/** PATCH006.4 电池红阈值：≤25%（= 相对 12h 只剩 3h，固定第 9h 起红） */
export const BATTERY_RED_RATIO = 0.25;
/** PATCH006.4 电池黄阈值下限：黄 = max(30%, 1 − 基准/12) ⇒ 基准 ≥9h 时固定 30% */
export const BATTERY_YELLOW_FLOOR = 0.3;

/** stats 命令返回体（镜像 core/src/commands/stats.rs SessionStats） */
export interface SessionStats {
  today_secs: number;
  week_secs: number;
  all_secs: number;
}

/** get/set_settings 命令参数与返回体（镜像 core/src/settings.rs ReminderSettings） */
export interface ReminderSettings {
  threshold_min: number;
  sound_enabled: boolean;
  notify_enabled: boolean;
  /** 自动下班时长（小时），与 Rust 侧 1–72 校验一致 */
  workday_auto_out_hours: number;
}

/** day_detail 返回体中的图谱区块（镜像 core/src/workday.rs DayBlock） */
export interface DayBlock {
  start: number;
  end: number;
  kind: "work" | "rest";
}

/** day_detail 命令返回体（镜像 core/src/workday.rs DaySummary） */
export interface DaySummary {
  duty_started_at: number | null;
  duty_ended_at: number | null;
  duty_secs: number;
  work_secs: number;
  rest_secs: number;
  blocks: DayBlock[];
  /** 今日上限小时数（PATCH005.3：基准 + 4h×加班步数、封顶 12h；过去日 0） */
  budget_hours: number;
  /** 今日已累计在岗秒数（PATCH005.3；过去日 0） */
  used_secs: number;
  /** 本班剩余预算秒数（PATCH005.3：表盘底轨跨度与"轴末"同一口径；暂下班 / 过去日 0） */
  remaining_secs: number;
}

/** week_detail 返回体中的单日行（镜像 core/src/commands/workday.rs WeekDay） */
export interface WeekDay {
  /** 日期标签（MM-DD） */
  date: string;
  /** 星期序（1 = 周一 … 7 = 周日） */
  weekday: number;
  /** 当日工作秒数 */
  work_secs: number;
  /** 当日在岗秒数 */
  duty_secs: number;
}

/** workday_total 返回体（镜像 core/src/commands/workday.rs WorkdayTotal）：全历史三值总和 + 有数据天数 */
export interface WorkdayTotal {
  /** 全历史在岗总秒数 */
  duty_secs: number;
  /** 全历史工作总秒数 */
  work_secs: number;
  /** 全历史休息总秒数 */
  rest_secs: number;
  /** 有数据天数（当日存在打卡/计时记录） */
  days: number;
  /** 最早记录所在本地日相对今天的偏移（负值或 0；空库 null）——统计往前翻的下界（PL025.1） */
  first_offset: number | null;
}

/** 崩溃恢复提示返回体（镜像 core/src/commands/workday.rs RecoveryOutcome，PL027.8）：
 * `closed_at` = 补下班记账时刻（仅"判下班"分支有值；无缝续为 null，前端静默） */
export interface RecoveryNotice {
  closed_at: number | null;
  /** 该次补下班是否以"自动下班"名义（越过自动下班点） */
  auto: boolean;
}
