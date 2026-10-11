//! 窗口级命令：双击标题缩回托盘（与 Alt+F4 / 托盘显隐同语义——隐藏到托盘，真退走托盘菜单）。

use tauri::AppHandle;

use crate::hide_main_window;

/// 缩回托盘（双击标题触发）：复用 lib 的隐藏漏斗（含位置记忆）。
/// 失败上抛文案供前端可见反馈——严格抛错主线（CT 同款命令只打日志、无用户可见反馈，CP 不沿）。
#[tauri::command]
pub fn main_hide_to_tray(app: AppHandle) -> Result<(), String> {
    hide_main_window(&app)
}
