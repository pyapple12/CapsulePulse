//! 托盘右键菜单窗（PATCH008）：自绘 HTML 菜单替代原生 Win32 菜单（照 CT PL021 逻辑）。
//! 本模块只放**纯逻辑**（锚定计算）与**守候线程**（点外收起）+ 锚点状态；建窗在 `lib.rs`
//! （与托盘 hover 窗同段），窗口尺寸由前端量内容后经 `tray_menu_resize` 命令重设。
//!
//! 锚定规则（照 CT `on_tray_right_button` 三段）：左缘 = 图标水平中轴（向右展开）、默认图标
//! 上方（间距 [`crate::TRAY_ANCHOR_MARGIN`]）、屏上缘放不下翻下方、屏右缘放不下整体左移贴边。
//!
//! **窗口调用一律锁外**：Tauri 非主线程窗口调用同步等待主线程应答，持锁调用 = 与主线程互等
//! 死锁（CT PL020.3 两轮实证）——本模块锁内只做纯数据读写，窗口查询/动作全在锁外。

use std::sync::Mutex;
use std::time::Duration;

use tauri::Manager;

use crate::TRAY_ANCHOR_MARGIN;

/// 菜单窗创建期初始宽（逻辑 px；真实尺寸由前端量内容后经 `tray_menu_resize` 重设）
pub const WINDOW_WIDTH: f64 = 160.0;
/// 菜单窗创建期初始高（逻辑 px；同上）
pub const HEIGHT_MIN: f64 = 24.0;
/// 点外收起守候步进周期（ms）
const WATCH_STEP_MS: u64 = 100;

/// 菜单锚点（托盘图标矩形，物理坐标 x / y / 宽 / 高）——右键时记录；`None` = 本会话未右键过
static MENU_ANCHOR: Mutex<Option<(i32, i32, i32, i32)>> = Mutex::new(None);

/// 记录托盘图标矩形锚点（右键事件路径调用；锁中毒静默跳过——锚点缺失只影响定位，不阻断）。
pub fn set_anchor(icon: (i32, i32, i32, i32)) {
    if let Ok(mut slot) = MENU_ANCHOR.lock() {
        *slot = Some(icon);
    }
}

/// 取锚点副本（锁内只拷值；窗口调用一律锁外）。
pub fn anchor() -> Option<(i32, i32, i32, i32)> {
    MENU_ANCHOR.lock().ok().and_then(|slot| *slot)
}

/// 菜单窗锚定（纯函数，单测直测；照 CT 三段规则）。
///
/// # 参数
/// `icon` 托盘图标矩形（物理 x/y/宽/高）；`size` 窗尺寸（物理 宽/高）；`screen` 显示器矩形
/// （物理 x/y/宽/高；`None` = 取不到 ⇒ 跳过屏缘钳制，照 CT）。
///
/// # 返回
/// 窗左上角物理坐标 `(x, y)`。
pub fn anchor_menu(
    icon: (i32, i32, i32, i32),
    size: (u32, u32),
    screen: Option<(i32, i32, i32, i32)>,
) -> (i32, i32) {
    let (ix, iy, iw, ih) = icon;
    let (sw, sh) = (size.0 as i32, size.1 as i32);
    let mut x = ix + iw / 2; // 左缘 = 图标水平中轴（向右展开）
    let mut y = iy - sh - TRAY_ANCHOR_MARGIN; // 默认图标上方
    if y < 0 {
        y = iy + ih + TRAY_ANCHOR_MARGIN; // 任务栏在顶部 ⇒ 翻下方
    }
    if let Some((sx, _sy, screen_w, _screen_h)) = screen {
        let right = sx + screen_w;
        if x + sw > right - TRAY_ANCHOR_MARGIN {
            x = right - TRAY_ANCHOR_MARGIN - sw; // 屏右缘放不下 ⇒ 整体左移贴边
        }
    }
    (x, y)
}

/// 按锚点重定位菜单窗（尺寸变化后调用；锚点未记录则跳过——此时窗也从未显示过）。
pub fn reanchor_menu(window: &tauri::WebviewWindow) {
    let Some(icon) = anchor() else {
        return;
    };
    let Ok(size) = window.outer_size() else {
        return;
    };
    let screen = window.current_monitor().ok().flatten().map(|monitor| {
        let pos = monitor.position();
        let sz = monitor.size();
        (pos.x, pos.y, sz.width as i32, sz.height as i32)
    });
    let (x, y) = anchor_menu(icon, (size.width, size.height), screen);
    if let Err(err) = window.set_position(tauri::PhysicalPosition::new(x, y)) {
        crate::warn_diag(&format!("菜单窗定位失败：{err}"));
    }
}

/// 菜单窗"点外收起"守候（PATCH008.1；照 CT `spawn_menu_watch`）：100ms 步进——起步**先等弹出
/// 那次右键松开**（立即判定会"弹出即收"，照 CT 宽限）；此后任一左/右键按下且光标不在菜单窗内
/// = 点外 ⇒ 收窗退出；菜单窗已不可见（动作路径已收）⇒ 自灭退出。**窗口查询锁外**（死锁铁律）。
pub fn spawn_menu_watch(app: &tauri::AppHandle) {
    let handle = app.clone();
    std::thread::spawn(move || {
        // 宽限：等弹出用的右键松开再开始点外判定（轮询松手，不抢焦点）
        while crate::win_input::right_down() {
            std::thread::sleep(Duration::from_millis(50));
        }
        loop {
            std::thread::sleep(Duration::from_millis(WATCH_STEP_MS));
            let Some(window) = handle.get_webview_window("tray-menu") else {
                return;
            };
            if !window.is_visible().unwrap_or(false) {
                return; // 菜单项动作已收起（命令路径）
            }
            let clicked_outside = (crate::win_input::left_down() || crate::win_input::right_down())
                && !crate::cursor_in_window(&window);
            if clicked_outside {
                if let Err(err) = window.hide() {
                    crate::warn_diag(&format!("菜单窗点外收起失败：{err}"));
                }
                return;
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 图标矩形固定样例（物理）：x=1000 / y=1400 / 24×24；屏 = (0,0) 起 1920×1080。
    fn icon() -> (i32, i32, i32, i32) {
        (1000, 1400, 24, 24)
    }

    /// 屏矩形（物理）
    fn screen() -> Option<(i32, i32, i32, i32)> {
        Some((0, 0, 1920, 1080))
    }

    /// 默认：左缘 = 图标中轴（1000 + 24/2）、顶部 = 图标顶 − 窗高 − 间距。
    #[test]
    fn anchors_above_at_icon_center() {
        let (x, y) = anchor_menu(icon(), (160, 200), screen());
        assert_eq!(x, 1012, "左缘 = 图标水平中轴（向右展开）");
        assert_eq!(y, 1400 - 200 - 8, "默认图标上方（留间距）");
    }

    /// 屏上缘放不下（y < 0）⇒ 翻到图标下方（间距同上）。
    #[test]
    fn flips_below_when_no_room_above() {
        let (_, y) = anchor_menu((1000, 0, 24, 24), (160, 200), screen());
        assert_eq!(y, 24 + 8, "图标贴屏顶 ⇒ 翻下方");
    }

    /// 屏右缘放不下 ⇒ 整体左移贴边（右缘同样留间距）。
    #[test]
    fn clamps_to_screen_right() {
        let (x, _) = anchor_menu((1900, 1400, 24, 24), (160, 200), screen());
        assert_eq!(x, 1920 - 8 - 160, "超屏右缘 ⇒ 左移贴边");
    }

    /// 取不到屏（None）⇒ 不做屏缘钳制（照 CT：取不到即不钳）。
    #[test]
    fn no_screen_skips_clamp() {
        let (x, _) = anchor_menu((1900, 1400, 24, 24), (160, 200), None);
        assert_eq!(x, 1900 + 12);
    }
}
