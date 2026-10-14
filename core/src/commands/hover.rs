//! 托盘 hover 概览命令（PL028.1）：单点聚合「今日明细（数据日 = 打卡日单一口径）+ 本段
//! 连续工作 + 提醒触发状态 + 设置基准」→ [`HoverSummary`]。**只读**（不写库、不触发提醒）。

use tauri::State;

use super::{poison, AppContext, CommandError};
use crate::hover::HoverSummary;
use crate::reminder::ReminderConfig;
use crate::session::Clock;

/// 聚合 hover 概览（PL028.1）：今日明细走 [`super::workday::day_summary_inner`] 单一口径；
/// 本段连续工作取会话状态机（Paused / Idle 为零）；提醒触发点取段内触发槽。
/// 各锁逐个取用即释放（不嵌套），无锁序面。
pub(crate) fn hover_summary_inner<C: Clock>(
    ctx: &AppContext<C>,
) -> Result<HoverSummary, CommandError> {
    let now = super::now_secs(ctx)?;
    let offset = super::workday::data_day_offset(ctx, now)?;
    let day = super::workday::day_summary_inner(ctx, offset, now)?;
    let (threshold_min, base_hours) = {
        let settings = poison("设置", ctx.settings.lock())?;
        (settings.threshold_min, settings.workday_auto_out_hours)
    };
    let segment = poison("会话", ctx.session.lock())?.segment_secs();
    let last_fired = poison("提醒", ctx.fire.lock())?.last_fired();
    Ok(crate::hover::hover_summary(
        &day,
        base_hours,
        now,
        segment,
        &ReminderConfig::from_minutes(threshold_min),
        last_fired,
    ))
}

/// 读取托盘 hover 概览（悬浮窗挂载时拉一次；此后每次显示由后端经 `hover-update` 事件推送）。
/// async（与其余读命令同款）：聚合含存储读，不占主线程。
#[tauri::command]
pub async fn tray_hover(handle: State<'_, AppContext>) -> Result<HoverSummary, CommandError> {
    hover_summary_inner(&handle)
}
