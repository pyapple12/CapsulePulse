//! 提醒副作用与设置命令：系统通知（降级白名单）、reminder-due 事件、提醒设置读写。

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
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
/// **热键归热键写路径管辖**（PL028.2）：本结构提交的热键字段从内存态继承，防陈旧副本抹掉刚录的新键。
/// 锁序 workday → settings 单向合法（见 AppContext 注释）。
fn set_settings_inner<C: Clock>(
    ctx: &AppContext<C>,
    settings: &ReminderSettings,
) -> Result<(), CommandError> {
    let (inherited_pos, inherited_hotkeys, current_hours, on_duty) = {
        let workday = poison("工作日", ctx.workday.lock())?;
        let current = poison("设置", ctx.settings.lock())?;
        (
            current.window_pos,
            (
                current.hotkey_toggle_visible.clone(),
                current.hotkey_toggle_timer.clone(),
            ),
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
    merged.hotkey_toggle_visible = inherited_hotkeys.0;
    merged.hotkey_toggle_timer = inherited_hotkeys.1;
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
/// PL028.3 起落库后再做两件窗口偏好副作用：应用置顶到主窗 + 同步双入口
/// （广播 prefs-changed + 托盘勾选态）——设置面板改偏好即时生效的唯一出口。
#[tauri::command]
pub fn set_settings(
    app: AppHandle,
    handle: State<'_, AppContext>,
    settings: ReminderSettings,
) -> Result<(), CommandError> {
    set_settings_inner(&handle, &settings)?;
    apply_always_on_top(&app, &handle);
    crate::sync_prefs_ui(&app);
    Ok(())
}

/// 窗口偏好项（PL028.3）：托盘右键菜单与设置面板双入口共用的开关标识。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrefKind {
    /// 窗口置顶（恒在最前）
    AlwaysOnTop,
    /// 贴边吸附（拖动松手贴边停驻）
    SnapToEdge,
}

/// 窗口偏好快照（PL028.3）：`prefs-changed` 广播载荷——键名漂移从此编译期可查，
/// 前端回显与托盘勾选态共享同一份事实源（照 CT FIX005.9 结构化载荷纪律）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrefsSnapshot {
    /// 窗口置顶开关
    pub always_on_top: bool,
    /// 贴边吸附开关
    pub snap_to_edge: bool,
}

/// 全量广播窗口偏好（PL028.3）：任一路径改动后调用，前端据此回显开关态。
/// 锁中毒跳过广播（纯同步层——落库已完成，不回滚不报错）。
pub(crate) fn emit_prefs_changed<C: Clock>(app: &AppHandle, ctx: &AppContext<C>) {
    let snapshot = match ctx.settings.lock() {
        Ok(settings) => PrefsSnapshot {
            always_on_top: settings.always_on_top,
            snap_to_edge: settings.snap_to_edge,
        },
        Err(err) => {
            crate::warn_diag(&format!("prefs-changed 读取失败（跳过广播）：{err:?}"));
            return;
        }
    };
    if let Err(err) = app.emit("prefs-changed", snapshot) {
        crate::warn_diag(&format!("prefs-changed 广播失败：{err}"));
    }
}

/// 读单个窗口偏好（PL028.3）：托盘菜单路径取当前值（取反后写回）。
pub(crate) fn window_pref<C: Clock>(
    ctx: &AppContext<C>,
    kind: PrefKind,
) -> Result<bool, CommandError> {
    let settings = poison("设置", ctx.settings.lock())?;
    Ok(match kind {
        PrefKind::AlwaysOnTop => settings.always_on_top,
        PrefKind::SnapToEdge => settings.snap_to_edge,
    })
}

/// 写单个窗口偏好（PL028.3 单一写路径）：落库 + 置顶即时应用到主窗。托盘菜单与设置
/// 面板共用；广播与托盘勾选态由调用方经 [`crate::sync_prefs_ui`] 收口（单一同步出口）。
pub(crate) fn set_window_pref<C: Clock>(
    app: &AppHandle,
    ctx: &AppContext<C>,
    kind: PrefKind,
    on: bool,
) -> Result<(), CommandError> {
    set_window_pref_core(ctx, kind, on)?;
    if let PrefKind::AlwaysOnTop = kind {
        apply_always_on_top(app, ctx);
    }
    Ok(())
}

/// [`set_window_pref`] 核心实现：更新内存态 + 原子落盘（窗口操作在壳层；直测 = 临时
/// 路径内存库，禁触真实用户数据）。
fn set_window_pref_core<C: Clock>(
    ctx: &AppContext<C>,
    kind: PrefKind,
    on: bool,
) -> Result<(), CommandError> {
    let mut settings = poison("设置", ctx.settings.lock())?;
    match kind {
        PrefKind::AlwaysOnTop => settings.always_on_top = on,
        PrefKind::SnapToEdge => settings.snap_to_edge = on,
    }
    settings.save(&ctx.settings_path)?;
    Ok(())
}

/// 把置顶设置应用到主窗（PL028.3）：失败落日志不阻断——设置已落库，下次启动 setup
/// 按 config 重应用即自愈（窗口为装饰层，不反噬业务；容错白名单登记项）。
pub(crate) fn apply_always_on_top<C: Clock>(app: &AppHandle, ctx: &AppContext<C>) {
    let on = match ctx.settings.lock() {
        Ok(settings) => settings.always_on_top,
        Err(err) => {
            crate::warn_diag(&format!("设置锁中毒，置顶未应用：{err:?}"));
            return;
        }
    };
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    if let Err(err) = window.set_always_on_top(on) {
        crate::warn_diag(&format!("置顶应用失败：{err}"));
    }
}

/// 快捷键动作标识（PL028.2）：两条动作共用一条写命令与一套录键 UI。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HotkeyAction {
    /// 窗口显隐切换（"显" = 显示 + 聚焦主窗）
    ToggleVisible,
    /// 工作 / 休息切换
    ToggleTimer,
}

impl HotkeyAction {
    /// 读该动作当前落库的组合串（缺省值见 settings.rs）。
    fn current<C: Clock>(self, ctx: &AppContext<C>) -> Result<String, CommandError> {
        let settings = poison("设置", ctx.settings.lock())?;
        Ok(match self {
            HotkeyAction::ToggleVisible => settings.hotkey_toggle_visible.clone(),
            HotkeyAction::ToggleTimer => settings.hotkey_toggle_timer.clone(),
        })
    }
}

/// 按动作注册全局热键（PL028.2）：`on_shortcut` 注册 + 挂处理函数（注册失败上抛字符串，
/// 命令层据此回滚；启动路径仅落日志）。处理函数只认 `Pressed`（防按/放双触发）。
pub(crate) fn register_action(
    app: &AppHandle,
    action: HotkeyAction,
    combo: &str,
) -> Result<(), String> {
    app.global_shortcut()
        .on_shortcut(combo, move |app, _shortcut, event| {
            if event.state != ShortcutState::Pressed {
                return;
            }
            match action {
                HotkeyAction::ToggleVisible => crate::toggle_main_window(app),
                HotkeyAction::ToggleTimer => {
                    let ctx = app.state::<AppContext>();
                    if let Err(err) = super::session::toggle_session(&ctx) {
                        crate::warn_diag(&format!("热键计时切换失败：{err}"));
                    }
                }
            }
        })
        .map_err(|err| err.to_string())
}

/// 写全局热键（PL028.2 单一写路径）：校验规范化 → 撤旧 → 注册新（失败则回滚旧注册并上抛，
/// 内存态与磁盘不动）→ 落库。
///
/// # 返回
/// 规范化后的组合串——前端据此回显，与落库值同源。
#[tauri::command]
pub fn set_hotkey(
    app: AppHandle,
    handle: State<'_, AppContext>,
    action: HotkeyAction,
    combo: String,
) -> Result<String, CommandError> {
    let normalized = crate::hotkey::parse(&combo)?;
    let old = action.current(&handle)?;
    if normalized == old {
        return Ok(normalized); // 无变化：不折腾注册
    }
    // 撤旧：旧键可能本就没注册成功（启动注册失败仅日志），故失败也仅记、继续换新
    if let Err(err) = app.global_shortcut().unregister(old.as_str()) {
        crate::warn_diag(&format!("旧热键撤销失败（继续换新）：{err}"));
    }
    if let Err(err) = register_action(&app, action, &normalized) {
        // 回滚：恢复旧注册（失败仅日志——旧键可用性交由下次启动按 config 恢复）
        if let Err(rollback) = register_action(&app, action, &old) {
            crate::warn_diag(&format!("旧热键回滚注册失败：{rollback}"));
        }
        return Err(CommandError::HotkeyRegister(format!(
            "快捷键注册失败（已保留 {old}）：{err}"
        )));
    }
    set_hotkey_core(&handle, action, &normalized)?;
    Ok(normalized)
}

/// [`set_hotkey`] 核心实现：更新内存态 + 原子落盘（注册在壳层；直测 = 临时路径内存库，
/// 禁触真实用户数据）。
fn set_hotkey_core<C: Clock>(
    ctx: &AppContext<C>,
    action: HotkeyAction,
    combo: &str,
) -> Result<(), CommandError> {
    let mut settings = poison("设置", ctx.settings.lock())?;
    match action {
        HotkeyAction::ToggleVisible => settings.hotkey_toggle_visible = combo.to_string(),
        HotkeyAction::ToggleTimer => settings.hotkey_toggle_timer = combo.to_string(),
    }
    settings.save(&ctx.settings_path)?;
    Ok(())
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
            always_on_top: false,
            snap_to_edge: true,
            hotkey_toggle_visible: "Alt+Shift+S".to_string(),
            hotkey_toggle_timer: "Alt+Shift+P".to_string(),
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
            always_on_top: false,
            snap_to_edge: true,
            hotkey_toggle_visible: "Alt+Shift+S".to_string(),
            hotkey_toggle_timer: "Alt+Shift+P".to_string(),
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

    /// PL028.3：窗口偏好写路径 core 直测——托盘与设置面板共用的落库口径（临时路径
    /// 内存库，禁触真实用户数据）；读回一致 + 另一字段不受牵连。
    #[test]
    fn window_pref_set_and_get_roundtrip() {
        let mut ctx = ctx(FakeClock::new());
        ctx.settings_path = crate::commands::test_support::temp_settings_path("window-pref");
        assert!(
            !window_pref(&ctx, PrefKind::AlwaysOnTop).unwrap(),
            "置顶缺省关"
        );
        assert!(
            window_pref(&ctx, PrefKind::SnapToEdge).unwrap(),
            "贴边缺省开"
        );

        set_window_pref_core(&ctx, PrefKind::AlwaysOnTop, true).unwrap();
        assert!(window_pref(&ctx, PrefKind::AlwaysOnTop).unwrap());
        assert!(
            window_pref(&ctx, PrefKind::SnapToEdge).unwrap(),
            "另一字段不受牵连"
        );

        set_window_pref_core(&ctx, PrefKind::SnapToEdge, false).unwrap();
        assert!(!window_pref(&ctx, PrefKind::SnapToEdge).unwrap());

        // 落盘重读确认持久化（重启后由 setup 应用）
        let loaded = crate::settings::ReminderSettings::load(&ctx.settings_path).unwrap();
        assert!(loaded.always_on_top && !loaded.snap_to_edge);
    }

    /// PL028.3：prefs-changed 载荷键名契约锁定——前端回显按此键名取值，
    /// 新增偏好字段须同步补断言防键名漂移（照 CT 同款契约纪律）。
    #[test]
    fn prefs_snapshot_serialization_contract() {
        let json = serde_json::to_string(&PrefsSnapshot {
            always_on_top: true,
            snap_to_edge: false,
        })
        .unwrap();
        assert!(json.contains("\"always_on_top\":true"));
        assert!(json.contains("\"snap_to_edge\":false"));
    }

    /// PL028.2：热键写路径 core 直测——规范化串落库 + 回读一致 + 另一条动作不受牵连
    /// （临时路径内存库，禁触真实用户数据）。
    #[test]
    fn hotkey_set_persists_normalized_combo() {
        let mut ctx = ctx(FakeClock::new());
        ctx.settings_path = crate::commands::test_support::temp_settings_path("hotkey-set");
        assert_eq!(
            get_settings_inner(&ctx).unwrap().hotkey_toggle_visible,
            "Alt+Shift+S",
            "缺省值 = 原硬编码热键"
        );
        set_hotkey_core(&ctx, HotkeyAction::ToggleVisible, "Ctrl+Alt+C").unwrap();
        let after = get_settings_inner(&ctx).unwrap();
        assert_eq!(after.hotkey_toggle_visible, "Ctrl+Alt+C");
        assert_eq!(after.hotkey_toggle_timer, "Alt+Shift+P", "另一条不受牵连");
        // 落盘重读确认持久化（重启由 setup 按 config 注册）
        let loaded = ReminderSettings::load(&ctx.settings_path).unwrap();
        assert_eq!(loaded.hotkey_toggle_visible, "Ctrl+Alt+C");
    }

    /// PL028.2：设置面板整结构提交不抹热键（热键归热键写路径管辖，命令层从内存态继承）。
    #[test]
    fn settings_set_preserves_hotkeys() {
        let mut ctx = ctx(FakeClock::new());
        ctx.settings_path = crate::commands::test_support::temp_settings_path("preserve-hotkeys");
        set_hotkey_core(&ctx, HotkeyAction::ToggleTimer, "Ctrl+Alt+T").unwrap();

        // 前端提交的副本不含新热键（陈旧默认值）→ 热键应保留，提醒字段照常覆盖
        let incoming = ReminderSettings {
            threshold_min: 30,
            ..ReminderSettings::default()
        };
        set_settings_inner(&ctx, &incoming).unwrap();

        let after = get_settings_inner(&ctx).unwrap();
        assert_eq!(after.threshold_min, 30, "提醒字段照常覆盖");
        assert_eq!(after.hotkey_toggle_timer, "Ctrl+Alt+T", "热键未被抹");
    }

    /// PL028.2：HotkeyAction 载荷键名契约锁定（前端按 snake_case 动作名传参）。
    #[test]
    fn hotkey_action_serialization_contract() {
        assert_eq!(
            serde_json::to_string(&HotkeyAction::ToggleVisible).unwrap(),
            "\"toggle_visible\""
        );
        assert_eq!(
            serde_json::to_string(&HotkeyAction::ToggleTimer).unwrap(),
            "\"toggle_timer\""
        );
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
