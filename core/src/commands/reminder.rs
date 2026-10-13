//! 提醒副作用与设置命令：系统通知（降级白名单）、reminder-due 事件、提醒设置读写。

use tauri::{AppHandle, Emitter, State};
use tauri_plugin_notification::NotificationExt;

use super::session::ReminderDecision;
use super::{poison, AppContext, CommandError};
use crate::session::Clock;
use crate::settings::ReminderSettings;

/// 提醒副作用：系统通知（notify_enabled 门控；失败降级仅声音——容错白名单）+ reminder-due 事件。
pub(super) fn deliver_reminder(
    app: &AppHandle,
    decision: ReminderDecision,
) -> Result<(), CommandError> {
    if !decision.fire {
        return Ok(());
    }
    if decision.notify_enabled {
        send_notification(app, decision.threshold_min);
    }
    app.emit("reminder-due", decision.threshold_min)
        .map_err(|e| CommandError::Event(e.to_string()))
}

/// 系统通知（容错白名单：发送失败降级仅声音——错误落日志，不阻断事件）。
fn send_notification(app: &AppHandle, threshold_min: u32) {
    if let Err(err) = app
        .notification()
        .builder()
        .title("CapsulePulse")
        .body(format!("已连续工作 {threshold_min} 分钟，休息一下吧"))
        .show()
    {
        eprintln!("系统通知发送失败（降级仅声音）：{err}");
        crate::diag::log(&format!("系统通知发送失败（降级仅声音）：{err}"));
    }
}

/// 设置读取。
fn get_settings_inner<C: Clock>(ctx: &AppContext<C>) -> Result<ReminderSettings, CommandError> {
    Ok(poison("设置", ctx.settings.lock())?.clone())
}

/// 设置保存：校验 → 持久化 → 更新内存态（任一步失败不污染内存态）。
/// **窗口位置归 Rust 管辖**（PATCH001.4）：提交值只覆盖提醒字段，位置从内存态继承——否则
/// 设置板提交的陈旧副本会把用户刚挪的窗口位置抹回旧值（前端不参与位置读写）。
/// **在岗禁改自动下班小时**（PL025.5）：本班基准不可中途改（账目口径唯一）；**整日锁定**
/// （PATCH005.3）：今天一旦开启上班 → 基准今天内（含今天下班后）不可改，仅"第二天未上班"可改；
/// 锁序 workday → settings 单向合法（见 AppContext 注释）。
fn set_settings_inner<C: Clock>(
    ctx: &AppContext<C>,
    settings: &ReminderSettings,
) -> Result<(), CommandError> {
    let (inherited_pos, current_hours, on_duty) = {
        let workday = poison("工作日", ctx.workday.lock())?;
        let current = poison("设置", ctx.settings.lock())?;
        (
            current.window_pos,
            current.workday_auto_out_hours,
            workday.is_on_duty(),
        )
    };
    // PATCH005.3 整日锁定：今天**一旦开启上班**（今天日历日内有打卡）→ 基准整日不可改（含今天
    // 下班后）；仅"第二天且未上班"可改——跨零点夜班过了零点即视为第二天，可改（与用户定案一致）。
    let today_start = crate::period::day_start_secs(
        &chrono::TimeZone::timestamp_opt(&chrono::Local, super::now_secs(ctx)?, 0)
            .single()
            .ok_or(CommandError::Clock)?,
    );
    let today_has_clock_in = poison("存储", ctx.storage.lock())?
        .events_between(today_start, today_start + 86_400)?
        .iter()
        .any(|(_, kind)| *kind == crate::workday::EventKind::ClockIn);
    if settings.workday_auto_out_hours != current_hours {
        if on_duty {
            return Err(CommandError::AutoOutLockedOnDuty(current_hours));
        }
        if today_has_clock_in {
            return Err(CommandError::AutoOutLockedToday(current_hours));
        }
    }
    let mut merged = settings.clone();
    merged.window_pos = inherited_pos;
    merged.save(&ctx.settings_path)?;
    *poison("设置", ctx.settings.lock())? = merged;
    Ok(())
}

/// 读取提醒设置。
#[tauri::command]
pub fn get_settings(handle: State<'_, AppContext>) -> Result<ReminderSettings, CommandError> {
    get_settings_inner(&handle)
}

/// 保存提醒设置（校验 → 持久化 → 更新内存态，即时生效）。
#[tauri::command]
pub fn set_settings(
    handle: State<'_, AppContext>,
    settings: ReminderSettings,
) -> Result<(), CommandError> {
    set_settings_inner(&handle, &settings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::test_support::{begin_duty, ctx, FakeClock};
    use crate::settings::WindowPos;

    /// T4a：set_settings 校验 + 持久化 + 内存态更新；非法阈值被拒。
    #[test]
    fn settings_set_persists_and_validates() {
        let mut ctx = ctx(FakeClock::new());
        // 独立设置文件：避免与同模块其他用例争用 temp_settings_path("ctx")（并行执行会互相覆盖）
        ctx.settings_path = crate::commands::test_support::temp_settings_path("settings-set");
        let s = ReminderSettings {
            threshold_min: 30,
            sound_enabled: false,
            notify_enabled: true,
            workday_auto_out_hours: 9,
            window_pos: None,
        };
        set_settings_inner(&ctx, &s).unwrap();
        assert_eq!(get_settings_inner(&ctx).unwrap(), s);
        assert!(std::fs::read_to_string(&ctx.settings_path)
            .unwrap()
            .contains("\"threshold_min\": 30"));

        let bad = ReminderSettings {
            threshold_min: 0,
            sound_enabled: true,
            notify_enabled: true,
            workday_auto_out_hours: 8,
            window_pos: None,
        };
        assert!(matches!(
            set_settings_inner(&ctx, &bad),
            Err(CommandError::Settings(
                crate::settings::SettingsError::InvalidThreshold(0)
            ))
        ));
    }

    /// PATCH005.3 整日锁定：今天已开启上班（含今天下班后）→ 改小时数被拒；无今日打卡 → 放行。
    #[test]
    fn auto_out_hours_locked_for_whole_day() {
        let mut ctx = ctx(FakeClock::new());
        ctx.settings_path = crate::commands::test_support::temp_settings_path("auto-out-lock");
        begin_duty(&ctx); // 今天打卡（clock_in 落在"今天"日历日内）
        let s = ReminderSettings {
            workday_auto_out_hours: 6,
            ..get_settings_inner(&ctx).unwrap()
        };
        assert!(
            matches!(
                set_settings_inner(&ctx, &s),
                Err(CommandError::AutoOutLockedOnDuty(_))
            ),
            "在岗中：拒改（既有口径优先命中）"
        );
        // 今天下班后：非在岗，但今天已有打卡 → 整日锁定同样拒改
        crate::commands::workday::clock_out_inner(&ctx, 2_000, crate::workday::EventKind::ClockOut)
            .unwrap();
        assert!(
            matches!(
                set_settings_inner(&ctx, &s),
                Err(CommandError::AutoOutLockedToday(_))
            ),
            "今天下班后仍整日锁定（非在岗但今日已有打卡）"
        );
    }

    /// PATCH001.4：整结构覆盖不抹窗口位置（位置归 Rust 管辖，命令层从内存态继承）。
    /// 播种方式与生产一致 = 直接写内存态（`save_window_position` 就是这么做的），
    /// 再走设置板路径提交一份不含位置的副本，断言位置未被抹。
    #[test]
    fn settings_set_preserves_window_pos() {
        let mut ctx = ctx(FakeClock::new());
        // 独立设置文件：避免与同模块其他用例争用 temp_settings_path("ctx")（并行执行会互相覆盖）
        ctx.settings_path =
            crate::commands::test_support::temp_settings_path("preserve-window-pos");
        {
            let mut settings = ctx.settings.lock().unwrap();
            settings.window_pos = Some(WindowPos { x: 320, y: 80 });
            settings.save(&ctx.settings_path).unwrap();
        }

        // 前端提交的副本不含位置（None）→ 位置应保留，提醒字段照常覆盖
        let incoming = ReminderSettings {
            threshold_min: 30,
            ..ReminderSettings::default()
        };
        assert_eq!(incoming.window_pos, None);
        set_settings_inner(&ctx, &incoming).unwrap();

        let after = get_settings_inner(&ctx).unwrap();
        assert_eq!(after.threshold_min, 30, "提醒字段照常覆盖");
        assert_eq!(
            after.window_pos,
            Some(WindowPos { x: 320, y: 80 }),
            "窗口位置未被抹"
        );
        assert!(std::fs::read_to_string(&ctx.settings_path)
            .unwrap()
            .contains("\"x\": 320"));
    }

    /// PL025.5：在岗期间禁改自动下班小时数（本班基准不可中途改）；其余字段照常可改；
    /// 未在岗可改。
    #[test]
    fn settings_set_rejects_auto_hours_change_on_duty() {
        let duty_ctx = ctx(FakeClock::new());
        begin_duty(&duty_ctx); // 上班 → 在岗（默认 8h）

        let changed = ReminderSettings {
            workday_auto_out_hours: 9,
            ..ReminderSettings::default()
        };
        assert!(matches!(
            set_settings_inner(&duty_ctx, &changed),
            Err(CommandError::AutoOutLockedOnDuty(8))
        ));

        // 其余字段照常可改（hours 维持 8）
        let others = ReminderSettings {
            threshold_min: 30,
            ..ReminderSettings::default()
        };
        set_settings_inner(&duty_ctx, &others).unwrap();
        let after = get_settings_inner(&duty_ctx).unwrap();
        assert_eq!(after.threshold_min, 30, "在岗不拦其他字段");
        assert_eq!(after.workday_auto_out_hours, 8, "小时数未被改动");

        // 未在岗：可改
        let off_ctx = ctx(FakeClock::new());
        set_settings_inner(&off_ctx, &changed).unwrap();
        assert_eq!(
            get_settings_inner(&off_ctx).unwrap().workday_auto_out_hours,
            9
        );
    }
}
