/**
 * IPC DTO 镜像类型（TS 侧单一收敛点）：与 Rust serde 结构逐字段对齐，
 * Rust 侧为契约单一来源——改结构时只改这里与对应 .rs，禁止在组件内重复声明。
 */

/** status 命令返回体（镜像 core/src/commands/session.rs SessionStatus） */
export interface SessionStatus {
  state: "idle" | "running" | "paused";
  total_ms: number;
  on_duty: boolean;
}

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
}
