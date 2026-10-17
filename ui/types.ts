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
  /** 本段连续工作毫秒（PATCH010.6）：`segment_secs` 语义——**仅 Running 非零**，Paused/Idle 恒 0。
   * 沙漏水位与设置板阈值下限的**单一事实源**。 */
  segment_ms: number;
  /** 本段是否已触发过提醒（PATCH010.6）：设置板据此**锁定阈值行**（铃响后只有暂停才能改）。 */
  reminder_fired: boolean;
}

/** 加班硬顶小时数（镜像 core/src/workday.rs `OVERTIME_CAP_HOURS`，单一来源在后端）：
 * 今日上限封顶于此值——到顶后加班钮隐藏、上班钮置灰（PATCH005.3）。 */
export const OVERTIME_CAP_HOURS = 12;
/** 自动下班小时数**可配下限**（镜像 `core/src/workday.rs` `AUTO_OUT_MIN_HOURS`，单一来源在后端；
 * 与 `OVERTIME_CAP_HOURS` 一起构成设置板步进器的区间；FIX006.16 补齐镜像） */
export const AUTO_OUT_MIN_HOURS = 4;
/** 提醒阈值**下限 / 上限**（分钟，镜像 `core/src/settings.rs` 的 `THRESHOLD_RANGE_MIN` /
 * `THRESHOLD_RANGE_MAX`，单一来源在后端）：阈值输入框的 `min`/`max`、夹取与步进边界
 * 一律引本常量，勿再写裸字面量（FIX008.6 补齐镜像）。 */
export const THRESHOLD_RANGE_MIN = 1;
export const THRESHOLD_RANGE_MAX = 99;
/** 提醒阈值**默认值**（分钟，镜像 `THRESHOLD_DEFAULT`）：输入空/0/NaN 时的回落值。 */
export const THRESHOLD_DEFAULT = 50;
/** PATCH006.4 电池分母：固定 12h = 100%（人一天能干的总量，与硬顶同源） */
export const BATTERY_TOTAL_HOURS = OVERTIME_CAP_HOURS;
/** PATCH006.4 电池红阈值：≤25%（= 相对 12h 只剩 3h，固定第 9h 起红） */
export const BATTERY_RED_RATIO = 0.25;
/** PATCH006.4 电池黄阈值下限：黄 = max(30%, 1 − 基准/12) ⇒ 基准 ≥9h 时固定 30% */
export const BATTERY_YELLOW_FLOOR = 0.3;

/** get/set_settings 命令参数与返回体（镜像 core/src/settings.rs ReminderSettings） */
export interface ReminderSettings {
  threshold_min: number;
  sound_enabled: boolean;
  notify_enabled: boolean;
  /** 自动下班时长（小时），与 Rust 侧 **4–12** 校验一致 */
  workday_auto_out_hours: number;
  /** 窗口置顶（PL028.3；缺省关）：窗口恒在最前 */
  always_on_top: boolean;
  /** 贴边吸附（PL028.3；缺省开）：拖动松手贴边停驻 */
  snap_to_edge: boolean;
  /** 窗口显隐切换全局热键（PL028.2；缺省 Alt+Shift+S）——`Super` 表 Win 键 */
  hotkey_toggle_visible: string;
  /** 工作/休息切换全局热键（PL028.2；缺省 Alt+Shift+P） */
  hotkey_toggle_timer: string;
  /** 窗口位置记忆（PL028.1；FIX006.17 补齐镜像）：后台每次隐藏/退出时写入，前端不消费；
   * 保存设置时后端**从内存态继承**该字段（`set_settings_inner`），故前端不传也不会丢 */
  window_pos: { x: number; y: number } | null;
}

/** 全局热键动作标识（镜像 core/src/commands/reminder.rs HotkeyAction，PL028.2）：
 * 两条动作共用一条写命令 `set_hotkey`。 */
export type HotkeyAction = "toggle_visible" | "toggle_timer";

/** prefs-changed 事件载荷（镜像 core/src/commands/reminder.rs PrefsSnapshot，PL028.3）：
 * 设置面板与托盘右键菜单双入口的开关态同步——任一路径改动即广播，两端回显同一事实源。 */
export interface PrefsSnapshot {
  always_on_top: boolean;
  snap_to_edge: boolean;
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
  /** 当前班起点（Unix 秒；FIX006.4 后端派生，未在岗 null）——表盘底轨与电池共用 */
  shift_start: number | null;
  /** 今日可加班（FIX006.4 后端派生：在岗 && 未到 12h 硬顶 && 未跨日） */
  can_overtime: boolean;
  /** 今日已到 12h 硬顶（FIX006.4 后端派生） */
  cap_reached: boolean;
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

/** 托盘 hover 概览（镜像 core/src/hover.rs HoverSummary，PL028.1）：`single_line` = 未在岗
 * （悬浮窗只一行高）；否则三行——行①标签（互斥）+ 剩余、行②工作/休息、行③离提醒分钟数。 */
export interface HoverSummary {
  state: "off" | "on_duty" | "paused";
  label: string;
  remaining_secs: number | null;
  work_secs: number;
  rest_secs: number;
  until_reminder_min: number;
  single_line: boolean;
}

/** 崩溃恢复提示返回体（镜像 core/src/commands/workday.rs RecoveryOutcome，PL027.8）：
 * `closed_at` = 补下班记账时刻（仅"判下班"分支有值；无缝续为 null，前端静默） */
export interface RecoveryNotice {
  closed_at: number | null;
  /** 该次补下班是否以"自动下班"名义（越过自动下班点）——**镜像完整性字段，前端当前未消费**
   * （FIX006.15f：按本文件"与 Rust serde 逐字段对齐"约定保留，勿删） */
  auto: boolean;
  /** 无缝续时的读数（= 断开前累计 + 停机时长；null = 未续）——FIX006.17 补齐的镜像字段 */
  resumed_secs: number | null;
}
