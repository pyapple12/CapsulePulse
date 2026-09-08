//! CapsulePulse 应用库：业务模块与 Tauri 应用装配。
//! main.rs 只做薄入口（调 [`run`]）；模块逻辑收在本库——lib 形态下 pub 项即公开 API，
//! 也是 `cargo test` 与后续命令层（commands.rs）的承载处。
//! 层边界：业务纯逻辑（session.rs 等）平铺于本 src 下且禁 import tauri，
//! 装配层只在本文件——纯逻辑可脱离窗口 cargo test 直测。
//! 玻璃效果：Windows 实机走 Acrylic（PL001 阶段 B 已判定通过）；macOS/Linux 延后（y.problems.md #1）。

pub mod commands;
pub mod session;

use std::sync::Mutex;

use tauri::Manager;

use crate::commands::SessionHandle;
use crate::session::WorkSession;

/// 装配并运行 Tauri 应用：窗口属性由 tauri.conf.json 声明（透明无边框 360×480），玻璃效果在 setup 挂载。
pub fn run() {
    tauri::Builder::default()
        .manage(SessionHandle(Mutex::new(WorkSession::default())))
        .invoke_handler(tauri::generate_handler![
            commands::session_start,
            commands::session_pause,
            commands::session_resume,
            commands::session_restart,
            commands::session_status
        ])
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
            Ok(())
        })
        .run(tauri::generate_context!())
        .unwrap_or_else(|err| {
            eprintln!("CapsulePulse 启动失败：{err}");
            std::process::exit(1);
        });
}
