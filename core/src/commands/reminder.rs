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
    }
}

/// 设置读取。
fn get_settings_inner<C: Clock>(ctx: &AppContext<C>) -> Result<ReminderSettings, CommandError> {
    Ok(poison(ctx.settings.lock())?.clone())
}

/// 设置保存：校验 → 持久化 → 更新内存态（任一步失败不污染内存态）。
fn set_settings_inner<C: Clock>(
    ctx: &AppContext<C>,
    settings: &ReminderSettings,
) -> Result<(), CommandError> {
    settings.save(&ctx.settings_path)?;
    let mut current = poison(ctx.settings.lock())?;
    *current = settings.clone();
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
    use crate::commands::test_support::{ctx, FakeClock};

    /// T4a：set_settings 校验 + 持久化 + 内存态更新；非法阈值被拒。
    #[test]
    fn settings_set_persists_and_validates() {
        let ctx = ctx(FakeClock::new());
        let s = ReminderSettings {
            threshold_min: 30,
            sound_enabled: false,
            notify_enabled: true,
            workday_auto_out_hours: 9,
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
        };
        assert!(matches!(
            set_settings_inner(&ctx, &bad),
            Err(CommandError::Settings(
                crate::settings::SettingsError::InvalidThreshold(0)
            ))
        ));
    }
}
