//! 托盘 hover 概览命令（PL028.1）：单点聚合「今日明细（数据日 = 打卡日单一口径）+ 本段
//! 连续工作 + 提醒触发状态 + 设置基准」→ [`HoverSummary`]。**只读**（不写库、不触发提醒）。

use tauri::{AppHandle, Manager, State};

use super::{poison, AppContext, CommandError};
use crate::hover;
use crate::hover::HoverSummary;
use crate::reminder::ReminderConfig;
use crate::session::Clock;

/// 聚合 hover 概览（PL028.1）：今日明细走 [`super::workday::day_summary_inner`] 单一口径；
/// 行①段带取**当前班**打卡点（`WorkdayState::OnDuty`，PATCH007.7 —— 不用日汇总首窗 ✗）；
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
    // 行①段带（PATCH007.7 修正版）：锚点 d0 = 基准 − **今日在本班之前的已用** —— 与自动下班
    // 同一套 `day_budget_context`（单一来源，不另立口径）；到点取自动下班基准槽（与主表盘
    // 轴末/蓝条同源）。工作日在岗才有当前班打卡点。
    let shift_clock_in_at = poison("工作日", ctx.workday.lock())?.clock_in_at();
    let bands = match shift_clock_in_at {
        Some(clock_in_at) => {
            let bc = super::workday::day_budget_context(ctx, clock_in_at, now)?;
            let d0_secs = (i64::from(base_hours) * 3_600 - bc.used_before_shift_secs).max(0);
            let due_at = poison("自动下班", ctx.auto_out.lock())?.due_at;
            Some(hover::ShiftBands {
                clock_in_at,
                d0_secs,
                base_hours,
                due_at,
            })
        }
        None => None,
    };
    let segment = poison("会话", ctx.session.lock())?.segment_secs();
    let last_fired = poison("提醒", ctx.fire.lock())?.last_fired();
    Ok(hover::hover_summary(
        &day,
        bands,
        now,
        segment,
        &ReminderConfig::from_minutes(threshold_min),
        last_fired,
    ))
}

/// 读取托盘 hover 概览（悬浮窗挂载时拉一次；此后每次显示与每秒节拍由后端经 `hover-update`
/// 事件推送）。async（与其余读命令同款）：聚合含存储读，不占主线程。
#[tauri::command]
pub async fn tray_hover(handle: State<'_, AppContext>) -> Result<HoverSummary, CommandError> {
    hover_summary_inner(&handle)
}

/// 重设悬浮窗尺寸（PATCH007.8/.14/.19/.23）：宽默认取固定 [`hover::WINDOW_WIDTH`]（228 ✓），**`width` 为
/// `Some` 时用它**（PATCH007.14：单行态「未在岗」按内容自适应 ✓）；高恒按内容实测（前端 `ceil` 取整，
/// 只多不少 ✓）。由悬浮窗前端调用 —— **JS `setSize` 需 ACL `allow-set-size` 会静默拒绝**（CT 实证 ✓），
/// 故窗口几何统一走 Rust 命令。设完**重锚托盘** ⇒ 右缘恒对齐图标右缘、宽度变化表现为左缘移动 ✓。
/// 失败仅落日志（装饰层纪律）。`.22` 的物理吸附已删（PATCH007.23）：其针对的"半像素"系误判 —— 真因是
/// `.21` 的 `margin: 1px 0` 简写写反致卡片比窗高 1px（溢出恒 1px、与窗高无关）；卡片铺满后吸附亦无用，
/// CT 无此物 ✓。
#[tauri::command]
pub fn tray_hover_resize(
    app: AppHandle,
    width: Option<f64>,
    height: f64,
) -> Result<(), CommandError> {
    let w = width.unwrap_or(hover::WINDOW_WIDTH);
    let Some(window) = app.get_webview_window("tray-hover") else {
        return Ok(()); // 窗不存在（退出收尾）：跳过
    };
    if let Err(err) = window.set_size(tauri::LogicalSize::new(w, height)) {
        crate::warn_diag(&format!("悬浮窗尺寸设置失败：{err}"));
        return Ok(());
    }
    crate::reanchor_tray_hover(&window);
    Ok(())
}
