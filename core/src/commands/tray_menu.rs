//! 托盘菜单窗命令（PATCH008）：自绘 HTML 菜单窗的尺寸重设与（`.5` 起）菜单项动作分发。
//! 尺寸与 hover 窗同纪律——宽高由前端量内容给出（菜单窗无固定宽）、经前端 `snap4` 向上取
//! 4 的倍数（125% 下逻辑物理双整数）；设完**重锚菜单窗**。窗口几何统一走 Rust 命令
//! （JS `setSize` 需 ACL `allow-set-size`，会静默拒绝——CT 实证）。

use tauri::{AppHandle, Manager};

use super::{clamp_side, CommandError, SIDE_MAX, SIDE_MIN};
use crate::MENU_ACTIONS;

/// 菜单项动作统一分发（PATCH008.5；照 CT `tray_menu_action`）：**先收菜单窗**（点项必收，
/// 原生菜单同款语义），再按 action 执行——分发语义复用 `crate::handle_action`（零逻辑重复）。
/// action 全集见 `crate::MENU_ACTIONS`（**单一来源**：FIX006.18 起校验与分发同源，
/// 不再各写一份白名单）。未知动作**严格报错**（非法输入不静默）。
#[tauri::command]
pub fn tray_menu_action(action: String, app: AppHandle) -> Result<(), CommandError> {
    if !MENU_ACTIONS.contains(&action.as_str()) {
        return Err(CommandError::UnknownMenuAction(action));
    }
    if let Some(menu_win) = app.get_webview_window("tray-menu") {
        if let Err(err) = menu_win.hide() {
            crate::warn_diag(&format!("菜单窗收起失败：{err}"));
        }
    }
    crate::handle_action(&app, &action);
    Ok(())
}

/// 重设菜单窗尺寸（PATCH008.3）：`width`/`height` 由前端量内容 + 读 CSS chrome 得出并已
/// `snap4`（向上取 4 的倍数）。设完**重锚菜单窗**（按锚定规则重算落位，屏缘钳制同步生效）。
/// 失败仅落日志（装饰层纪律，容错白名单同款）。
#[tauri::command]
pub fn tray_menu_resize(app: AppHandle, width: f64, height: f64) -> Result<(), CommandError> {
    // FIX006.17：后端最后一道夹取（前端量内容算错的护栏）
    let (width, height) = (
        clamp_side(width, SIDE_MIN, SIDE_MAX),
        clamp_side(height, SIDE_MIN, SIDE_MAX),
    );
    let Some(window) = app.get_webview_window("tray-menu") else {
        return Ok(()); // 窗不存在（退出收尾）：跳过
    };
    if let Err(err) = window.set_size(tauri::LogicalSize::new(width, height)) {
        crate::warn_diag(&format!("菜单窗尺寸设置失败：{err}"));
        return Ok(());
    }
    crate::menu::reanchor_menu(&window);
    Ok(())
}
