//! CapsulePulse 应用库：业务模块与 Tauri 应用装配。
//! main.rs 只做薄入口（调 [`run`]）；模块逻辑收在本库——lib 形态下 pub 项即公开 API，
//! 也是 `cargo test` 与命令层（commands/ 目录，按职责分文件）的承载处。
//! 层边界：业务纯逻辑（session.rs 等）平铺于本 src 下且禁 import tauri，
//! 装配层只在本文件——纯逻辑可脱离窗口 cargo test 直测。
//! 玻璃效果：CT 分态纱方案（PL021.4 定案）——恒纯 alpha 透明窗（tauri.conf transparent），
//! 观感由前端纱层承担（失焦 30% 纱、聚焦 0%），Rust 仅转发 window-focus 焦点态；
//! DWM 系统背板材质（PL011）已随 CT 化退役，仅保留 Win11 激活描边隐藏（glass_backdrop.rs）；
//! macOS/Linux 延后（y.problems.md #1）。

pub mod commands;
pub mod period;
pub mod reminder;
pub mod session;
pub mod settings;
pub mod storage;
pub mod workday;

mod diag;
mod glass_backdrop;
mod paths;

use std::sync::Mutex;

use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, WindowEvent,
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

use crate::commands::AppContext;
use crate::reminder::ReminderFire;
use crate::session::WorkSession;
use crate::settings::{ReminderSettings, WindowPos};
use crate::storage::Storage;
use crate::workday::WorkdayState;

/// 运行期警告双写：eprintln（dev 控制台可见）+ diag 落档（release GUI 无 stderr，
/// 文件日志是唯一落点——FIX003.8）。仅用于失败不中断主流程的容错路径。
fn warn_diag(msg: &str) {
    eprintln!("{msg}");
    diag::log(msg);
}

/// 显示主窗口并聚焦（还原最小化；失败逐项记日志，不中断流程）。
fn show_main_window(app: &AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    if let Err(err) = window.show() {
        warn_diag(&format!("窗口显示失败：{err}"));
    }
    if let Err(err) = window.unminimize() {
        warn_diag(&format!("窗口还原失败：{err}"));
    }
    if let Err(err) = window.set_focus() {
        warn_diag(&format!("窗口聚焦失败：{err}"));
    }
}

/// 隐藏主窗口到托盘（关闭语义 = 隐藏；退出走托盘菜单）。
/// 返回结果：命令层上抛给前端可见反馈；事件/托盘路径按容错白名单记日志、不阻断主流程。
pub(crate) fn hide_main_window(app: &AppHandle) -> Result<(), String> {
    save_window_position(app); // 位置记忆：每次隐藏都记一次（失败仅日志，容错白名单④同款纪律）
    let Some(window) = app.get_webview_window("main") else {
        return Err("主窗口缺失，无法隐藏".to_string());
    };
    window.hide().map_err(|err| format!("窗口隐藏失败：{err}"))
}

/// 保存的位置是否落在任一显示器矩形内（纯函数：入参 (x, y, w, h) 物理坐标，可单测）。
/// 显示器枚举失败与真越界同回 false（调用方回默认位）——落日志由调用方负责，此处不静默。
fn position_in_monitors(rects: &[(i32, i32, u32, u32)], x: i32, y: i32) -> bool {
    rects
        .iter()
        .any(|&(mx, my, mw, mh)| x >= mx && x < mx + mw as i32 && y >= my && y < my + mh as i32)
}

/// 显示器矩形清单（物理坐标）：枚举失败回空表 + 双写落日志（调用方按越界处理 → 默认位）。
fn monitor_rects(window: &tauri::WebviewWindow) -> Vec<(i32, i32, u32, u32)> {
    match window.available_monitors() {
        Ok(monitors) => monitors
            .iter()
            .map(|m| {
                let pos = m.position();
                let size = m.size();
                (pos.x, pos.y, size.width, size.height)
            })
            .collect(),
        Err(err) => {
            warn_diag(&format!("显示器枚举失败，按越界回默认位：{err}"));
            Vec::new()
        }
    }
}

/// 主屏右上默认落位（距边 40px）：按窗口实际 `inner_size` 换算，不硬编码窗口尺寸。
fn default_position(window: &tauri::WebviewWindow) -> Result<tauri::PhysicalPosition<i32>, String> {
    const MARGIN_PX: i32 = 40;
    let monitor = window
        .primary_monitor()
        .map_err(|err| format!("主显示器查询失败：{err}"))?
        .ok_or_else(|| "无主显示器，无法计算默认落位".to_string())?;
    let pos = monitor.position();
    let size = monitor.size();
    let inner = window
        .inner_size()
        .map_err(|err| format!("窗口尺寸查询失败：{err}"))?;
    Ok(tauri::PhysicalPosition::new(
        pos.x + size.width as i32 - inner.width as i32 - MARGIN_PX,
        pos.y + MARGIN_PX,
    ))
}

/// 启动落位（setup 调用）：存档位置在任一显示器内 → 用之；否则（从未保存 / 越界 / 枚举失败）
/// → 默认右上落位。位置归 Rust 管辖（前端不读写，见 commands/reminder 的继承说明）。
fn restore_window_position(app: &tauri::App, window: &tauri::WebviewWindow) -> Result<(), String> {
    let saved = app
        .state::<AppContext>()
        .settings
        .lock()
        .map_err(|err| format!("设置锁中毒，位置未恢复：{err}"))?
        .window_pos;
    let position = match saved {
        Some(pos) if position_in_monitors(&monitor_rects(window), pos.x, pos.y) => {
            tauri::PhysicalPosition::new(pos.x, pos.y)
        }
        _ => default_position(window)?,
    };
    window
        .set_position(position)
        .map_err(|err| format!("窗口落位失败：{err}"))
}

/// 保存窗口位置到 config.json（隐藏/退出前调用）：失败落日志不阻断（容错白名单④同款纪律）。
fn save_window_position(app: &AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    let Ok(pos) = window.outer_position() else {
        warn_diag("窗口位置读取失败（位置未保存）");
        return;
    };
    let ctx = app.state::<AppContext>();
    let Ok(mut settings) = ctx.settings.lock() else {
        warn_diag("设置锁中毒，窗口位置未保存");
        return;
    };
    settings.window_pos = Some(WindowPos { x: pos.x, y: pos.y });
    if let Err(err) = settings.save(&ctx.settings_path) {
        warn_diag(&format!("窗口位置保存失败：{err}"));
    }
}

/// 主窗口显隐切换（托盘单击、托盘菜单、Alt+Shift+S 共用）。
fn toggle_main_window(app: &AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    match window.is_visible() {
        Ok(true) => {
            if let Err(err) = hide_main_window(app) {
                warn_diag(&err);
            }
        }
        Ok(false) => show_main_window(app),
        Err(err) => warn_diag(&format!("窗口可见性查询失败：{err}")),
    }
}

/// 托盘菜单动作分发（按菜单项 id；计时切换与退出落库复用命令层自由函数）。
fn handle_action(app: &AppHandle, id: &str) {
    match id {
        "toggle_visible" => toggle_main_window(app),
        "toggle_timer" => {
            let ctx = app.state::<AppContext>();
            if let Err(err) = commands::session::toggle_session(&ctx) {
                warn_diag(&format!("计时切换失败：{err}"));
            }
        }
        "quit" => {
            let ctx = app.state::<AppContext>();
            // 位置记忆（PATCH001.4）：托盘退出走 app.exit(0)、不经 CloseRequested，此处补一次
            save_window_position(app);
            if let Err(err) = commands::session::persist_before_quit(&ctx) {
                // 容错白名单 ④：退出前落库失败仍退出（退出意图优先）；双写助手统一出口（FIX005.5）
                warn_diag(&format!("退出前落库失败（仍退出）：{err}"));
            }
            app.exit(0);
        }
        _ => {}
    }
}

/// 构建托盘：应用图标 + 菜单（显示/隐藏、开始/暂停、退出）；左键单击 = 显隐切换。
fn build_tray(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let toggle_visible = MenuItem::with_id(app, "toggle_visible", "显示/隐藏", true, None::<&str>)?;
    let toggle_timer = MenuItem::with_id(app, "toggle_timer", "开始/暂停", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&toggle_visible, &toggle_timer, &quit])?;
    let icon = app
        .default_window_icon()
        .ok_or("缺少应用图标（tauri.conf.json bundle.icon），托盘无法构建")?;
    TrayIconBuilder::with_id("main-tray")
        .icon(icon.clone())
        .tooltip("CapsulePulse")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| handle_action(app, event.id.as_ref()))
        .on_tray_icon_event(|tray, event| {
            // 左键单击 = 显隐切换；右键弹菜单（show_menu_on_left_click 已关）
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                toggle_main_window(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

/// 注册全局热键：Alt+Shift+P 计时切换、Alt+Shift+S 窗口显隐（Rust 侧注册，不经前端 IPC）。
fn register_global_shortcuts(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    app.global_shortcut()
        .on_shortcut("Alt+Shift+P", |app, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                let ctx = app.state::<AppContext>();
                if let Err(err) = commands::session::toggle_session(&ctx) {
                    warn_diag(&format!("计时切换失败：{err}"));
                }
            }
        })?;
    app.global_shortcut()
        .on_shortcut("Alt+Shift+S", |app, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                toggle_main_window(app);
            }
        })?;
    Ok(())
}

/// 装配并运行 Tauri 应用：窗口属性由 tauri.conf.json 声明（透明无边框 300×400），
/// 玻璃观感走前端 CT 分态纱方案（失焦 30% 纱、聚焦 0% 纱桌面直透；PL021.4 定案——
/// DWM 系统背板材质随 CT 化退役，磨砂观感不再由系统层提供）。
pub fn run() {
    // 存储 + 设置：默认运行时路径（configs/ + data/，双落址见 paths）；失败严格报错退出（错误策略主线）
    let storage = match Storage::open_default() {
        Ok(storage) => storage,
        Err(err) => {
            diag::log(&format!("存储初始化失败：{err}"));
            eprintln!("存储初始化失败：{err}");
            std::process::exit(1);
        }
    };
    let settings_path = match paths::default_config_path() {
        Ok(path) => path,
        Err(err) => {
            diag::log(&format!("设置路径解析失败：{err}"));
            eprintln!("设置路径解析失败：{err}");
            std::process::exit(1);
        }
    };
    let settings = match ReminderSettings::load(&settings_path) {
        Ok(settings) => settings,
        Err(err) => {
            diag::log(&format!("设置加载失败：{err}"));
            eprintln!("设置加载失败：{err}");
            std::process::exit(1);
        }
    };
    // 工作日恢复（PL005）：上次 clock_out 为空 = 在岗中，重启即回到在岗态
    let workday = match storage.workday_latest() {
        Ok(latest) => WorkdayState::from_latest(latest),
        Err(err) => {
            diag::log(&format!("工作日状态恢复失败：{err}"));
            eprintln!("工作日状态恢复失败：{err}");
            std::process::exit(1);
        }
    };
    if workday.is_on_duty() {
        // 用户可感知的关键状态（上次下班打卡缺失，班账从恢复时刻起算），双写落档备查
        warn_diag("已恢复在岗状态（上次下班打卡缺失）");
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            // 二次启动：唤起已有主窗口（单实例，防双开写同一 data/pulse.db）
            show_main_window(app);
        }))
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(AppContext {
            workday: Mutex::new(workday),
            session: Mutex::new(WorkSession::default()),
            storage: Mutex::new(storage),
            settings: Mutex::new(settings),
            fire: Mutex::new(ReminderFire::default()),
            settings_path,
        })
        .invoke_handler(tauri::generate_handler![
            commands::session::session_start,
            commands::session::session_pause,
            commands::session::session_resume,
            commands::session::session_restart,
            commands::session::session_set_run_mode,
            commands::session::session_status,
            commands::stats::session_stats,
            commands::reminder::get_settings,
            commands::reminder::set_settings,
            commands::workday::clock_in,
            commands::workday::clock_out,
            commands::workday::day_detail,
            commands::workday::week_detail,
            commands::workday::workday_total,
            commands::window::main_hide_to_tray
        ])
        .on_window_event(|window, event| {
            // 关闭语义 = 隐藏到托盘（PL004 定案）；退出走托盘菜单
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                if let Err(err) = hide_main_window(window.app_handle()) {
                    warn_diag(&err);
                }
            }
            // 焦点联动分态纱（PL021.4 CT 定案）：失焦 30% 纱、聚焦 0% 纱（桌面直透）——
            // DWM 系统背板材质已随 CT 化退役（恒纯 alpha 透明窗），此处仅转发焦点态给前端
            if let WindowEvent::Focused(focused) = event {
                // 分态纱浓度联动：前端监听本事件切换 focused class；发送失败落日志不吞
                // （观感降级为常纱态，功能不受损；容错白名单 FIX003.7 剩余适用部分）
                if let Err(err) = window.emit("window-focus", *focused) {
                    diag::log(&format!("window-focus 事件发送失败：{err}"));
                }
            }
        })
        .setup(|app| {
            // PANIC 取证（y.problems#5①）：默认 hook 只把 panic 打到 stderr（GUI 进程无人看见），
            // 持锁线程 panic 后只剩"锁中毒"连锁日志、真凶无痕——这里链式挂一个 diag 落 logger，
            // 消息 + 位置 + 线程名进 data/pulse.log，复现即可定位根因
            let default_hook = std::panic::take_hook();
            std::panic::set_hook(Box::new(move |info| {
                let thread = std::thread::current();
                let name = thread.name().unwrap_or("<unnamed>");
                crate::diag::log(&format!("PANIC（线程 {name}）：{info}"));
                default_hook(info);
            }));
            // 玻璃效果不在此挂载（CT 定案）：恒纯 alpha 透明（tauri.conf transparent），
            // 分态纱由前端 ::before 纱层承担（失焦 30%/聚焦 0%），Rust 仅转发 window-focus
            // 主窗口整定：① 位置记忆（PATCH001.4）——存档在屏内 → 恢复，否则默认右上落位；
            // ② 隐藏 Win11 激活描边——聚焦态 DWM 给活动窗口画一圈系统强调色描边，在透明无边框
            //    窗上表现为"窗口外面一圈蓝框"（2026-10-11 用户实证）；③ 补系统圆角——tauri.conf
            //    的 shadow:false 关掉 Tauri 投影后隐式圆角随之丢失，不补则上角露方角（CT e8d049b）
            if let Some(window) = app.get_webview_window("main") {
                if let Err(err) = restore_window_position(app, &window) {
                    warn_diag(&err);
                }
                glass_backdrop::hide_active_border(&window);
                glass_backdrop::apply_round_corners(&window);
            }
            build_tray(app)?;
            register_global_shortcuts(app)?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .unwrap_or_else(|err| {
            eprintln!("CapsulePulse 启动失败：{err}");
            std::process::exit(1);
        });
}

#[cfg(test)]
mod tests {
    use super::position_in_monitors;

    /// PATCH001.4：位置在显示器矩形内 → true（多屏负坐标同样成立）。
    #[test]
    fn position_inside_monitor_is_true() {
        let monitors = [(0, 0, 1920, 1080), (-1920, 0, 1920, 1080)];
        assert!(position_in_monitors(&monitors, 0, 0), "主屏左上角");
        assert!(position_in_monitors(&monitors, 1919, 1079), "主屏右下角内");
        assert!(
            position_in_monitors(&monitors, -1920, 0),
            "副屏（负坐标）左上角"
        );
        assert!(position_in_monitors(&monitors, -1, 500), "副屏右缘内");
    }

    /// PATCH001.4：越界 / 空清单（枚举失败）→ false（调用方回默认落位）。
    #[test]
    fn position_outside_monitors_is_false() {
        let monitors = [(0, 0, 1920, 1080)];
        assert!(!position_in_monitors(&monitors, 1920, 0), "右缘外一格");
        assert!(!position_in_monitors(&monitors, 0, -1), "上缘外一格");
        assert!(!position_in_monitors(&monitors, -5000, 500), "远在屏外");
        assert!(!position_in_monitors(&[], 10, 10), "显示器枚举失败（空表）");
    }
}
