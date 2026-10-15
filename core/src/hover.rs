//! 托盘 hover 今日概览（PL028.1）：把「今日明细 + 本段连续工作 + 提醒触发状态 + 设置基准」
//! 归约成结构化 DTO。纯逻辑零依赖（禁 import tauri，`cargo test` 直测）——命令层只负责取数。
//! 前端按 [`HoverSummary::single_line`] 渲染 1 行（未在岗）或 3 行（在岗 / 暂下班），
//! 窗口高度由后端按同一判据设定（两端同源）。

use std::time::Duration;

use serde::Serialize;

use crate::reminder::ReminderConfig;
use crate::workday::DaySummary;

/// 悬浮窗宽度（逻辑 px，**固定**）：用户 2026-10-16 定案 **226**（225 → 224 试窄后内容换行 ✗ ⇒ 定 226）。
/// 内容实测（该行）≈ **202.5**（「工作 0小时25分钟　休息 0小时 0分钟」）⇒ 可用 = 226 − 22 = **204** ✓ 不换行 ✓。
/// ⚠️ 226 × 1.25 = **282.5 物理（非整数）** ⇒ 系统取整后等效 225.6/226.4 ⇒ 右缘 1px 仍可能半像素吸附
/// （用户已知并接受，先按 226 落 ✓）。
pub const WINDOW_WIDTH: f64 = 228.0;
/// 悬浮窗内容高度下限（逻辑 px；创建期初始高用 ✓）
pub const HEIGHT_MIN: f64 = 24.0;

/// hover 概览状态（决定行数与行①文案）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HoverState {
    /// 今天无打卡（含跨零点夜班结束后的全新数据日）⇒ **只一行**「未在岗」
    Off,
    /// 在岗中（末班未闭合）⇒ 三行
    OnDuty,
    /// 暂下班（当天有打卡但已收段）⇒ 三行
    Paused,
}

/// 托盘 hover 今日概览 DTO（PL028.1）。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HoverSummary {
    /// 状态（前端据此决定渲染分支）
    pub state: HoverState,
    /// 行①标签：`未在岗` / `正班` / `1st 加班` / `2nd 加班` / `暂下班`（**互斥**，无中间值）
    pub label: String,
    /// 行①剩余秒数（在岗 = 本班到点 − now；其余 `None` ⇒ 不渲染该值）
    pub remaining_secs: Option<i64>,
    /// 行②今日工作秒数
    pub work_secs: i64,
    /// 行②今日休息秒数
    pub rest_secs: i64,
    /// 行③距下次休息提醒的分钟数（1 位小数；暂下班 / 未在岗 = 阈值本身，冻结）
    pub until_reminder_min: f64,
    /// 是否只渲染一行（= 未在岗）
    pub single_line: bool,
}

/// 归约 hover 概览（PL028.1；行①段带口径见 PATCH007.7 修正版）。
///
/// # 参数
/// `day` 今日明细（数据日 = 打卡日，PL027 单一口径；**行②与状态判定**用）；`bands` 行①段带输入
/// （[`ShiftBands`]；`None` = 未在岗 / 暂下班）；`now_secs` 当下 Unix 秒；`segment` 本段连续工作
/// （Paused/Idle 为零）；`config` 提醒配置；`last_fired` 段内上次提醒触发点（未触发为 `None`）。
///
/// # 返回
/// [`HoverSummary`]——`state` 三分支：无打卡 ⇒ 一行；有打卡且已收段 ⇒ 三行「暂下班」；
/// 末班未闭合 ⇒ 三行（行①按**本班已过时长**落带：`本班` / `1st 加班` / `2nd 加班`）。
pub fn hover_summary(
    day: &DaySummary,
    bands: Option<ShiftBands>,
    now_secs: i64,
    segment: Duration,
    config: &ReminderConfig,
    last_fired: Option<Duration>,
) -> HoverSummary {
    let threshold_min = config.threshold.as_secs_f64() / 60.0;
    let Some(duty_started_at) = day.duty_started_at else {
        // 今天无打卡：只一行「未在岗」，行②③不渲染（行③取值仅占位）
        return HoverSummary {
            state: HoverState::Off,
            label: "未在岗".to_string(),
            remaining_secs: None,
            work_secs: day.work_secs,
            rest_secs: day.rest_secs,
            until_reminder_min: threshold_min,
            single_line: true,
        };
    };
    let paused = day.duty_ended_at.is_some();
    let (label, remaining_secs) = if paused {
        // 暂下班：行①固定文案、行③冻结在阈值
        ("暂下班".to_string(), None)
    } else {
        // 在岗：行① = 本班已过时长落在哪一段带（PATCH007.7 修正版）——**锚点 = 本班带长
        // `d0 = 基准 − 本班之前已用`**（A2.8：7h50m 后再打卡 ⇒ 到点 = 剩 10 分钟处）；
        // 剩余取**自动下班到点**（与主表盘轴末/蓝条同一事实源）——到点恒等于当前带的带末
        // （加班只在到点弹框里点、点一次推一带），故两者等价、取前者即天然单一口径。
        let b = bands.unwrap_or(ShiftBands {
            clock_in_at: duty_started_at,
            d0_secs: i64::from(crate::workday::OVERTIME_CAP_HOURS) * 3_600,
            base_hours: crate::workday::OVERTIME_CAP_HOURS,
            due_at: None,
        });
        let (band, band_end_offset) = shift_band(now_secs - b.clock_in_at, b.d0_secs, b.base_hours);
        let label = match band {
            Band::Base => "本班",
            Band::First => "1st 加班",
            Band::Second => "2nd 加班",
        };
        let remaining = b
            .due_at
            .map(|due| due - now_secs)
            .unwrap_or_else(|| b.clock_in_at + band_end_offset - now_secs);
        (label.to_string(), Some(remaining.max(0)))
    };
    let until = if paused {
        config.threshold
    } else {
        until_next_reminder(segment, config, last_fired)
    };
    HoverSummary {
        state: if paused {
            HoverState::Paused
        } else {
            HoverState::OnDuty
        },
        label,
        remaining_secs,
        work_secs: day.work_secs,
        rest_secs: day.rest_secs,
        until_reminder_min: until.as_secs_f64() / 60.0,
        single_line: false,
    }
}

/// 距下次休息提醒的剩余时长（PL028.1）：未触发过 = 阈值 − 本段连续工作；
/// 已触发 = 重发间隔 −（本段 − 上次触发点）；两者下溢均夹到 0（时钟/状态竞态不产生负值）。
fn until_next_reminder(
    segment: Duration,
    config: &ReminderConfig,
    last_fired: Option<Duration>,
) -> Duration {
    match last_fired {
        None => config.threshold.saturating_sub(segment),
        Some(last) => config.repeat.saturating_sub(segment.saturating_sub(last)),
    }
}

/// 悬停停留阈值（ms）：图标上持续停留满此值才出卡（瞬间划过不打扰）。
pub const DWELL_MS: u64 = 500;
/// 守候步进周期（ms）：守候线程节拍与离开判据共用（单一来源）。
pub const WATCH_STEP_MS: u64 = 100;
/// 离开判据拍数：离开两区域连续满此拍数才收起（`WATCH_STEP_MS` × 本值 = 400ms）。
pub const MISS_LIMIT: u32 = 4;
/// 悬停期重推节拍（拍）：`WATCH_STEP_MS` × 本值 = 1 秒（PATCH007.6 用户定案）。
pub const PUSH_EVERY_TICKS: u32 = 10;

/// 守候相位（PATCH007.2 抄 CT `tray.rs`；PATCH008.6 补齐 `Suppressed`）：`Idle` 无事；
/// `Armed` 已武装（`Enter` 已触发，图标上持续停留满 [`DWELL_MS`] 才显示）；`Shown` 显示中
/// （离开两区域连续 [`MISS_LIMIT`] 拍才收起）；`Suppressed` **菜单窗开着 ⇒ 锁死**（照 CT：
/// 菜单收起后回 `Idle`，**需新 Enter 重新武装**，不自动弹出）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Phase {
    /// 无事（未武装、未显示）
    Idle,
    /// 已武装：`since_ms` = 武装时刻（Unix 毫秒），停留满阈值才显示
    Armed {
        /// 武装时刻（Unix 毫秒）
        since_ms: u64,
    },
    /// 显示中：`miss` = 连续离开两区域的采样步数（满 [`MISS_LIMIT`] 拍收起）
    Shown {
        /// 连续离开两区域的采样步数
        miss: u32,
    },
    /// 菜单窗开着：锁死（PATCH008.6，照 CT `Suppressed`）——菜单收起后回 `Idle`
    Suppressed,
}

/// 守候单步的窗口动作（悬浮窗 `show`/`hide` 的唯一调用者 = 守候线程）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatchAction {
    /// 无动作
    None,
    /// 显示悬浮窗
    Show,
    /// 收起悬浮窗
    Hide,
}

/// 守候状态机单步（**纯函数**，单测直测；照 CT `tray.rs::step_watch` 形态，PATCH008.6 补齐
/// `Suppressed` 态）。
///
/// # 参数
/// `phase` 当前相位；`now_ms` 当下 Unix 毫秒；`in_tray` 光标是否在托盘图标矩形内；
/// `in_window` 光标是否在悬浮窗矩形内；`menu_open` 菜单窗此刻是否可见（锁死事实源）；
/// `window_visible` 悬浮窗此刻是否可见。
///
/// # 返回
/// 新相位 + 本拍窗口动作。规则：`Idle` 不动；`Armed` 遇**菜单开即锁死**、遇残留可见窗先收
/// （新周期从隐藏态计时）、离开图标即取消、停留满 [`DWELL_MS`] 转 `Shown` 并显示；`Shown` 在
/// 图标或窗内即清零 `miss`（守住）、**菜单开则收窗并锁死**，连续离开满 [`MISS_LIMIT`] 拍转
/// `Idle` 并收起；`Suppressed` 菜单收 ⇒ 回 `Idle`（需新 `Enter` 重新武装），**窗可见一律收走**
/// （兜底 show 后到 / 残留两形态）。
pub fn step_watch(
    phase: Phase,
    now_ms: u64,
    in_tray: bool,
    in_window: bool,
    menu_open: bool,
    window_visible: bool,
) -> (Phase, WatchAction) {
    match phase {
        Phase::Idle => (Phase::Idle, WatchAction::None),
        Phase::Armed { since_ms } => {
            if menu_open {
                return (Phase::Suppressed, WatchAction::None); // 武装期菜单开：锁死
            }
            if window_visible {
                return (Phase::Armed { since_ms }, WatchAction::Hide);
            }
            if !in_tray {
                return (Phase::Idle, WatchAction::None); // 停留期内离开：取消出现
            }
            if now_ms.saturating_sub(since_ms) >= DWELL_MS {
                return (Phase::Shown { miss: 0 }, WatchAction::Show);
            }
            (Phase::Armed { since_ms }, WatchAction::None)
        }
        Phase::Shown { miss } => {
            if menu_open {
                return (Phase::Suppressed, WatchAction::Hide); // 显示中菜单开：收窗锁死
            }
            if in_tray || in_window {
                return (Phase::Shown { miss: 0 }, WatchAction::None); // 两区域任一在内：守住
            }
            let miss = miss + 1;
            if miss >= MISS_LIMIT {
                (Phase::Idle, WatchAction::Hide)
            } else {
                (Phase::Shown { miss }, WatchAction::None)
            }
        }
        Phase::Suppressed => {
            let next = if menu_open { phase } else { Phase::Idle };
            if window_visible {
                return (next, WatchAction::Hide); // 锁死期窗可见一律收走（兜底）
            }
            (next, WatchAction::None)
        }
    }
}

/// 行①段带（PATCH007.7）：按**当前班已过时长**落在哪一段带。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Band {
    /// 本班 `[0, 基准)`
    Base,
    /// 1st 加班 `[基准, 基准 + 4h)`
    First,
    /// 2nd 加班 `[基准 + 4h, 12h)`（仅当基准 < 8h 时该带非空）
    Second,
}

/// 行①段带输入（PATCH007.7 修正版）：本班打卡 + **本班带长 d0** + 基准 + 自动下班到点。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShiftBands {
    /// 本班打卡（当前班，Unix 秒）
    pub clock_in_at: i64,
    /// 本班带长 `d0 = 基准 − 今日在本班之前的已用`（秒；到顶/已用≥基准时为 0）
    pub d0_secs: i64,
    /// 自动下班基准小时数（算总账可用 = 12h 硬顶扣掉本班之前已用）
    pub base_hours: u32,
    /// 自动下班到点（`auto_out.due_at`；未武装为 `None` ⇒ 剩余兜底用带末）
    pub due_at: Option<i64>,
}

/// 段带判定与带末偏移（PATCH007.7 修正版；纯函数，单测直测）。
/// **锚点 = 本班带长 `d0 = 基准 − 今日在本班之前的已用`**（不是原始基准 ✗ —— A2.8 实测：
/// 做到 7h50m 再打卡 ⇒ 到点 = 剩 10 分钟处；A2.7 实测：已用 8h + 隐式加班 ⇒ 到点 = 打卡 + 4h）。
/// 带末（相对本班打卡）：本班 `d0` / 1st `min(d0 + 4h, 总账可用)` / 2nd `总账可用`；
/// **总账可用 = d0 + (12h − 基准)**（= 12h 硬顶扣掉本班之前已用 ⇒ 基准 8h + 已用 0 时 2nd 带为空）。
///
/// # 参数
/// `elapsed_secs` 本班已过秒数；`d0_secs` 本班带长；`base_hours` 自动下班基准小时数。
///
/// # 返回
/// `(段带, 带末相对本班打卡的偏移秒数)`。
pub fn shift_band(elapsed_secs: i64, d0_secs: i64, base_hours: u32) -> (Band, i64) {
    let step = i64::from(crate::workday::OVERTIME_STEP_HOURS) * 3_600;
    let cap_offset =
        i64::from(crate::workday::OVERTIME_CAP_HOURS) * 3_600 - i64::from(base_hours) * 3_600;
    let total = d0_secs + cap_offset; // 总账可用（相对本班打卡）
    let first_end = (d0_secs + step).min(total);
    if elapsed_secs < d0_secs {
        (Band::Base, d0_secs)
    } else if elapsed_secs < first_end {
        (Band::First, first_end)
    } else {
        (Band::Second, total)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workday::DaySummary;

    fn day(
        duty_started_at: Option<i64>,
        duty_ended_at: Option<i64>,
        work_secs: i64,
        rest_secs: i64,
    ) -> DaySummary {
        DaySummary {
            duty_started_at,
            duty_ended_at,
            duty_secs: work_secs + rest_secs,
            work_secs,
            rest_secs,
            blocks: Vec::new(),
            budget_hours: 0,
            used_secs: work_secs + rest_secs,
            remaining_secs: 0,
        }
    }

    fn mins(n: u64) -> Duration {
        Duration::from_secs(n * 60)
    }

    fn config() -> ReminderConfig {
        ReminderConfig::from_minutes(50)
    }

    fn hours(n: i64) -> i64 {
        n * 3_600
    }

    /// 段带边界（已用 0 ⇒ `d0` = 基准 = 6h）：本班 `[0,6h)` / 1st `[6h,10h)` / 2nd `[10h,12h)`，
    /// 带末分别 6h / 10h / 12h（与用户例一致）。
    #[test]
    fn shift_band_boundaries_for_base_six() {
        let d0 = hours(6);
        assert_eq!(shift_band(hours(5) + 3_599, d0, 6), (Band::Base, hours(6)));
        assert_eq!(shift_band(hours(6), d0, 6), (Band::First, hours(10)));
        assert_eq!(shift_band(hours(10) - 1, d0, 6), (Band::First, hours(10)));
        assert_eq!(shift_band(hours(10), d0, 6), (Band::Second, hours(12)));
    }

    /// 基准 8h（已用 0）：1st 带末即 12h 硬顶（2nd 带为空）；基准 12h：只有本班。
    #[test]
    fn shift_band_base_eight_and_twelve() {
        assert_eq!(shift_band(hours(8), hours(8), 8), (Band::First, hours(12)));
        assert_eq!(
            shift_band(hours(9), hours(12), 12),
            (Band::Base, hours(12)),
            "基准 12h 无处加班"
        );
    }

    /// **红线**：带锚点 = 本班带长 `d0 = 基准 − 本班之前已用`（PATCH007.7 修正版）。
    /// 用户实机场景：基准 8h、今日在本班之前已用 **7h57m** ⇒ `d0 = 3 分钟`；本班已走 **35 分钟**
    /// ⇒ 应显示 **1st 加班**、剩余 = 到点（打卡 + 3min + 4h）− now（错法锚在原始 8h ⇒ 误显
    /// 「本班 剩余 7h25m」✗ —— A2.8 实测口径即为反例）。
    #[test]
    fn band_anchors_on_d0_after_prior_usage() {
        let clock_in = 1_000;
        let now = clock_in + 35 * 60;
        let d0 = hours(8) - (7 * 3_600 + 57 * 60); // 3 分钟
        let s = hover_summary(
            &day(Some(clock_in), None, 3_600, 0),
            Some(ShiftBands {
                clock_in_at: clock_in,
                d0_secs: d0,
                base_hours: 8,
                due_at: Some(clock_in + d0 + hours(4)),
            }),
            now,
            Duration::ZERO,
            &config(),
            None,
        );
        assert_eq!(s.label, "1st 加班", "已用 7h57m ⇒ 本班带只剩 3 分钟");
        assert_eq!(s.remaining_secs, Some(hours(4) + d0 - 35 * 60));
    }

    /// 无到点时剩余兜底用**带末**（已用 0、本班 35 分钟 ⇒ 仍本班、剩余 = 8h − 35min）。
    #[test]
    fn band_remaining_falls_back_to_band_end() {
        let clock_in = 1_000;
        let s = hover_summary(
            &day(Some(clock_in), None, 0, 0),
            Some(ShiftBands {
                clock_in_at: clock_in,
                d0_secs: hours(8),
                base_hours: 8,
                due_at: None,
            }),
            clock_in + 35 * 60,
            Duration::ZERO,
            &config(),
            None,
        );
        assert_eq!(s.label, "本班");
        assert_eq!(
            s.remaining_secs,
            Some(hours(8) - 35 * 60),
            "无到点时兜底用带末"
        );
    }

    /// 未上班（今天无打卡）：**只一行**「未在岗」，无剩余值。
    #[test]
    fn off_state_is_single_line() {
        let s = hover_summary(
            &day(None, None, 0, 0),
            None,
            10_000,
            Duration::ZERO,
            &config(),
            None,
        );
        assert_eq!(s.state, HoverState::Off);
        assert_eq!(s.label, "未在岗");
        assert_eq!(s.remaining_secs, None);
        assert!(s.single_line, "未上班只渲染一行");
    }

    /// 在岗：三行、行② 工作/休息 照给、行③ 按阈值倒计（未触发过）。
    #[test]
    fn on_duty_rows_and_countdown() {
        let clock_in = 1_000;
        let s = hover_summary(
            &day(Some(clock_in), None, 3_600, 600),
            Some(ShiftBands {
                clock_in_at: clock_in,
                d0_secs: hours(8),
                base_hours: 8,
                due_at: None,
            }),
            clock_in + 30 * 60,
            mins(30),
            &config(),
            None,
        );
        assert_eq!(s.state, HoverState::OnDuty);
        assert!(!s.single_line);
        assert_eq!(s.work_secs, 3_600);
        assert_eq!(s.rest_secs, 600);
        assert!(
            (s.until_reminder_min - 20.0).abs() < 1e-9,
            "未触发过按阈值倒计（50 − 30）"
        );
    }

    /// 到点已过（竞态）夹到 0，不出现负剩余。
    #[test]
    fn remaining_never_negative() {
        let s = hover_summary(
            &day(Some(1_000), None, 0, 0),
            Some(ShiftBands {
                clock_in_at: 1_000,
                d0_secs: hours(8),
                base_hours: 8,
                due_at: None,
            }),
            99_999,
            Duration::ZERO,
            &config(),
            None,
        );
        assert_eq!(s.remaining_secs, Some(0));
    }

    /// 暂下班：三行「暂下班」，无剩余值，行③**冻结**在阈值（不随本段走）。
    #[test]
    fn paused_state_freezes_reminder() {
        let s = hover_summary(
            &day(Some(1_000), Some(9_000), 7_200, 1_800),
            None,
            20_000,
            mins(40),
            &config(),
            Some(mins(50)),
        );
        assert_eq!(s.state, HoverState::Paused);
        assert_eq!(s.label, "暂下班");
        assert_eq!(s.remaining_secs, None);
        assert!(!s.single_line);
        assert_eq!(s.work_secs, 7_200);
        assert_eq!(s.rest_secs, 1_800);
        assert!((s.until_reminder_min - 50.0).abs() < 1e-9, "行③冻结在阈值");
    }

    /// 行③已触发后按 5 分钟重发节拍倒数（1 位小数由前端格式化）。
    #[test]
    fn reminder_countdown_after_fire() {
        let clock_in = 1_000;
        let s = hover_summary(
            &day(Some(clock_in), None, 0, 0),
            Some(ShiftBands {
                clock_in_at: clock_in,
                d0_secs: hours(8),
                base_hours: 8,
                due_at: None,
            }),
            clock_in + 60,
            mins(52),
            &config(),
            Some(mins(50)),
        );
        // 已触发：5 分钟间隔 − 距上次触发 2 分钟 = 3 分钟
        assert!((s.until_reminder_min - 3.0).abs() < 1e-9);
    }

    /// 空闲期：任何输入都不动作（未武装 = 启动后未 hover 过）。
    #[test]
    fn idle_ignores_all_input() {
        assert_eq!(
            step_watch(Phase::Idle, 1_000, true, true, false, true),
            (Phase::Idle, WatchAction::None)
        );
    }

    /// 武装期未满停留：继续守候（不显示）。
    #[test]
    fn armed_waits_until_dwell() {
        let phase = Phase::Armed { since_ms: 1_000 };
        assert_eq!(
            step_watch(
                phase.clone(),
                1_000 + DWELL_MS - 1,
                true,
                false,
                false,
                false
            ),
            (phase, WatchAction::None)
        );
    }

    /// 武装期停留满阈值：显示。
    #[test]
    fn armed_shows_after_dwell() {
        assert_eq!(
            step_watch(
                Phase::Armed { since_ms: 1_000 },
                1_000 + DWELL_MS,
                true,
                false,
                false,
                false
            ),
            (Phase::Shown { miss: 0 }, WatchAction::Show)
        );
    }

    /// 武装期遇残留可见窗：先收掉并保持武装（新周期从隐藏态计时）。
    #[test]
    fn armed_hides_residual_window() {
        let phase = Phase::Armed { since_ms: 1_000 };
        assert_eq!(
            step_watch(phase.clone(), 1_500, true, false, false, true),
            (phase, WatchAction::Hide)
        );
    }

    /// 武装期离开图标：取消出现回空闲（瞬间划过不打扰）。
    #[test]
    fn armed_cancels_when_leaving_tray() {
        assert_eq!(
            step_watch(
                Phase::Armed { since_ms: 1_000 },
                1_400,
                false,
                false,
                false,
                false
            ),
            (Phase::Idle, WatchAction::None)
        );
    }

    /// 显示期：在图标或悬浮窗内 ⇒ 守住并清零 miss（从卡片移回图标不闪没）。
    #[test]
    fn shown_keeps_while_in_tray_or_window() {
        assert_eq!(
            step_watch(Phase::Shown { miss: 3 }, 2_000, true, false, false, true),
            (Phase::Shown { miss: 0 }, WatchAction::None)
        );
        assert_eq!(
            step_watch(Phase::Shown { miss: 3 }, 2_000, false, true, false, true),
            (Phase::Shown { miss: 0 }, WatchAction::None)
        );
    }

    /// 显示期：离开两区域未满判据拍数 ⇒ 暂不收起（防抖动）。
    #[test]
    fn shown_tolerates_short_absence() {
        assert_eq!(
            step_watch(Phase::Shown { miss: 0 }, 2_000, false, false, false, true),
            (Phase::Shown { miss: 1 }, WatchAction::None)
        );
    }

    /// 显示期：连续离开满判据拍数 ⇒ 收起回空闲。
    #[test]
    fn shown_hides_after_full_absence() {
        assert_eq!(
            step_watch(
                Phase::Shown {
                    miss: MISS_LIMIT - 1
                },
                2_000,
                false,
                false,
                false,
                true
            ),
            (Phase::Idle, WatchAction::Hide)
        );
    }

    /// PATCH008.6：武装期菜单开 ⇒ 转锁死（不出窗）。
    #[test]
    fn armed_goes_suppressed_when_menu_open() {
        assert_eq!(
            step_watch(
                Phase::Armed { since_ms: 1_000 },
                1_200,
                true,
                false,
                true,
                false
            ),
            (Phase::Suppressed, WatchAction::None)
        );
    }

    /// PATCH008.6：显示中菜单开 ⇒ 收窗 + 锁死。
    #[test]
    fn shown_hides_and_suppresses_when_menu_open() {
        assert_eq!(
            step_watch(Phase::Shown { miss: 0 }, 2_000, true, false, true, true),
            (Phase::Suppressed, WatchAction::Hide)
        );
    }

    /// PATCH008.6：锁死期菜单收 ⇒ 回空闲（需新 `Enter` 重新武装，不自动弹出）。
    #[test]
    fn suppressed_releases_when_menu_closes() {
        assert_eq!(
            step_watch(Phase::Suppressed, 3_000, true, false, false, false),
            (Phase::Idle, WatchAction::None)
        );
    }

    /// PATCH008.6：锁死期窗可见一律收走（兜底 show 后到 / 残留两形态）。
    #[test]
    fn suppressed_always_hides_visible_window() {
        assert_eq!(
            step_watch(Phase::Suppressed, 3_000, true, false, true, true),
            (Phase::Suppressed, WatchAction::Hide)
        );
        assert_eq!(
            step_watch(Phase::Suppressed, 3_000, true, false, false, true),
            (Phase::Idle, WatchAction::Hide)
        );
    }
}
