//! 窗口框架层（Windows 专属）：DWM 属性级 FFI（`extern dwmapi` 直连，零新依赖）。
//! 用途（`lib.rs` setup 一次性整定）：
//! ① **隐藏 Win11 激活描边**（`DWMWA_BORDER_COLOR = NONE`）：聚焦态 DWM 会给活动窗口画一圈
//!    系统强调色描边，在透明无边框窗上表现为"窗口外面一圈蓝框"（2026-10-11 用户截图实证——
//!    纯 CSS 撤掉玻璃描边后蓝线仍在，故判定绘制方为 OS 而非页面）。
//! ② **补系统圆角**（`DWMWA_WINDOW_CORNER_PREFERENCE = ROUND`）：`tauri.conf.json` 的
//!    `shadow: false` 关掉 Tauri 给无边框窗的投影后，**隐式圆角随之丢失**——Tauri 文档原文
//!    "`shadow: true` will make undecorated window have a 1px white border, and on Windows 11,
//!    it will have a rounded corners"；不显式补回则两个上角露出窗口方角
//!    （移植 CapsuleTODO `e8d049b`，2026-10-11 同款实测）。
//! 系统背板（`DWMWA_SYSTEMBACKDROP_TYPE`）**按板开合启用**（PL029，2026-10-16）：
//! 平时保持纯 alpha 透明（现状）；**打开统计板 / 设置板且窗口聚焦时**挂系统亚克力
//! （`DWMSBT_TRANSIENTWINDOW`），收板或失焦立即摘回 `DWMSBT_NONE`——观感参照 CapsuleTODO
//! 的焦点两态实现（CT `set_focused_backdrop`），差异仅在触发口径（CT = 焦点，CP = 板开合 + 焦点）。
//! **失焦必须主动摘**：DWM 材质是焦点绑定的，不摘会退回不透明灰板（PL010 实证登记，
//! WebView2 无等价 API 可拦）。
//! 失败落日志维持前态——纯装饰层不阻断主流程（容错白名单 PL029 同款纪律，下次切换自愈）。

/// `DWMWA_BORDER_COLOR`：窗口边框色属性（Win11 激活描边）。
#[cfg(target_os = "windows")]
const DWMWA_BORDER_COLOR: u32 = 34;

/// `DWMWA_COLOR_NONE`：隐藏边框（激活态不再画强调色描边）。
#[cfg(target_os = "windows")]
const DWMWA_COLOR_NONE: u32 = 0xFFFF_FFFE;

/// `DWMWA_WINDOW_CORNER_PREFERENCE`：窗口圆角偏好属性（Win11）。
#[cfg(target_os = "windows")]
const DWMWA_WINDOW_CORNER_PREFERENCE: u32 = 33;

/// `DWMWCP_ROUND`：系统标准圆角（半径随系统主题）。
#[cfg(target_os = "windows")]
const DWMWCP_ROUND: u32 = 2;

/// `DWMWA_SYSTEMBACKDROP_TYPE`：系统背板档位属性（Win11 22H2+；PL029）。
#[cfg(target_os = "windows")]
const DWMWA_SYSTEMBACKDROP_TYPE: u32 = 38;

/// `DWMSBT_NONE`：无背板（窗口纯 alpha 透明基线）。
#[cfg(target_os = "windows")]
const DWMSBT_NONE: u32 = 1;

/// `DWMSBT_TRANSIENTWINDOW`：系统亚克力档（瞬态窗观感，实时模糊背后内容）。
#[cfg(target_os = "windows")]
const DWMSBT_TRANSIENTWINDOW: u32 = 3;

#[cfg(target_os = "windows")]
#[link(name = "dwmapi")]
extern "system" {
    /// DWM 窗口属性写入（成功返回 0，失败返回非零 HRESULT）。
    fn DwmSetWindowAttribute(hwnd: isize, attr: u32, value: *const u32, size: u32) -> i32;
}

/// 隐藏 Win11 激活描边（此后窗口聚焦不再画系统强调色边框）。
/// 非 Windows 平台为空实现（同一签名，调用点保持无平台分支）。失败落日志维持前态。
pub fn hide_active_border(window: &tauri::WebviewWindow) {
    #[cfg(target_os = "windows")]
    {
        let Ok(hwnd) = window.hwnd() else {
            crate::diag::log("窗口句柄获取失败（激活描边未隐藏）");
            return;
        };
        let none = DWMWA_COLOR_NONE;
        let hr = unsafe { DwmSetWindowAttribute(hwnd.0 as isize, DWMWA_BORDER_COLOR, &none, 4) };
        if hr != 0 {
            crate::warn_diag(&format!("窗口激活描边隐藏失败 hr={hr:#x}（维持前态）"));
        }
    }
    #[cfg(not(target_os = "windows"))]
    let _ = window;
}

/// 补系统圆角（Win11）：DWM 把窗口矩形切圆，与 CSS `--radius-board: 8px` 对齐——
/// `shadow: false` 关投影后隐式圆角丢失，不补则两个上角露出窗口方角。
/// 非 Windows 为空实现。**Win10 无此属性、调用失败属预期**：只落 dev 控制台、不落 diag
/// （避免 Win10 每次启动写一条常驻噪音）。
pub fn apply_round_corners(window: &tauri::WebviewWindow) {
    #[cfg(target_os = "windows")]
    {
        let Ok(hwnd) = window.hwnd() else {
            crate::console("窗口句柄获取失败（系统圆角未设置）");
            return;
        };
        let pref = DWMWCP_ROUND;
        let hr = unsafe {
            DwmSetWindowAttribute(hwnd.0 as isize, DWMWA_WINDOW_CORNER_PREFERENCE, &pref, 4)
        };
        if hr != 0 {
            crate::console(&format!(
                "窗口系统圆角设置失败 hr={hr:#x}（Win10 无此属性，属预期；忽略）"
            ));
        }
    }
    #[cfg(not(target_os = "windows"))]
    let _ = window;
}

/// 系统背板开关（PL029）：`on = true` 挂系统亚克力（`DWMSBT_TRANSIENTWINDOW`）、
/// `false` 摘回纯透明（`DWMSBT_NONE`）。由前端在"板开合 + 窗口聚焦"变化时驱动。
/// **失焦必须传 `false`**（DWM 材质焦点绑定，不摘会退回不透明灰板）。
/// 非 Windows 为空实现（无系统背板，静默 no-op）。失败落日志维持前态、下次切换自愈。
pub fn set_backdrop(window: &tauri::WebviewWindow, on: bool) {
    #[cfg(target_os = "windows")]
    {
        let Ok(hwnd) = window.hwnd() else {
            // FIX008.15：原为 eprintln + warn_diag 双调（同一句打两遍 console）⇒ 收敛为一次
            crate::warn_diag("窗口句柄获取失败（系统背板未切换）");
            return;
        };
        let backdrop = if on {
            DWMSBT_TRANSIENTWINDOW
        } else {
            DWMSBT_NONE
        };
        let hr = unsafe {
            DwmSetWindowAttribute(hwnd.0 as isize, DWMWA_SYSTEMBACKDROP_TYPE, &backdrop, 4)
        };
        if hr != 0 {
            // FIX008.15：原为 eprintln + warn_diag 双调（同一句打两遍 console）⇒ 收敛为一次
            crate::warn_diag(&format!("系统背板切换失败 hr={hr:#x}（维持前态）"));
        }
    }
    #[cfg(not(target_os = "windows"))]
    let _ = (window, on);
}
