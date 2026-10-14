//! 托盘 hover 今日概览（PL028.1）：把「今日明细 + 本段连续工作 + 提醒触发状态 + 设置基准」
//! 归约成结构化 DTO。纯逻辑零依赖（禁 import tauri，`cargo test` 直测）——命令层只负责取数。
//! 前端按 [`HoverSummary::single_line`] 渲染 1 行（未在岗）或 3 行（在岗 / 暂下班），
//! 窗口高度由后端按同一判据设定（两端同源）。

use std::time::Duration;

use serde::Serialize;

use crate::reminder::ReminderConfig;
use crate::workday::DaySummary;

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

/// 归约 hover 概览（PL028.1 纯函数，注入输入直测）。
///
/// # 参数
/// `day` 今日明细（数据日 = 打卡日，PL027 单一口径）；`base_hours` 自动下班基准（算加班次数）；
/// `now_secs` 当下 Unix 秒；`segment` 本段连续工作（Paused/Idle 为零）；`config` 提醒配置；
/// `last_fired` 段内上次提醒触发点（未触发为 `None`）。
///
/// # 返回
/// [`HoverSummary`]——`state` 三分支：无打卡 ⇒ 一行；有打卡且已收段 ⇒ 三行「暂下班」；
/// 末班未闭合 ⇒ 三行「正班 / 1st 加班 / 2nd 加班」。
pub fn hover_summary(
    day: &DaySummary,
    base_hours: u32,
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
        // 在岗：本班到点 = 本班打卡 + 剩余预算（与表盘底轨同一口径；加班推后 ⇒ 跟着跳）
        let deadline = duty_started_at + day.remaining_secs;
        let steps = overtime_steps(day.budget_hours, base_hours);
        let label = match steps {
            0 => "正班",
            1 => "1st 加班",
            _ => "2nd 加班",
        };
        (label.to_string(), Some((deadline - now_secs).max(0)))
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

/// 加班次数（PL028.1）：本日上限 = 基准 + 4h × 次数（硬顶 12h）⇒ 次数 = (上限 − 基准) / 4，
/// 只可能 0 / 1 / 2（基准 12h ⇒ 0；8h ⇒ 1；4h ⇒ 2）。
fn overtime_steps(budget_hours: u32, base_hours: u32) -> u32 {
    budget_hours.saturating_sub(base_hours) / 4
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workday::DaySummary;

    fn day(
        duty_started_at: Option<i64>,
        duty_ended_at: Option<i64>,
        work_secs: i64,
        rest_secs: i64,
        budget_hours: u32,
        remaining_secs: i64,
    ) -> DaySummary {
        DaySummary {
            duty_started_at,
            duty_ended_at,
            duty_secs: work_secs + rest_secs,
            work_secs,
            rest_secs,
            blocks: Vec::new(),
            budget_hours,
            used_secs: work_secs + rest_secs,
            remaining_secs,
        }
    }

    fn mins(n: u64) -> Duration {
        Duration::from_secs(n * 60)
    }

    fn config() -> ReminderConfig {
        ReminderConfig::from_minutes(50)
    }

    /// 未上班（今天无打卡）：**只一行**「未在岗」，无剩余值。
    #[test]
    fn off_state_is_single_line() {
        let s = hover_summary(
            &day(None, None, 0, 0, 0, 0),
            8,
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

    /// 在岗正班：三行；剩余 = 本班到点（打卡 + 剩余预算）− now；加班次数 0。
    #[test]
    fn on_duty_regular_shift() {
        // 打卡 1_000，剩余预算 3h ⇒ 到点 1_000 + 10_800 = 11_800；now 10_000 ⇒ 剩 1_800
        let s = hover_summary(
            &day(Some(1_000), None, 3_600, 600, 8, 10_800),
            8,
            10_000,
            mins(30),
            &config(),
            None,
        );
        assert_eq!(s.state, HoverState::OnDuty);
        assert_eq!(s.label, "正班");
        assert_eq!(s.remaining_secs, Some(1_800));
        assert!(!s.single_line);
        assert_eq!(s.work_secs, 3_600);
        assert_eq!(s.rest_secs, 600);
        // 行③：阈值 50 − 本段 30 = 20 分钟
        assert!(
            (s.until_reminder_min - 20.0).abs() < 1e-9,
            "未触发过按阈值倒计"
        );
    }

    /// 加班标签互斥：上限 12h ⇒ 按基准推 1st / 2nd；到顶后再无第三档。
    #[test]
    fn overtime_label_reflects_steps() {
        let one = hover_summary(
            &day(Some(1_000), None, 0, 0, 12, 0),
            8,
            1_000,
            Duration::ZERO,
            &config(),
            None,
        );
        assert_eq!(one.label, "1st 加班", "基准 8h + 一次加班 ⇒ 上限 12h");
        let two = hover_summary(
            &day(Some(1_000), None, 0, 0, 12, 0),
            4,
            1_000,
            Duration::ZERO,
            &config(),
            None,
        );
        assert_eq!(
            two.label, "2nd 加班",
            "基准 4h + 两次加班 ⇒ 上限 12h（硬顶）"
        );
        let zero = hover_summary(
            &day(Some(1_000), None, 0, 0, 12, 0),
            12,
            1_000,
            Duration::ZERO,
            &config(),
            None,
        );
        assert_eq!(zero.label, "正班", "基准 12h 无处加班");
    }

    /// 到点已过（竞态）夹到 0，不出现负剩余。
    #[test]
    fn remaining_never_negative() {
        let s = hover_summary(
            &day(Some(1_000), None, 0, 0, 8, 100),
            8,
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
            &day(Some(1_000), Some(9_000), 7_200, 1_800, 8, 0),
            8,
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
        let s = hover_summary(
            &day(Some(1_000), None, 0, 0, 8, 3_600),
            8,
            1_000,
            mins(52),
            &config(),
            Some(mins(50)),
        );
        // 已触发：5 分钟间隔 − 距上次触发 2 分钟 = 3 分钟
        assert!((s.until_reminder_min - 3.0).abs() < 1e-9);
    }
}
