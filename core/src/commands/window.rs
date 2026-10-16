//! 窗口级命令：双击标题缩回托盘（与 Alt+F4 / 托盘显隐同语义——隐藏到托盘，真退走托盘菜单）；
//! 板开合驱动的系统背板切换（PL029）。

use tauri::{AppHandle, Manager};

use crate::hide_main_window;

/// 缩回托盘（双击标题触发）：复用 lib 的隐藏漏斗（含位置记忆）。
/// 失败上抛文案供前端可见反馈——严格抛错主线（CT 同款命令只打日志、无用户可见反馈，CP 不沿）。
#[tauri::command]
pub fn main_hide_to_tray(app: AppHandle) -> Result<(), String> {
    hide_main_window(&app)
}

/// 板开合驱动的系统背板（PL029）：`open = true` 挂系统亚克力、`false` 摘回纯透明。
/// 由前端在"板开合 + 窗口聚焦"变化时调用（失焦必须传 `false`——DWM 材质焦点绑定）。
/// **返回 `()` 不抛错**：材质是纯装饰层，失败仅落诊断日志、维持前态（容错白名单 PL029）。
#[tauri::command]
pub fn set_board_backdrop(app: AppHandle, open: bool) {
    let Some(window) = app.get_webview_window("main") else {
        crate::warn_diag("主窗口缺失（系统背板未切换）");
        return;
    };
    crate::glass_backdrop::set_backdrop(&window, open);
}
