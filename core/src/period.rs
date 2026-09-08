//! 统计周期边界：今日/本周起点的纯函数计算（PL002 阶段 A）。
//! 入参泛型 over chrono 时区（生产传 Local，测试传 FixedOffset）——核心逻辑机器时区无关可测；
//! 本周定义 = 周一起自然周（用户定案 2026-09-09）。
//! 纯整数推导（零点秒数 + 距周一已知天数），无 Option 分支、无 panic 路径。

use chrono::{DateTime, Datelike, TimeZone, Timelike};

/// 今日起点（本地当日零点）的 Unix 秒。
pub fn day_start_secs<Tz: TimeZone>(now: &DateTime<Tz>) -> i64 {
    now.timestamp() - i64::from(now.num_seconds_from_midnight())
}

/// 本周起点（周一 00:00:00）的 Unix 秒——周日深夜仍归位本周一。
pub fn week_start_secs<Tz: TimeZone>(now: &DateTime<Tz>) -> i64 {
    day_start_secs(now) - i64::from(now.weekday().num_days_from_monday()) * 86_400
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

    /// 本周起点 = 周一零点：周内任意时刻（含周日深夜）都归位到同一周一。
    #[test]
    fn week_start_is_monday_midnight() {
        let monday = at(2026, 9, 7, 8, 0, 0);
        let wednesday = at(2026, 9, 9, 12, 0, 0);
        let sunday_late = at(2026, 9, 13, 23, 59, 59);
        let expected = at(2026, 9, 7, 0, 0, 0).timestamp();
        assert_eq!(week_start_secs(&monday), expected);
        assert_eq!(week_start_secs(&wednesday), expected);
        assert_eq!(week_start_secs(&sunday_late), expected);
    }

    /// 周边界翻转：周日 23:59:59 仍属旧周；周一 00:00:00 恰好归位新周零点。
    #[test]
    fn week_boundary_flips_at_monday_midnight() {
        let sun_last_tick = at(2026, 9, 13, 23, 59, 59);
        let mon_first_tick = at(2026, 9, 14, 0, 0, 0);
        assert_eq!(
            week_start_secs(&sun_last_tick),
            at(2026, 9, 7, 0, 0, 0).timestamp()
        );
        assert_eq!(
            week_start_secs(&mon_first_tick),
            at(2026, 9, 14, 0, 0, 0).timestamp()
        );
    }

    /// 今日起点恰在边界：00:00:00 即当日零点自身。
    #[test]
    fn day_start_at_exact_midnight() {
        let exact = at(2026, 9, 9, 0, 0, 0);
        assert_eq!(day_start_secs(&exact), exact.timestamp());
    }
}
