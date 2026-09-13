//! CapsulePulse 应用库：业务模块与 Tauri 应用装配。
//! main.rs 只做薄入口（调 [`run`]）；模块逻辑收在本库——lib 形态下 pub 项即公开 API，
//! 也是 `cargo test` 与命令层（commands/ 目录，按职责分文件）的承载处。
//! 层边界：业务纯逻辑（session.rs 等）平铺于本 src 下且禁 import tauri，
//! 装配层只在本文件——纯逻辑可脱离窗口 cargo test 直测。
//! 玻璃效果：Windows 实机走 DWM 系统背板常驻真磨砂（PL010.7，Terminal 同款机制）；macOS/Linux 延后（y.problems.md #1）。

pub mod commands;
pub mod period;
pub mod reminder;
pub mod session;
pub mod settings;
pub mod storage;
pub mod workday;

mod diag;
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
                // 容错白名单 ④：退出前落库失败仍退出（退出意图优先）；失败落诊断日志
                diag::log(&format!("退出前落库失败（仍退出）：{err}"));
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

/// 设置窗口 DWM 系统背板材质（PL011 焦点联动）：kind = DWMSBT_TRANSIENTWINDOW（3，Acrylic
/// 真磨砂，DWM 实时合成背后真实内容）或 DWMSBT_NONE（1，撤回背板、纯 alpha 透明）。透明走
/// 像素 alpha 合成不绑定焦点、失焦常驻；磨砂是 DWM 焦点绑定材质，只做聚焦态点睛。失败返回
/// Err 由调用方记录——焦点切换是运行时事件，不可中断主流程（错误策略：明确记录非吞错）。
///
/// # 参数
/// - `window`：目标窗口（取其原生句柄）
/// - `kind`：DWM_SYSTEMBACKDROP_TYPE 枚举值（3 = Acrylic，1 = 无背板）
#[cfg(target_os = "windows")]
fn set_system_backdrop(window: &tauri::Window, kind: u32) -> Result<(), String> {
    #[link(name = "dwmapi")]
    extern "system" {
        fn DwmSetWindowAttribute(hwnd: isize, attr: u32, value: *const u32, size: u32) -> i32;
    }
    let hwnd = window.hwnd().map_err(|e| format!("取窗口句柄失败：{e}"))?.0 as isize;
    // DWMWA_SYSTEMBACKDROP_TYPE = 38
    let hr = unsafe { DwmSetWindowAttribute(hwnd, 38, &kind, 4) };
    if hr != 0 {
        return Err(format!("设置系统背板（kind={kind}）失败：HRESULT {hr:#x}"));
    }
    Ok(())
}

/// 装配并运行 Tauri 应用：窗口属性由 tauri.conf.json 声明（透明无边框 380×560），玻璃效果走
/// 焦点联动（平时纯 alpha 透明，聚焦瞬间挂 DWM Acrylic 真磨砂，PL011 定案）。
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
            commands::workday::day_detail,
            commands::workday::week_detail
        ])
        .on_window_event(|window, event| {
            // 关闭语义 = 隐藏到托盘（PL004 定案）；退出走托盘菜单
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                hide_main_window(window.app_handle());
            }
            // 焦点联动材质（PL011）：平时纯 alpha 透明（不挂背板，启动也不挂），聚焦瞬间挂
            // Acrylic 真磨砂、失焦即刻撤回——透明不绑定焦点可常驻，磨砂是 DWM 焦点绑定材质
            // 只做聚焦点睛；启动后窗口获得首焦的 Focused(true) 事件自动完成首次挂载
            if let WindowEvent::Focused(focused) = event {
                #[cfg(target_os = "windows")]
                {
                    // DWMSBT_TRANSIENTWINDOW = 3（Acrylic）；DWMSBT_NONE = 1（无背板，非 0——
                    // 0 是 DWMSBT_AUTO，会让 DWM 自行决定材质）
                    let kind: u32 = if *focused { 3 } else { 1 };
                    if let Err(err) = set_system_backdrop(window, kind) {
                        diag::log(&format!("焦点联动背板切换失败：{err}"));
                    }
                }
                // 分态纱浓度联动（用户定案：磨砂态 0% 纱、透明态 30% 纱）——前端监听本事件
                // 切换 focused class；发送失败落日志不吞（材质降级为常纱态，功能不受损）
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
            // 玻璃效果不在此挂载（PL011 定案）：启动默认纯 alpha 透明（不设背板），聚焦瞬间
            // 由 on_window_event 的 Focused 分支挂 DWM Acrylic 背板、失焦撤回——见
            // set_system_backdrop 与焦点联动注释
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
