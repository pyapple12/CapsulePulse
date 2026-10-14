//! 贴边吸附纯逻辑（PL028.3，抄 CT PL017）：窗口位置对所在显示器四边的吸附判定。
//! 纯函数零依赖（禁 import tauri，`cargo test` 直测）；被 `lib.rs` 的 `WindowEvent::Moved`
//! 防抖状态机消费（松手吸附定案）。阈值与落位间距单一来源 = [`crate::settings::SNAP_*`]。

use crate::settings::{SNAP_GAP_PX, SNAP_THRESHOLD_PX};

/// 整数矩形（物理坐标，x/y 为左上角）：[`snap_position`] 的窗口与显示器入参载体。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RectI32 {
    /// 左上角 x
    pub x: i32,
    /// 左上角 y
    pub y: i32,
    /// 宽
    pub w: i32,
    /// 高
    pub h: i32,
}

/// 计算吸附后窗口左上角位置：任一边距屏幕对应边 ≤ 阈值时吸附到距边 gap 落位——
/// 四边独立判定可组合（角落同时吸两轴）；未命中或矩形非法时原样返回。
/// **幂等**：已在吸附位返回同值（调用方据此判等跳过 `set_position`，防 Moved→snap→Moved 死循环）。
///
/// # 参数
/// `win` 窗口矩形；`mon` 显示器矩形（调用方取 `current_monitor` 的物理坐标）。
///
/// # 返回
/// 吸附修正后的窗口左上角 `(x, y)`——可能与输入相同（未命中 / 已吸附）。
pub fn snap_position(win: RectI32, mon: RectI32) -> (i32, i32) {
    // 矩形非法（零 / 负尺寸）防御式跳过——实测值异常不该崩
    if win.w <= 0 || win.h <= 0 || mon.w <= 0 || mon.h <= 0 {
        return (win.x, win.y);
    }
    let mut x = win.x;
    let mut y = win.y;
    // 判定窗口 = ±阈值：缘距落在 [-50, +50] 都算命中——正值为距边尚有间隙、负值为拖动过冲
    // （缘已超出屏幕）；过冲拉回贴边符合手感（CT 实测：拖到角落常过冲，只判正向会漏轴）。
    let left_gap = win.x - mon.x;
    if (-(SNAP_THRESHOLD_PX)..=SNAP_THRESHOLD_PX).contains(&left_gap) {
        x = mon.x + SNAP_GAP_PX;
    }
    let right_gap = mon.x + mon.w - (win.x + win.w);
    if (-(SNAP_THRESHOLD_PX)..=SNAP_THRESHOLD_PX).contains(&right_gap) {
        x = mon.x + mon.w - win.w - SNAP_GAP_PX;
    }
    let top_gap = win.y - mon.y;
    if (-(SNAP_THRESHOLD_PX)..=SNAP_THRESHOLD_PX).contains(&top_gap) {
        y = mon.y + SNAP_GAP_PX;
    }
    let bottom_gap = mon.y + mon.h - (win.y + win.h);
    if (-(SNAP_THRESHOLD_PX)..=SNAP_THRESHOLD_PX).contains(&bottom_gap) {
        y = mon.y + mon.h - win.h - SNAP_GAP_PX;
    }
    (x, y)
}

#[cfg(test)]
mod tests {
    use super::{snap_position, RectI32};

    /// 标准场景：窗口 300×400、显示器 1920×1080 从 (0,0) 起（多屏负坐标由相对计算天然覆盖）。
    const WIN: RectI32 = RectI32 {
        x: 0,
        y: 0,
        w: 300,
        h: 400,
    };
    const MON: RectI32 = RectI32 {
        x: 0,
        y: 0,
        w: 1920,
        h: 1080,
    };

    fn snap_at(x: i32, y: i32) -> (i32, i32) {
        snap_position(RectI32 { x, y, ..WIN }, MON)
    }

    #[test]
    fn snaps_to_left_edge() {
        assert_eq!(snap_at(30, 300), (5, 300));
    }

    #[test]
    fn snaps_to_right_edge() {
        assert_eq!(snap_at(1920 - 300 - 20, 300), (1920 - 300 - 5, 300));
    }

    #[test]
    fn snaps_to_top_edge() {
        assert_eq!(snap_at(500, 10), (500, 5));
    }

    #[test]
    fn snaps_to_bottom_edge() {
        assert_eq!(snap_at(500, 1080 - 400 - 40), (500, 1080 - 400 - 5));
    }

    #[test]
    fn snaps_corner_on_both_axes() {
        assert_eq!(snap_at(45, 45), (5, 5));
    }

    #[test]
    fn no_snap_beyond_threshold() {
        assert_eq!(snap_at(60, 60), (60, 60));
    }

    #[test]
    fn snaps_overshot_right_edge() {
        assert_eq!(snap_at(1920 - 300 + 20, 300), (1920 - 300 - 5, 300));
    }

    #[test]
    fn snaps_overshot_corner_both_axes() {
        assert_eq!(
            snap_at(1920 - 300 + 30, 1080 - 400 + 10),
            (1920 - 300 - 5, 1080 - 400 - 5)
        );
    }

    #[test]
    fn idempotent_at_snapped_position() {
        assert_eq!(snap_at(5, 5), (5, 5));
    }

    #[test]
    fn invalid_rect_passthrough() {
        let (x, y) = snap_position(
            RectI32 {
                x: 10,
                y: 10,
                w: 0,
                h: 400,
            },
            MON,
        );
        assert_eq!((x, y), (10, 10));
    }
}
