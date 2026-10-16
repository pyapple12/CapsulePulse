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
pub mod hotkey;
pub mod hover;
pub mod menu;
pub mod period;
pub mod reminder;
pub mod session;
pub mod settings;
pub mod snap;
pub mod storage;
pub mod workday;

mod diag;
mod glass_backdrop;
mod paths;

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use crate::commands::reminder::{HotkeyAction, PrefKind};
use crate::commands::{AppContext, AutoOutWatch};
use crate::hover::{Phase, WatchAction};
use crate::reminder::ReminderFire;
use crate::session::WorkSession;
use crate::settings::{ReminderSettings, WindowPos};
use crate::storage::Storage;
use crate::workday::WorkdayState;
use tauri::{
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, PhysicalPosition, WindowEvent,
};

/// 运行期警告双写：eprintln（dev 控制台可见）+ diag 落档（release GUI 无 stderr，
/// 文件日志是唯一落点——FIX003.8）。仅用于失败不中断主流程的容错路径。
pub(crate) fn warn_diag(msg: &str) {
    eprintln!("{msg}");
    diag::log(msg);
}

// —— 贴边吸附状态机（PL028.3，抄 CT PL017.4「A 案松手吸附」）——
// 松手判定（CT 三轮实测修正）：**左键状态为准，Moved 静默为辅**——鼠标悬停 / 慢速拖动
// 都会造成 Moved 静默超过阈值，仅凭静默会在拖动中误吸（"还没松手就吸走又回到手里"）；
// 左键按住 = 必在拖动，绝不吸附。启动恢复的单次程序性 Moved 左键未按 ⇒ 不置位 ⇒ 不吸。
/// 拖动静默阈值（ms）：置位判定与松手判定共用（两处必须同值，否则吸附手感错乱）。
const DRAG_QUIET_MS: u64 = 150;
static LAST_MOVED_MS: AtomicU64 = AtomicU64::new(0);
static DRAG_ACTIVE: AtomicBool = AtomicBool::new(false);
static DRAG_SNAP_PENDING: AtomicBool = AtomicBool::new(false);

#[cfg(target_os = "windows")]
mod win_input {
    /// 左键虚拟键码
    const VK_LBUTTON: i32 = 0x01;
    /// 右键虚拟键码（PATCH008：菜单窗点外收起判据与弹出宽限共用）
    const VK_RBUTTON: i32 = 0x02;
    /// `GetAsyncKeyState` 高位 = 物理 / 合成按下中
    const PRESSED: u16 = 0x8000;

    /// 鼠标左键是否按下
    pub fn left_down() -> bool {
        unsafe { (GetAsyncKeyState(VK_LBUTTON) as u16) & PRESSED != 0 }
    }

    /// 鼠标右键是否按下
    pub fn right_down() -> bool {
        unsafe { (GetAsyncKeyState(VK_RBUTTON) as u16) & PRESSED != 0 }
    }

    #[link(name = "user32")]
    extern "system" {
        fn GetAsyncKeyState(v_key: i32) -> i16;
    }
}

/// 非 Windows 空桩（调用点零 cfg）：无 Win32 键态查询，恒"未按下"——吸附状态机退化为
/// 仅靠 Moved 静默判定（macOS/Linux 行为面随 [problems#1] 一并实机验证）。
#[cfg(not(target_os = "windows"))]
mod win_input {
    /// 鼠标左键是否按下（非 Windows 空桩：恒 false）
    pub fn left_down() -> bool {
        false
    }

    /// 鼠标右键是否按下（非 Windows 空桩：恒 false）
    pub fn right_down() -> bool {
        false
    }
}

/// 当前时刻（Unix 毫秒）：吸附状态机的时间源（只用于算间隔，不要求单调性）。
fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// 贴边吸附判定与落位（PL028.3）：读开关（锁中毒跳过——吸附是体验增强层，容错白名单
/// 登记项）→ 取所在显示器 → [`snap::snap_position`] 算修正位 → **与当前位不同才**
/// `set_position`（幂等防环：吸附位仍在阈值内，不判等会 Moved→snap→Moved 死循环）。
/// 坐标基准 = **视觉（client）矩形**：无边框窗可能带不可见 resize 边，outer 矩形直接
/// 判定会视觉偏移；用 inner_position/inner_size 取视觉矩形参与判定，落位换算回 outer
/// 坐标（偏移 = inner − outer 自动吸收缩放与隐形边——CT 截图像素实测结论）。
fn snap_if_needed(window: &tauri::Window) {
    let snap_on = window
        .app_handle()
        .state::<AppContext>()
        .settings
        .lock()
        .map(|s| s.snap_to_edge)
        .unwrap_or_else(|err| {
            warn_diag(&format!("贴边吸附：设置锁读取失败，跳过吸附：{err:?}"));
            false
        });
    if !snap_on {
        return;
    }
    let Ok(Some(monitor)) = window.current_monitor() else {
        return;
    };
    let Ok(outer) = window.outer_position() else {
        return;
    };
    let Ok(inner) = window.inner_position() else {
        return;
    };
    let Ok(vsize) = window.inner_size() else {
        return;
    };
    let mp = monitor.position();
    let ms = monitor.size();
    let (sx, sy) = snap::snap_position(
        snap::RectI32 {
            x: inner.x,
            y: inner.y,
            w: vsize.width as i32,
            h: vsize.height as i32,
        },
        snap::RectI32 {
            x: mp.x,
            y: mp.y,
            w: ms.width as i32,
            h: ms.height as i32,
        },
    );
    // 视觉目标 → outer 目标（隐形边偏移 = inner − outer）
    let (tx, ty) = (sx - (inner.x - outer.x), sy - (inner.y - outer.y));
    if (tx, ty) != (outer.x, outer.y) {
        if let Err(err) = window.set_position(PhysicalPosition::new(tx, ty)) {
            warn_diag(&format!("贴边吸附落位失败：{err}"));
        }
    }
}

// —— 托盘 hover 今日概览窗（PL028.1；PATCH007 收口）——
// 显隐 = **单一常驻守候线程 + 三态状态机**（PATCH007.2，抄 CT `tray.rs` 形态）：`Enter` 只记录
// 图标矩形与相位（**不直接 show**），守候线程 100ms 步进结算 show/hide（**唯一调用者** ⇒
// 杜绝多线程换代的孤儿窗）；停留阈值防划过误触、离开两区域连续 4 拍（400ms）才收起——CT
// PL019.1 实证「`Leave` 事件慢速移出会丢失」，故**不依赖 `Leave`**。显示中按 1 秒节拍重推
// 概览（PATCH007.6：悬停期实时刷新）。光标判据仅 Windows 有（非 Windows 退化为事件驱动）。
/// 托盘窗（hover / 菜单）与托盘图标的锚定间距（物理 px；PATCH008 起两窗共用单一来源）
pub(crate) const TRAY_ANCHOR_MARGIN: i32 = 8;
/// 守候线程退出口：悬浮窗连续缺席拍数上限（满即自灭，防退出收尾空转）
const HOVER_ABSENT_LIMIT: u32 = 50;

/// 守候状态（唯一事实源）：相位 + 托盘图标矩形锚点（物理坐标 x / y / 宽 / 高；`None` = 启动后
/// 未 hover 过）。事件处理（`Enter` / 右键）与守候线程经本锁读写；**锁内只做纯数据读写与
/// `GetCursorPos` 纯 FFI**——任何窗口调用（`is_visible` / `outer_position` / `show` / `hide` /
/// `set_position`）**一律锁外**（CT PL020.3 两轮死锁实证：Tauri 2 非主线程窗口调用同步等待
/// 主线程应答，持锁调用 = 与主线程互等死锁）。
struct HoverWatch {
    /// 守候相位（见 [`hover::Phase`]）
    phase: Phase,
    /// 托盘图标矩形（物理坐标）
    anchor: Option<(i32, i32, i32, i32)>,
}

static HOVER_WATCH: Mutex<HoverWatch> = Mutex::new(HoverWatch {
    phase: Phase::Idle,
    anchor: None,
});

#[cfg(target_os = "windows")]
mod win_cursor {
    /// Win32 POINT（物理像素）
    #[repr(C)]
    struct Point {
        x: i32,
        y: i32,
    }

    /// 当前光标位置（物理像素；取失败回 None）
    pub fn cursor_pos() -> Option<(i32, i32)> {
        let mut pt = Point { x: 0, y: 0 };
        let ok = unsafe { GetCursorPos(&mut pt) };
        (ok != 0).then_some((pt.x, pt.y))
    }

    #[link(name = "user32")]
    extern "system" {
        fn GetCursorPos(point: *mut Point) -> i32;
    }
}

/// 非 Windows 空桩（调用点零 cfg）：无光标查询 ⇒ 判据退化为"纯事件驱动"
/// （Enter 延迟后显示、Leave 延迟后收起），随 [problems#1] 跨平台实机一并验证。
#[cfg(not(target_os = "windows"))]
mod win_cursor {
    /// 当前光标位置（非 Windows 空桩：恒 None = 无查询能力）
    pub fn cursor_pos() -> Option<(i32, i32)> {
        None
    }
}

/// 光标是否在托盘图标矩形内：`Some(true/false)` 为判定结果，`None` = 本平台无查询能力
/// （调用方按"不在"处理 ⇒ 退化为纯事件驱动语义）。锚点由调用方从锁内拷出（本函数不取锁）。
fn cursor_in_tray(anchor: Option<(i32, i32, i32, i32)>) -> Option<bool> {
    let Some((x, y, w, h)) = anchor else {
        return Some(false);
    };
    win_cursor::cursor_pos().map(|(cx, cy)| cx >= x && cx < x + w && cy >= y && cy < y + h)
}

/// 光标是否在悬浮窗矩形内（PATCH007.2，抄 CT `cursor_in_window`）：**物理位置兜底**——窗
/// hide/show 切换期 webview 的 `mouseenter` 可能丢失，物理位置不依赖事件。非 Windows 恒
/// `false`（无查询能力 ⇒ 卡片不因"光标在卡上"而保持，随 [problems#1] 一并验证）。
pub(crate) fn cursor_in_window(window: &tauri::WebviewWindow) -> bool {
    let (Some((cx, cy)), Ok(pos), Ok(size)) = (
        win_cursor::cursor_pos(),
        window.outer_position(),
        window.outer_size(),
    ) else {
        return false;
    };
    cx >= pos.x && cx < pos.x + size.width as i32 && cy >= pos.y && cy < pos.y + size.height as i32
}

/// 菜单窗此刻是否开着（PATCH008.6）：hover 守候的 `Suppressed` 锁死事实源。
/// **只允许锁外调用**：`is_visible` 走事件循环同步等待主线程应答，锁内调用 = 互挂死锁。
fn menu_is_open(app: &AppHandle) -> bool {
    app.get_webview_window("tray-menu")
        .map(|window| window.is_visible().unwrap_or(false))
        .unwrap_or(false)
}

/// 悬浮窗锚定（PATCH007.2，抄 CT 预览窗几何）：右缘对齐托盘图标右缘；默认图标上方，屏上缘
/// 放不下则翻到下方（任务栏在顶部场景）。锚点由调用方从锁内拷出（本函数不取锁、窗口调用锁外）。
fn anchor_tray_hover(window: &tauri::WebviewWindow, anchor: Option<(i32, i32, i32, i32)>) {
    let Some((x, y, w, h)) = anchor else {
        return;
    };
    let Ok(size) = window.outer_size() else {
        return;
    };
    let px = x + w - size.width as i32; // 右对齐托盘图标
    let mut py = y - size.height as i32 - TRAY_ANCHOR_MARGIN; // 默认图标上方
    if py < 0 {
        py = y + h + TRAY_ANCHOR_MARGIN; // 翻到下方
    }
    if let Err(err) = window.set_position(PhysicalPosition::new(px, py)) {
        warn_diag(&format!("hover 窗定位失败：{err}"));
    }
}

/// 重锚悬浮窗（PATCH007.8）：窗高变化后重算位置（**下缘保持"托盘顶 − 间距"、顶缘移动** ——
/// CT `reanchor_to_tray` 同款，用户定案的收缩方向）。锚点从锁内拷出（窗口调用锁外）。
pub(crate) fn reanchor_tray_hover(window: &tauri::WebviewWindow) {
    let anchor = match HOVER_WATCH.lock() {
        Ok(watch) => watch.anchor,
        Err(_) => None,
    };
    anchor_tray_hover(window, anchor);
}

/// 推送一次概览（PATCH007.6 单一出口）：聚合 DTO → 按行数设窗高 → `emit_to` 推给悬浮窗。
/// **每次悬停首帧**与**悬停期每秒节拍**共用本函数（同一单一口径）。任一步失败落日志并返回
/// `false`（装饰层不阻断主流程——容错白名单登记项；首帧失败则不显示空壳）。
fn push_hover_summary(app: &AppHandle) -> bool {
    let Some(window) = app.get_webview_window("tray-hover") else {
        return false;
    };
    let ctx = app.state::<AppContext>();
    let summary = match commands::hover::hover_summary_inner(&ctx) {
        Ok(summary) => summary,
        Err(err) => {
            warn_diag(&format!("hover 概览聚合失败（跳过本拍）：{err}"));
            return false;
        }
    };
    // 窗尺寸**不在此设**（PATCH007.8）：宽固定于 `hover::WINDOW_WIDTH`（创建期已设）、高由
    // 前端量内容后经 `tray_hover_resize` 命令重设 —— 单一归属，避免两处打架
    if let Err(err) = window.emit_to("tray-hover", "hover-update", &summary) {
        warn_diag(&format!("hover 概览推送失败：{err}"));
        return false;
    }
    true
}

/// 收起悬浮窗（失败仅日志）。
fn hide_tray_hover(app: &AppHandle) {
    let Some(window) = app.get_webview_window("tray-hover") else {
        return;
    };
    if let Err(err) = window.hide() {
        warn_diag(&format!("hover 窗收起失败：{err}"));
    }
}

/// 托盘右键（PATCH008.6）：先收悬浮窗 + **转 `Suppressed` 锁死**（菜单窗即将弹出，照 CT）——
/// 守候线程据菜单窗可见性维持锁死，菜单收起后回 `Idle`（需新 `Enter` 重新移入图标才再出卡）。
fn dismiss_tray_hover(app: &AppHandle) {
    if let Ok(mut watch) = HOVER_WATCH.lock() {
        watch.phase = Phase::Suppressed;
    }
    hide_tray_hover(app);
}

/// 托盘右键（PATCH008.4；照 CT `on_tray_right_button`）：菜单窗已开 ⇒ 再右键 = 收窗返回；
/// 未开 ⇒ 记锚点 + **先收 hover 窗**（它恒置顶、会浮在菜单之上）+ 按锚定规则定位（含屏缘
/// 钳制，单一来源）+ 显示 + 起点外收起守候。失败仅落日志（装饰层纪律）。
fn on_tray_right_button(app: &AppHandle, rect: tauri::Rect) {
    let (tauri::Position::Physical(pos), tauri::Size::Physical(size)) = (rect.position, rect.size)
    else {
        return;
    };
    let Some(menu_win) = app.get_webview_window("tray-menu") else {
        return;
    };
    if menu_win.is_visible().unwrap_or(false) {
        if let Err(err) = menu_win.hide() {
            warn_diag(&format!("菜单窗收起失败：{err}"));
        }
        return; // 菜单开着再右键 = 收起
    }
    menu::set_anchor((pos.x, pos.y, size.width as i32, size.height as i32));
    dismiss_tray_hover(app);
    menu::reanchor_menu(&menu_win);
    if let Err(err) = menu_win.show() {
        warn_diag(&format!("菜单窗显示失败：{err}"));
        return;
    }
    menu::spawn_menu_watch(app);
}

/// 托盘图标悬停进入（PATCH007.2）：记录图标矩形 + 相位分派——`Shown` 只清 `miss` **不重武装**
/// （CT FIX006.18 连续性：光标自悬浮窗移回图标不闪没）；其余相位转 `Armed` 重新计时。
/// **显隐动作由守候线程执行**，本函数只写状态（`Leave` 不再参与判定——CT PL019.1 实证其慢速
/// 移出会丢失）。
fn on_tray_hover_enter(rect: tauri::Rect) {
    let (tauri::Position::Physical(pos), tauri::Size::Physical(size)) = (rect.position, rect.size)
    else {
        return;
    };
    let Ok(mut watch) = HOVER_WATCH.lock() else {
        return;
    };
    watch.anchor = Some((pos.x, pos.y, size.width as i32, size.height as i32));
    watch.phase = match watch.phase {
        Phase::Shown { .. } => Phase::Shown { miss: 0 },
        _ => Phase::Armed { since_ms: now_ms() },
    };
}

/// 悬浮窗守候线程（PATCH007.2 + PATCH007.6；抄 CT `spawn_preview_watcher` 形态）：100ms 步进
/// 驱动 [`hover::step_watch`]，是悬浮窗 **show/hide 的唯一调用者**（事件处理只写状态 ⇒ 杜绝
/// 多线程换代的孤儿窗）；相位为 `Shown` 时按 1 秒节拍重推概览（悬停期实时刷新，离开即停、
/// 零空转）。窗口查询与动作**全在锁外**（见 [`HoverWatch`] 注释）。退出口：窗连续缺席满
/// [`HOVER_ABSENT_LIMIT`] 拍 ⇒ 线程自灭（应用退出收尾或创建失败，不再空转）。
fn spawn_hover_watcher(app: &AppHandle) {
    let handle = app.clone();
    std::thread::spawn(move || {
        let mut absent_ticks: u32 = 0;
        let mut push_ticks: u32 = 0;
        loop {
            std::thread::sleep(Duration::from_millis(hover::WATCH_STEP_MS));
            let Some(window) = handle.get_webview_window("tray-hover") else {
                absent_ticks += 1;
                if absent_ticks >= HOVER_ABSENT_LIMIT {
                    return;
                }
                continue;
            };
            absent_ticks = 0;
            // 窗口查询（is_visible / outer_*）与动作（show / hide / set_position）全在锁外执行
            let visible = window.is_visible().unwrap_or(false);
            let in_window = visible && cursor_in_window(&window);
            // **窗口调用一律锁外**：`menu_is_open` 与 `cursor_in_window` 同属"等主线程应答"的
            // 查询——放进下面的锁块内会与主线程（`on_tray_hover_enter` / `dismiss_tray_hover`
            // 取同一把锁）互等死锁（PATCH008.6 初版即踩此坑：悬停/右键双双卡死）。
            let menu_open = menu_is_open(&handle);
            let (action, anchor, phase_after) = {
                let mut watch = match HOVER_WATCH.lock() {
                    Ok(guard) => guard,
                    Err(_) => continue, // 锁中毒：跳过本拍（守候是装饰层，不阻断主流程）
                };
                let in_tray = cursor_in_tray(watch.anchor).unwrap_or(false);
                let (next, action) = hover::step_watch(
                    watch.phase.clone(),
                    now_ms(),
                    in_tray,
                    in_window,
                    menu_open,
                    visible,
                );
                watch.phase = next.clone();
                (action, watch.anchor, next)
            };
            match action {
                WatchAction::Show => {
                    // 锁内结算与锁外执行之间有毫秒级空窗：此间右键会把相位归零并收窗
                    // ⇒ 复核"仍是 Shown"再显示（CT FIX007.17 同款竞态复核）
                    let still_shown = HOVER_WATCH
                        .lock()
                        .map(|watch| matches!(watch.phase, Phase::Shown { .. }))
                        .unwrap_or(false);
                    if !still_shown {
                        continue;
                    }
                    // PATCH007.11（与 CT 对齐）：**先锚定 + 显示**，数据推送只作尽力而为 ——
                    // CT 守候线程即 `anchor_preview + show` 直给、不把显示绑在推送成功上；我原版
                    // 多了一道"推送成功才显示"的闸门 ⇒ 推送瞬时失败就整窗不出现 = **概率性不出窗**
                    // 的根因 ✗（`push_hover_summary` 内部失败已自行落日志）。
                    anchor_tray_hover(&window, anchor);
                    if let Err(err) = window.show() {
                        warn_diag(&format!("hover 窗显示失败：{err}"));
                    }
                    push_hover_summary(&handle);
                }
                WatchAction::Hide => hide_tray_hover(&handle),
                WatchAction::None => {}
            }
            // PATCH007.6：显示中按 1 秒节拍重推（首帧已由 Show 分支推）；非显示态归零计数
            if matches!(phase_after, Phase::Shown { .. }) {
                push_ticks += 1;
                if push_ticks >= hover::PUSH_EVERY_TICKS {
                    push_ticks = 0;
                    let _ = push_hover_summary(&handle);
                }
            } else {
                push_ticks = 0;
            }
        }
    });
}

/// 显示主窗口并聚焦（还原最小化；失败逐项记日志，不中断流程）。
/// PL025.8 起自动下班待决也走此口（到点前 60s 把窗口拉到前台）。
pub(crate) fn show_main_window(app: &AppHandle) {
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
pub(crate) fn toggle_main_window(app: &AppHandle) {
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

/// 窗口偏好全量同步（PL028.3 单一同步出口）：广播 `prefs-changed`（设置面板与自绘菜单窗
/// 回显）——设置面板与托盘菜单双入口的改动都经此收口。PATCH008 起原生菜单退役，勾选态刷新
/// 由菜单窗自身的 `prefs-changed` 监听承担（本函数只剩广播）。
pub(crate) fn sync_prefs_ui(app: &AppHandle) {
    let ctx = app.state::<AppContext>();
    commands::reminder::emit_prefs_changed(app, &ctx);
}

/// 托盘窗口偏好切换（PL028.3）：读当前值取反 → 落库 + 置顶即时生效 → 双入口同步。
/// 任一步失败落日志（勾选态由下次事实源读取纠正——ctx 未变即事实未变）。
fn toggle_window_pref(app: &AppHandle, kind: PrefKind) {
    let ctx = app.state::<AppContext>();
    let current = match commands::reminder::window_pref(&ctx, kind) {
        Ok(value) => value,
        Err(err) => {
            warn_diag(&format!("托盘偏好读取失败（未切换）：{err}"));
            return;
        }
    };
    if let Err(err) = commands::reminder::set_window_pref(app, &ctx, kind, !current) {
        warn_diag(&format!("托盘偏好切换失败（未生效）：{err}"));
        return;
    }
    sync_prefs_ui(app);
}

/// 托盘菜单动作分发（按动作 id；计时切换与退出落库复用命令层自由函数）——PATCH008 起由
/// 自绘菜单窗命令 [`commands::tray_menu::tray_menu_action`] 调用（原生菜单事件分发已退役）。
pub(crate) fn handle_action(app: &AppHandle, id: &str) {
    match id {
        // 聚焦主窗（PATCH008：照 CT 语义——只显示 + 还原 + 聚焦，**不再负责隐藏**；
        // 隐藏仍走标题栏关闭 / 显隐热键）
        "focus_main" => show_main_window(app),
        "toggle_timer" => {
            let ctx = app.state::<AppContext>();
            if let Err(err) = commands::session::toggle_session(&ctx) {
                warn_diag(&format!("计时切换失败：{err}"));
            }
        }
        // PL028.3：两项窗口偏好（勾选态即当前值，点击 = 取反）
        "toggle_always_on_top" => toggle_window_pref(app, PrefKind::AlwaysOnTop),
        "toggle_snap_to_edge" => toggle_window_pref(app, PrefKind::SnapToEdge),
        "quit" => {
            let ctx = app.state::<AppContext>();
            // 位置记忆（PATCH001.4）：托盘退出走 app.exit(0)、不经 CloseRequested，此处补一次
            save_window_position(app);
            if let Err(err) = commands::session::persist_before_quit(&ctx) {
                // 容错白名单 ④：退出前落库失败仍退出（退出意图优先）；双写助手统一出口（FIX005.5）
                warn_diag(&format!("退出前落库失败（仍退出）：{err}"));
            }
            // PATCH007.4（CT PL021.5 三重闭环）：显式移除托盘图标双保险（App drop 亦会删；
            // 多窗存在时确认退出链不残留幽灵图标）；返回值 None = 图标本不存在（异常态，无错误可吞）
            drop(app.remove_tray_by_id("main-tray"));
            app.exit(0);
        }
        _ => {}
    }
}

/// 构建托盘：应用图标 + 事件接线（左键单击 = 显隐切换；右键 = 自绘菜单窗开关；悬停 = 概览卡）。
/// PATCH008 起菜单为自绘 HTML 窗（`tray-menu`），原生菜单及其勾选态刷新链已退役。
fn build_tray(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let icon = app
        .default_window_icon()
        .ok_or("缺少应用图标（tauri.conf.json bundle.icon），托盘无法构建")?;
    TrayIconBuilder::with_id("main-tray")
        .icon(icon.clone())
        .on_tray_icon_event(|tray, event| match event {
            // 左键单击 = 显隐切换
            TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } => toggle_main_window(tray.app_handle()),
            // 右键（PATCH008，照 CT「右键**按下**」）：自绘菜单窗开关（再按 = 收；内部先收 hover 窗）。
            // **必须过滤 `button_state`**：`Click` 对按下/抬起**各发一次**，不过滤会"按下弹窗、
            // 抬起又收窗" = 一闪即没（左键臂同款过滤在先，PATCH008 订正）。
            TrayIconEvent::Click {
                button: MouseButton::Right,
                button_state: MouseButtonState::Down,
                rect,
                ..
            } => on_tray_right_button(tray.app_handle(), rect),
            // PL028.1 / PATCH007.2：悬停托盘图标 ⇒ 只记录矩形与相位，显隐由守候线程结算
            // （`Leave` 不参与判定——CT PL019.1 实证其慢速移出会丢失）
            TrayIconEvent::Enter { rect, .. } => on_tray_hover_enter(rect),
            _ => {}
        })
        .build(app)?;
    Ok(())
}

/// 注册全局热键（PL028.2：组合由设置驱动，Rust 侧注册不经前端 IPC）——窗口显隐切换
/// （缺省 `Alt+Shift+S`）+ 工作/休息切换（缺省 `Alt+Shift+P`）。单条注册失败**仅落日志
/// 不阻断启动**（热键是便利层；面板里录新组合即可即时恢复——容错白名单登记项）。
fn register_global_shortcuts(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let (visible, timer) = {
        let ctx = app.state::<AppContext>();
        let settings = ctx
            .settings
            .lock()
            .map_err(|err| format!("设置锁中毒，热键未注册：{err:?}"))?;
        (
            settings.hotkey_toggle_visible.clone(),
            settings.hotkey_toggle_timer.clone(),
        )
    };
    let handle = app.handle().clone();
    for (action, combo) in [
        (HotkeyAction::ToggleVisible, visible),
        (HotkeyAction::ToggleTimer, timer),
    ] {
        if let Err(err) = commands::reminder::register_action(&handle, action, &combo) {
            warn_diag(&format!("全局热键注册失败（{combo}）：{err}"));
        }
    }
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
    // 工作日状态**初判**（PL027.6 单一事实源）：库中"最后一条打卡未配对"= 疑似在岗。
    // （PL027.4 起由启动对账三分支细化：停机 ≤10 分钟无缝续、>10 分钟按最后心跳补下班）
    // PATCH005.7：此处只是初判，**结论由紧随其后的启动对账给出**——故措辞保持中性；原措辞
    // "已恢复在岗状态"在 >10 分钟补下班那一支会先留一句与后续"补下班"相矛盾的结论（纯日志误导）。
    let workday = match storage.open_duty_clock_in() {
        Ok(Some(clock_in_at)) => {
            // 缓存中存在未闭合班：先留痕备查（双写落档），结论待启动对账
            warn_diag("启动：缓存中存在未闭合班（待对账）");
            WorkdayState::restore_on_duty(clock_in_at)
        }
        Ok(None) => WorkdayState::Off,
        Err(err) => {
            diag::log(&format!("工作日状态恢复失败：{err}"));
            eprintln!("工作日状态恢复失败：{err}");
            std::process::exit(1);
        }
    };
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
            auto_out: Mutex::new(AutoOutWatch::default()),
            settings_path,
            wall: Box::new(commands::SystemWallClock),
            recovery: Mutex::new(None),
            auto_out_notice: Mutex::new(None),
        })
        .invoke_handler(tauri::generate_handler![
            commands::workday::take_recovery_notice,
            commands::workday::take_auto_out_notice,
            commands::session::session_start,
            commands::session::session_pause,
            commands::session::session_resume,
            commands::session::session_restart,
            commands::session::session_set_run_mode,
            commands::session::session_status,
            commands::stats::session_stats,
            commands::reminder::get_settings,
            commands::reminder::set_settings,
            commands::reminder::set_hotkey,
            commands::hover::tray_hover,
            commands::hover::tray_hover_resize,
            commands::tray_menu::tray_menu_action,
            commands::tray_menu::tray_menu_resize,
            commands::workday::clock_in,
            commands::workday::clock_out,
            commands::workday::overtime,
            commands::workday::day_detail,
            commands::workday::week_detail,
            commands::workday::workday_total,
            commands::window::main_hide_to_tray,
            commands::window::set_board_backdrop
        ])
        .on_window_event(|window, event| {
            // 关闭语义 = 隐藏到托盘（PL004 定案）；退出走托盘菜单
            if let WindowEvent::CloseRequested { api, .. } = event {
                // PATCH007.4（CT FIX005.1 / FIX006.2）：非主窗（悬浮窗）一律只拦不处理——
                // 否则其关窗会误落主窗位置记忆，且 runtime 默认销毁会让悬浮窗本会话失效
                if window.label() != "main" {
                    api.prevent_close();
                    return;
                }
                api.prevent_close();
                if let Err(err) = hide_main_window(window.app_handle()) {
                    warn_diag(&err);
                }
            }
            if let WindowEvent::Moved(_) = event {
                // PL028.3：吸附状态机只服务主窗——其它窗（无边框浮层等）的 Moved 不入
                // （时间戳污染 / 误吸附；CT P2-3 坑）
                if window.label() != "main" {
                    return;
                }
                // 贴边吸附状态机（PL028.3）：Moved 更新时间戳；左键按住（真拖动，含悬停 /
                // 慢速——静默判定会误吸）或间隔不超 DRAG_QUIET_MS 置 DRAG_ACTIVE；
                // pending 单线程守到"左键已松 + 静默满 DRAG_QUIET_MS"→ 吸附一次
                let now = now_ms();
                let last = LAST_MOVED_MS.swap(now, Ordering::Relaxed);
                let dragging = now.saturating_sub(last) <= DRAG_QUIET_MS || win_input::left_down();
                if dragging {
                    DRAG_ACTIVE.store(true, Ordering::Relaxed);
                }
                if !DRAG_SNAP_PENDING.swap(true, Ordering::Relaxed) {
                    let win = window.clone();
                    std::thread::spawn(move || loop {
                        std::thread::sleep(Duration::from_millis(100));
                        if win_input::left_down() {
                            continue; // 左键仍按住 = 还在拖（悬停 / 慢速不误吸）
                        }
                        if now_ms().saturating_sub(LAST_MOVED_MS.load(Ordering::Relaxed))
                            < DRAG_QUIET_MS
                        {
                            continue; // 刚松手还在惯性 / 最后移动
                        }
                        DRAG_SNAP_PENDING.store(false, Ordering::Relaxed);
                        if DRAG_ACTIVE.swap(false, Ordering::Relaxed) {
                            // PATCH011：真拖动过 ⇒ 通知前端"拖拽结束"（前端据此解除焦点抖动屏蔽；
                            // 原生拖拽循环会给出成对 Focused(false/true) 抖动，见 z.plan 附录 PATCH011）。
                            // 发送失败仅日志——装饰层纪律，下次拖拽自愈。
                            if let Err(err) = win.emit("drag-end", ()) {
                                diag::log(&format!("drag-end 事件发送失败：{err}"));
                            }
                            snap_if_needed(&win);
                        }
                        break;
                    });
                }
            }
            // 焦点联动分态纱（PL021.4 CT 定案）：失焦 30% 纱、聚焦 0% 纱（桌面直透）——
            // DWM 系统背板材质已随 CT 化退役（恒纯 alpha 透明窗），此处仅转发焦点态给前端
            if let WindowEvent::Focused(focused) = event {
                // PATCH007.4（CT FIX005.2）：焦点态只服务主窗——悬浮窗（focusable(false) 理论
                // 不获焦，此处防御式过滤）获焦不得翻转主窗纱态
                if window.label() != "main" {
                    return;
                }
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
                // 置顶按 config 应用（PL028.3）：失败仅日志——设置已落库，下次启动重试即自愈
                let on_top = app
                    .state::<AppContext>()
                    .settings
                    .lock()
                    .map(|s| s.always_on_top)
                    .unwrap_or(false);
                if let Err(err) = window.set_always_on_top(on_top) {
                    warn_diag(&format!("启动置顶应用失败：{err}"));
                }
                glass_backdrop::hide_active_border(&window);
                glass_backdrop::apply_round_corners(&window);
            }
            // 托盘 hover 概览窗（PL028.1）：屏外创建 + visible(true) 让 webview 首绘即完成，
            // build 后立即 hide（CT 预览窗同款——隐藏创建会首 hover 空窗；屏外 show 会被
            // Windows clamp 回可见区留残影）。focusable(false)：点击不抢系统焦点。
            let hover = tauri::WebviewWindowBuilder::new(
                app,
                "tray-hover",
                tauri::WebviewUrl::App("index.html".into()),
            )
            .title("CapsulePulse 概览")
            .position(-2000.0, -2000.0)
            .inner_size(hover::WINDOW_WIDTH, hover::HEIGHT_MIN)
            .transparent(true)
            .decorations(false)
            .skip_taskbar(true)
            .resizable(false)
            .visible(true)
            .focused(false)
            .focusable(false)
            .always_on_top(true)
            .shadow(false)
            .build()?;
            if let Err(err) = hover.hide() {
                warn_diag(&format!("hover 窗首绘后隐藏失败：{err}"));
            }
            // 补系统圆角（PATCH007 漏抄补上；CT 预览窗同款）：`shadow(false)` 关掉 Tauri 投影后
            // 窗口**隐式圆角随之丢失** ⇒ 不补则卡片四角外露方角（CP AGENTS 已记该根因；
            // CT `glass_backdrop::apply_round_corners(&preview)` ✓）—— 让 DWM 切掉 CSS 圆角外的
            // 窗口直角区，窗角与卡片 `border-radius` 视觉统一。
            glass_backdrop::apply_round_corners(&hover);
            // 隐藏 Win11 窗口描边（PATCH007.15 漏抄补上 —— 主窗窗口整定是**两步**：描边隐藏 +
            // 系统圆角；我给悬浮窗只抄了后一步 ✗）：DWM 给窗口画的那圈系统描边落在**卡片之外**
            // （webview 之外 ⇒ CSS 改不动 ✗）⇒ 表现为"卡片外还有一圈" ✓。与主窗完全对齐 ✓。
            glass_backdrop::hide_active_border(&hover);
            // 托盘菜单窗（PATCH008）：自绘 HTML 菜单替代原生 Win32 菜单（照 CT PL021 逻辑）——
            // 生命周期与 hover 窗同款（屏外创建 + visible(true) 让 webview 首绘即完成 + build 后
            // 立即 hide；不显任务栏 / 不可缩放 / 不获焦 / 置顶 / 无投影），窗口整定两步一致
            // （系统圆角 + 激活描边隐藏）。
            let menu_win = tauri::WebviewWindowBuilder::new(
                app,
                "tray-menu",
                tauri::WebviewUrl::App("index.html".into()),
            )
            .title("CapsulePulse 菜单")
            .position(-2000.0, -2000.0)
            .inner_size(menu::WINDOW_WIDTH, menu::HEIGHT_MIN)
            .transparent(true)
            .decorations(false)
            .skip_taskbar(true)
            .resizable(false)
            .visible(true)
            .focused(false)
            .focusable(false)
            .always_on_top(true)
            .shadow(false)
            .build()?;
            if let Err(err) = menu_win.hide() {
                warn_diag(&format!("菜单窗首绘后隐藏失败：{err}"));
            }
            glass_backdrop::apply_round_corners(&menu_win);
            glass_backdrop::hide_active_border(&menu_win);
            // 悬浮窗守候线程（PATCH007.2 + .6）：100ms 步进结算显隐（**唯一调用者**）
            // + 显示中按 1 秒节拍重推概览
            spawn_hover_watcher(&app.handle().clone());
            build_tray(app)?;
            register_global_shortcuts(app)?;
            // 惰性按天汇总（PL027.7）：启动时把"已封口但未滚"的过去日滚进 days（幂等；
            // 失败仅落档——汇总只是加速层，不影响主流程）
            let ctx = app.state::<AppContext>();
            let now = match commands::now_secs(&ctx) {
                Ok(now) => now,
                Err(err) => {
                    warn_diag(&format!("按天汇总跳过（时钟异常）：{err}"));
                    0
                }
            };
            // 脏数据归一化（PL027.9）：嵌套打卡 → "前班在新打卡刻结束"（幂等；失败不阻断启动）
            if now > 0 {
                match commands::workday::normalize_dirty_data_inner(&ctx) {
                    Ok(0) => {}
                    Ok(n) => warn_diag(&format!("启动归一化：追加 {n} 条纠正事件")),
                    Err(err) => warn_diag(&format!("启动归一化失败（忽略）：{err}")),
                }
            }
            // 旧模型一次性迁移（PL027.1b）：备份 → 校验 → 删旧两表 → 回填汇总 → 置版本。
            // 失败不阻断启动（新代码只读流水，旧表留着无害），仅落档、下次启动重试。
            if now > 0 {
                commands::workday::migrate_on_start(&ctx, now);
            }
            if now > 0 {
                if let Err(err) = commands::workday::seal_days_inner(&ctx, now) {
                    warn_diag(&format!("按天汇总失败（忽略）：{err}"));
                }
            }
            // 心跳线程（PL027.3）：在岗期间每 60 秒刷新运行态缓存（未上班空转）
            commands::workday::spawn_beat_thread(&app.handle().clone());
            // 启动恢复三分支（PL027.4）：无缝续（≤10 分钟）/ 按最后心跳补下班（>10 分钟）/ 无事。
            // 幂等；失败不阻断启动（按常规启动路径继续）
            if now > 0 {
                match commands::workday::recover_on_startup(&ctx, now) {
                    Ok(outcome) => {
                        if let Some(at) = outcome.closed_at {
                            warn_diag(&format!("启动恢复：已按 {at} 补下班记账"));
                            // 一次性提示（PL027.8）：仅"判下班"分支留痕供前端弹条
                            if let Ok(mut slot) = ctx.recovery.lock() {
                                *slot = Some(outcome);
                            }
                        }
                    }
                    Err(err) => warn_diag(&format!("启动恢复失败（按常规启动继续）：{err}")),
                }
            }
            // 兜底自动下班武装（PL025.8）：在岗恢复时按"上班 + N"重建基准并起线程——
            // 重启后 tick 之外仍有第二道保险（与打卡路径同源：arm → spawn）
            let ctx = app.state::<AppContext>();
            match commands::workday::arm_auto_out_on_start(&ctx) {
                Ok(Some(generation)) => {
                    commands::workday::spawn_auto_out_watcher(&app.handle().clone(), generation);
                }
                Ok(None) => {}
                Err(err) => warn_diag(&format!("自动下班基准重建失败（兜底线程未武装）：{err}")),
            }
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
