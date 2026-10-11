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
//! 系统背板（`DWMWA_SYSTEMBACKDROP_TYPE`）**未启用**：CP 玻璃观感为纯 CSS 分态纱
//! （PL021.4 定案，恒纯 alpha 透明窗），不挂 OS 材质；将来若做"聚焦真磨砂"再在本文件补
//! `set_focused_backdrop`。
//! 失败落日志维持前态——纯装饰层不阻断主流程（容错白名单 PATCH001.3 同款纪律，下次调用自愈）。

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
            eprintln!("窗口激活描边隐藏失败 hr={hr:#x}（维持前态）");
            crate::diag::log(&format!("窗口激活描边隐藏失败 hr={hr:#x}"));
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
            eprintln!("窗口句柄获取失败（系统圆角未设置）");
            return;
        };
        let pref = DWMWCP_ROUND;
        let hr = unsafe {
            DwmSetWindowAttribute(hwnd.0 as isize, DWMWA_WINDOW_CORNER_PREFERENCE, &pref, 4)
        };
        if hr != 0 {
            eprintln!("窗口系统圆角设置失败 hr={hr:#x}（Win10 无此属性，属预期；忽略）");
        }
    }
    #[cfg(not(target_os = "windows"))]
    let _ = window;
}
