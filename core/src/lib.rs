//! CapsulePulse 应用库：业务模块与 Tauri 应用装配。
//! main.rs 只做薄入口（调 [`run`]）；模块逻辑收在本库——lib 形态下 pub 项即公开 API，
//! 也是 `cargo test` 与命令层（commands/ 目录，按职责分文件）的承载处。
//! 层边界：业务纯逻辑（session.rs 等）平铺于本 src 下且禁 import tauri，
//! 装配层只在本文件——纯逻辑可脱离窗口 cargo test 直测。
//! 玻璃效果：Windows 实机走 Acrylic（PL001 阶段 B 已判定通过）；macOS/Linux 延后（y.problems.md #1）。

pub mod commands;
pub mod period;
pub mod reminder;
pub mod session;
pub mod settings;
pub mod storage;
pub mod workday;

mod paths;

use std::sync::Mutex;

use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, WindowEvent,
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

use crate::commands::AppContext;
use crate::reminder::ReminderFire;
use crate::session::WorkSession;
use crate::settings::ReminderSettings;
use crate::storage::Storage;
use crate::workday::WorkdayState;

/// 显示主窗口并聚焦（还原最小化；失败逐项记日志，不中断流程）。
fn show_main_window(app: &AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    if let Err(err) = window.show() {
        eprintln!("窗口显示失败：{err}");
    }
    if let Err(err) = window.unminimize() {
        eprintln!("窗口还原失败：{err}");
    }
    if let Err(err) = window.set_focus() {
        eprintln!("窗口聚焦失败：{err}");
    }
}

/// 隐藏主窗口到托盘（关闭语义 = 隐藏；退出走托盘菜单）。
fn hide_main_window(app: &AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    if let Err(err) = window.hide() {
        eprintln!("窗口隐藏失败：{err}");
    }
}

/// 主窗口显隐切换（托盘单击、托盘菜单、Alt+Shift+S 共用）。
fn toggle_main_window(app: &AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    match window.is_visible() {
        Ok(true) => hide_main_window(app),
        Ok(false) => show_main_window(app),
        Err(err) => eprintln!("窗口可见性查询失败：{err}"),
    }
}

/// 托盘菜单动作分发（按菜单项 id；计时切换与退出落库复用命令层自由函数）。
fn handle_action(app: &AppHandle, id: &str) {
    match id {
        "toggle_visible" => toggle_main_window(app),
        "toggle_timer" => {
            let ctx = app.state::<AppContext>();
            if let Err(err) = commands::session::toggle_session(&ctx) {
                eprintln!("计时切换失败：{err}");
            }
        }
        "quit" => {
            let ctx = app.state::<AppContext>();
            if let Err(err) = commands::session::persist_before_quit(&ctx) {
                eprintln!("退出前落库失败（仍退出）：{err}");
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
                    eprintln!("计时切换失败：{err}");
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

/// 装配并运行 Tauri 应用：窗口属性由 tauri.conf.json 声明（透明无边框 360×480），玻璃效果在 setup 挂载。
pub fn run() {
    // 存储 + 设置：默认运行时路径（configs/ + data/，双落址见 paths）；失败严格报错退出（错误策略主线）
    let storage = match Storage::open_default() {
        Ok(storage) => storage,
        Err(err) => {
            eprintln!("存储初始化失败：{err}");
            std::process::exit(1);
        }
    };
    let settings_path = match paths::default_config_path() {
        Ok(path) => path,
        Err(err) => {
            eprintln!("设置路径解析失败：{err}");
            std::process::exit(1);
        }
    };
    let settings = match ReminderSettings::load(&settings_path) {
        Ok(settings) => settings,
        Err(err) => {
            eprintln!("设置加载失败：{err}");
            std::process::exit(1);
        }
    };
    // 工作日恢复（PL005）：上次 clock_out 为空 = 在岗中，重启即回到在岗态
    let workday = match storage.workday_latest() {
        Ok(latest) => WorkdayState::from_latest(latest),
        Err(err) => {
            eprintln!("工作日状态恢复失败：{err}");
            std::process::exit(1);
        }
    };
    if workday.is_on_duty() {
        eprintln!("已恢复在岗状态（上次下班打卡缺失）");
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
            commands::session::session_status,
            commands::stats::session_stats,
            commands::reminder::get_settings,
            commands::reminder::set_settings,
            commands::workday::clock_in,
            commands::workday::clock_out,
            commands::workday::day_detail
        ])
        .on_window_event(|window, event| {
            // 关闭语义 = 隐藏到托盘（PL004 定案）；退出走托盘菜单
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                hide_main_window(window.app_handle());
            }
        })
        .setup(|app| {
            // 玻璃效果挂载：失败严格抛错（setup 错误会上抛阻断启动），不静默降级
            #[cfg(target_os = "windows")]
            {
                let Some(window) = app.get_webview_window("main") else {
                    return Err("主窗口不存在（tauri.conf.json 声明与代码不符）".into());
                };
                // 半透明深灰色调：静态 tint，深浅色观感由前端 CSS 分层处理（PL001.5）
                window_vibrancy::apply_acrylic(&window, Some((32, 32, 32, 125)))?;
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
