//! 统计周期边界：今日起点的纯函数计算（PL002 阶段 A）。
//! 入参泛型 over chrono 时区（生产传 Local，测试传 FixedOffset）——核心逻辑机器时区无关可测；
//! 纯整数推导（零点秒数），无 Option 分支、无 panic 路径。
//!
//! **本周口径不在本模块**（FIX010.2 订正）：自 PL022 起"周一起自然周"走**按日期偏移逐日取本地零点**
//! （`commands/workday.rs::monday_offset` + 逐日 `day_bounds`），不再由单个"周起点秒"推导。原先的
//! `week_start_secs` 按 `× 86_400` 整数近似推周一零点（DST 时区会偏 ±1 小时），且全仓零消费者，
//! 属废弃 API ⇒ 已删除（AGENTS「不长期保留废弃方案」）。

use chrono::{DateTime, TimeZone, Timelike};

/// 今日起点（本地当日零点）的 Unix 秒。
pub fn day_start_secs<Tz: TimeZone>(now: &DateTime<Tz>) -> i64 {
    now.timestamp() - i64::from(now.num_seconds_from_midnight())
}

#[cfg(test)]
mod tests {
    use chrono::{FixedOffset, TimeZone};

    use super::*;

    /// 固定 +8 时区（近似 CST），测试确定性锚点。
    fn tz() -> FixedOffset {
        FixedOffset::east_opt(8 * 3600).unwrap()
    }

    /// 已知锚点：2026-09-07（周一）、09-09（周三）、09-13（周日）、09-14（周一）——日历事实取自 chrono 自身。
    fn at(y: i32, m: u32, d: u32, h: u32, mi: u32, s: u32) -> chrono::DateTime<FixedOffset> {
        tz().with_ymd_and_hms(y, m, d, h, mi, s).unwrap()
    }

    /// 今日起点 = 本地当日零点（跨零点两侧各自归位）。
    #[test]
    fn day_start_is_local_midnight() {
        let before = at(2026, 9, 8, 23, 59, 59);
        let after = at(2026, 9, 9, 0, 0, 1);
        assert_eq!(day_start_secs(&before), at(2026, 9, 8, 0, 0, 0).timestamp());
        assert_eq!(day_start_secs(&after), at(2026, 9, 9, 0, 0, 0).timestamp());
    }

    /// 今日起点恰在边界：00:00:00 即当日零点自身。
    #[test]
    fn day_start_at_exact_midnight() {
        let exact = at(2026, 9, 9, 0, 0, 0);
        assert_eq!(day_start_secs(&exact), exact.timestamp());
    }
}
