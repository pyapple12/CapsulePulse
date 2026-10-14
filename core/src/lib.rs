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
use crate::reminder::ReminderFire;
use crate::session::WorkSession;
use crate::settings::{ReminderSettings, WindowPos};
use crate::storage::Storage;
use crate::workday::WorkdayState;
use tauri::{
    menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem},
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
    /// 鼠标左键是否按下（GetAsyncKeyState 高位 = 物理 / 合成按下中）
    pub fn left_down() -> bool {
        const VK_LBUTTON: i32 = 0x01;
        const PRESSED: u16 = 0x8000;
        unsafe { (GetAsyncKeyState(VK_LBUTTON) as u16) & PRESSED != 0 }
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

// —— 托盘 hover 今日概览窗（PL028.1）——
// 几何与节奏（抄 CT PL018 预览窗形态，收敛为一进一出两拍）：
// Enter → 记录图标矩形 + 停留阈值后仍悬停才显示（防"划过托盘"误触）；Leave → 延迟收起
// （给"从图标移向悬浮窗"留缓冲，防闪烁）。光标判据仅 Windows 有（非 Windows 退化为纯事件驱动）。
/// 悬浮窗宽度（逻辑 px；高度按内容行数在显示前由后端设定——两端同源）
const HOVER_WIDTH: f64 = 200.0;
/// 一行态（未在岗）高度
const HOVER_ONE_LINE_HEIGHT: f64 = 46.0;
/// 三行态（在岗 / 暂下班）高度
const HOVER_THREE_LINE_HEIGHT: f64 = 112.0;
/// 悬浮窗与托盘图标的间距（物理 px）
const HOVER_ANCHOR_MARGIN: i32 = 8;
/// 悬停停留阈值（ms）：防划过误触
const HOVER_DWELL_MS: u64 = 400;
/// 离开后延迟收起（ms）：防图标↔悬浮窗之间的闪烁
const HOVER_HIDE_DELAY_MS: u64 = 250;

/// 托盘图标矩形（物理坐标 x/y/w/h；Enter 事件记录，供锚定与光标判据）——未 hover 过为 None
static HOVER_ANCHOR: Mutex<Option<(i32, i32, i32, i32)>> = Mutex::new(None);

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

/// 光标是否仍在托盘图标矩形内：`Some(true/false)` 为判定结果，`None` = 本平台无查询能力
/// （Enter 路径视为"仍在"继续、Leave 路径视为"已离开"收起——纯事件驱动语义）。
fn cursor_in_tray() -> Option<bool> {
    let anchor = match HOVER_ANCHOR.lock() {
        Ok(slot) => *slot,
        Err(_) => None,
    };
    let Some((x, y, w, h)) = anchor else {
        return Some(false);
    };
    win_cursor::cursor_pos().map(|(cx, cy)| cx >= x && cx < x + w && cy >= y && cy < y + h)
}

/// 悬浮窗锚定（PL028.1，抄 CT 预览窗几何）：右缘对齐托盘图标右缘；默认图标上方，屏上缘
/// 放不下则翻到下方（任务栏在顶部场景）。锚点未记录则跳过（启动后未 hover 过，窗也从未显示）。
fn anchor_tray_hover(window: &tauri::WebviewWindow) {
    let anchor = match HOVER_ANCHOR.lock() {
        Ok(slot) => *slot,
        Err(_) => None,
    };
    let Some((x, y, w, h)) = anchor else {
        return;
    };
    let Ok(size) = window.outer_size() else {
        return;
    };
    let px = x + w - size.width as i32; // 右对齐托盘图标
    let mut py = y - size.height as i32 - HOVER_ANCHOR_MARGIN; // 默认图标上方
    if py < 0 {
        py = y + h + HOVER_ANCHOR_MARGIN; // 翻到下方
    }
    if let Err(err) = window.set_position(PhysicalPosition::new(px, py)) {
        warn_diag(&format!("hover 窗定位失败：{err}"));
    }
}

/// 显示悬浮窗（PL028.1）：聚合概览 → 按行数定尺寸 → 推送 `hover-update`（前端据此渲染）
/// → 锚定 → show。任一步失败落日志（装饰层不阻断主流程——容错白名单登记项）。
fn show_tray_hover(app: &AppHandle) {
    let Some(window) = app.get_webview_window("tray-hover") else {
        return;
    };
    let ctx = app.state::<AppContext>();
    let summary = match commands::hover::hover_summary_inner(&ctx) {
        Ok(summary) => summary,
        Err(err) => {
            warn_diag(&format!("hover 概览聚合失败（不显示）：{err}"));
            return;
        }
    };
    let height = if summary.single_line {
        HOVER_ONE_LINE_HEIGHT
    } else {
        HOVER_THREE_LINE_HEIGHT
    };
    if let Err(err) = window.set_size(tauri::LogicalSize::new(HOVER_WIDTH, height)) {
        warn_diag(&format!("hover 窗尺寸设置失败：{err}"));
    }
    if let Err(err) = window.emit_to("tray-hover", "hover-update", &summary) {
        warn_diag(&format!("hover 概览推送失败：{err}"));
    }
    anchor_tray_hover(&window);
    if let Err(err) = window.show() {
        warn_diag(&format!("hover 窗显示失败：{err}"));
    }
}

/// 收起悬浮窗（PL028.1；失败仅日志）。
fn hide_tray_hover(app: &AppHandle) {
    let Some(window) = app.get_webview_window("tray-hover") else {
        return;
    };
    if let Err(err) = window.hide() {
        warn_diag(&format!("hover 窗收起失败：{err}"));
    }
}

/// 托盘图标悬停进入（PL028.1）：记录图标矩形 → 停留阈值后仍悬停才显示（防划过误触）。
fn on_tray_hover_enter(app: &AppHandle, rect: tauri::Rect) {
    let (tauri::Position::Physical(pos), tauri::Size::Physical(size)) = (rect.position, rect.size)
    else {
        return;
    };
    if let Ok(mut slot) = HOVER_ANCHOR.lock() {
        *slot = Some((pos.x, pos.y, size.width as i32, size.height as i32));
    }
    let handle = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(HOVER_DWELL_MS));
        if cursor_in_tray() == Some(false) {
            return; // 已划过 / 已移开：不显示
        }
        show_tray_hover(&handle);
    });
}

/// 托盘图标悬停离开（PL028.1）：延迟收起（缓冲"从图标移向悬浮窗"，防闪烁）。
fn on_tray_hover_leave(app: &AppHandle) {
    let handle = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(HOVER_HIDE_DELAY_MS));
        if cursor_in_tray() == Some(true) {
            return; // 又回到图标上：保持显示
        }
        hide_tray_hover(&handle);
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

/// 窗口偏好全量同步（PL028.3 单一同步出口）：广播 `prefs-changed`（设置面板回显）+
/// 刷新托盘勾选态——设置面板与托盘菜单双入口的改动都经此收口。
pub(crate) fn sync_prefs_ui(app: &AppHandle) {
    let ctx = app.state::<AppContext>();
    commands::reminder::emit_prefs_changed(app, &ctx);
    refresh_tray_menu(app);
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

/// 构建托盘菜单（PL028.3）：显示/隐藏 + 开始/暂停 + 分隔线 + **窗口置顶 / 贴边吸附**
/// （`CheckMenuItem`，勾选态 = 设置事实源）+ 退出。重建即勾选态刷新（[`refresh_tray_menu`]）。
fn build_tray_menu(
    manager: &impl Manager<tauri::Wry>,
    always_on_top: bool,
    snap_to_edge: bool,
) -> Result<Menu<tauri::Wry>, Box<dyn std::error::Error>> {
    let toggle_visible =
        MenuItem::with_id(manager, "toggle_visible", "显示/隐藏", true, None::<&str>)?;
    let toggle_timer = MenuItem::with_id(manager, "toggle_timer", "开始/暂停", true, None::<&str>)?;
    let on_top = CheckMenuItem::with_id(
        manager,
        "toggle_always_on_top",
        "窗口置顶",
        true,
        always_on_top,
        None::<&str>,
    )?;
    let snap = CheckMenuItem::with_id(
        manager,
        "toggle_snap_to_edge",
        "贴边吸附",
        true,
        snap_to_edge,
        None::<&str>,
    )?;
    let quit = MenuItem::with_id(manager, "quit", "退出", true, None::<&str>)?;
    let sep_a = PredefinedMenuItem::separator(manager)?;
    let sep_b = PredefinedMenuItem::separator(manager)?;
    Ok(Menu::with_items(
        manager,
        &[
            &toggle_visible,
            &toggle_timer,
            &sep_a,
            &on_top,
            &snap,
            &sep_b,
            &quit,
        ],
    )?)
}

/// 托盘菜单勾选态刷新（PL028.3）：`TrayIcon` 只有 `set_menu`、无菜单读取口 ⇒ 按设置
/// 事实源**重建菜单并回灌**。失败落日志（托盘为体验层，下次刷新自愈）。
fn refresh_tray_menu(app: &AppHandle) {
    let ctx = app.state::<AppContext>();
    let (on_top, snap) = match ctx.settings.lock() {
        Ok(settings) => (settings.always_on_top, settings.snap_to_edge),
        Err(err) => {
            warn_diag(&format!("设置锁中毒，托盘勾选态未刷新：{err:?}"));
            return;
        }
    };
    let menu = match build_tray_menu(app, on_top, snap) {
        Ok(menu) => menu,
        Err(err) => {
            warn_diag(&format!("托盘菜单重建失败（勾选态未刷新）：{err}"));
            return;
        }
    };
    let Some(tray) = app.tray_by_id("main-tray") else {
        return;
    };
    if let Err(err) = tray.set_menu(Some(menu)) {
        warn_diag(&format!("托盘菜单刷新失败：{err}"));
    }
}

/// 托盘菜单动作分发（按菜单项 id；计时切换与退出落库复用命令层自由函数）。
fn handle_action(app: &AppHandle, id: &str) {
    match id {
        "toggle_visible" => toggle_main_window(app),
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
            app.exit(0);
        }
        _ => {}
    }
}

/// 构建托盘：应用图标 + 菜单（显示/隐藏、开始/暂停、窗口置顶、贴边吸附、退出）；
/// 左键单击 = 显隐切换。菜单项勾选态按 config 初始化（PL028.3）。
fn build_tray(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let (on_top, snap) = {
        let ctx = app.state::<AppContext>();
        let settings = ctx
            .settings
            .lock()
            .map_err(|err| format!("设置锁中毒，托盘无法构建：{err:?}"))?;
        (settings.always_on_top, settings.snap_to_edge)
    };
    let menu = build_tray_menu(app, on_top, snap)?;
    let icon = app
        .default_window_icon()
        .ok_or("缺少应用图标（tauri.conf.json bundle.icon），托盘无法构建")?;
    TrayIconBuilder::with_id("main-tray")
        .icon(icon.clone())
        .tooltip("CapsulePulse")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| handle_action(app, event.id.as_ref()))
        .on_tray_icon_event(|tray, event| match event {
            // 左键单击 = 显隐切换；右键弹菜单（show_menu_on_left_click 已关）
            TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } => toggle_main_window(tray.app_handle()),
            // 右键（原生菜单将弹出）：先收掉 hover 概览窗——它恒置顶，会浮在菜单之上
            TrayIconEvent::Click {
                button: MouseButton::Right,
                ..
            } => hide_tray_hover(tray.app_handle()),
            // PL028.1：悬停托盘图标 → 今日概览悬浮窗（停留阈值 + 光标判据防误触）
            TrayIconEvent::Enter { rect, .. } => on_tray_hover_enter(tray.app_handle(), rect),
            TrayIconEvent::Leave { .. } => on_tray_hover_leave(tray.app_handle()),
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
            commands::workday::clock_in,
            commands::workday::clock_out,
            commands::workday::overtime,
            commands::workday::day_detail,
            commands::workday::week_detail,
            commands::workday::workday_total,
            commands::window::main_hide_to_tray
        ])
        .on_window_event(|window, event| {
            // 关闭语义 = 隐藏到托盘（PL004 定案）；退出走托盘菜单
            if let WindowEvent::CloseRequested { api, .. } = event {
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
                            snap_if_needed(&win);
                        }
                        break;
                    });
                }
            }
            // 焦点联动分态纱（PL021.4 CT 定案）：失焦 30% 纱、聚焦 0% 纱（桌面直透）——
            // DWM 系统背板材质已随 CT 化退役（恒纯 alpha 透明窗），此处仅转发焦点态给前端
            if let WindowEvent::Focused(focused) = event {
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
            .inner_size(HOVER_WIDTH, HOVER_ONE_LINE_HEIGHT)
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
