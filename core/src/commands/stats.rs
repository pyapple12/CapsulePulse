//! 统计命令：今日/本周/累计三值聚合（前端低频拉取）。
//! PL027.6 起**统一走单日管道口径**（与三值/图谱同源）——不再读 sessions 表求和；
//! PL027.7 起由 `days` 汇总表加速已封口数据日的聚合。

use serde::Serialize;
use tauri::State;

use super::workday::{day_summary_inner, week_detail_inner, workday_total_inner};
use super::{now_secs, AppContext, CommandError};
use crate::session::Clock;

/// stats 命令返回体：三值统计（秒；serde 结构单一来源，TS 侧镜像）。
#[derive(Debug, Serialize)]
pub struct SessionStats {
    /// 今日累计秒数。
    pub today_secs: i64,
    /// 本周累计秒数（周一起）。
    pub week_secs: i64,
    /// 全部累计秒数。
    pub all_secs: i64,
}

/// 统计快照：今日 = 当日管道工作秒；本周 = 本周 7 日管道求和；累计 = 全历史逐日求和
/// （PL027.7 起由 `days` 汇总表加速）。三者与三值/图谱**同源**（单一事实源）。
fn stats_snapshot<C: Clock>(ctx: &AppContext<C>) -> Result<SessionStats, CommandError> {
    let now = now_secs(ctx)?;
    let today_secs = day_summary_inner(ctx, 0, now)?.work_secs;
    let week_secs: i64 = week_detail_inner(ctx, 0, now)?
        .iter()
        .map(|day| day.work_secs)
        .sum();
    let all_secs = workday_total_inner(ctx, now)?.work_secs;
    Ok(SessionStats {
        today_secs,
        week_secs,
        all_secs,
    })
}

/// 查询今日/本周/累计统计（async：聚合查询移出主线程，FIX002.8）。
#[tauri::command]
pub async fn session_stats(handle: State<'_, AppContext>) -> Result<SessionStats, CommandError> {
    stats_snapshot(&handle)
}

#[cfg(test)]
mod tests {
    use chrono::{Local, TimeZone};

    use super::*;
    use crate::commands::test_support::{ctx, FakeClock};
    use crate::workday::EventKind;

    /// PL027.6：stats 三值统一走单日管道——构造"打卡 + 工作段"流水，今日/累计同源。
    #[test]
    fn stats_snapshot_uses_single_day_pipeline() {
        let ctx = ctx(FakeClock::new());
        // 注入墙钟基准 = 假钟 base（1_000）：数据日随注入时钟定界（不用真实"今天"，
        // 否则管道认的"今日"与夹具落点不同日）
        let injected = crate::commands::now_secs(&ctx).unwrap();
        let now_dt = Local.timestamp_opt(injected, 0).single().unwrap();
        let today0 = crate::period::day_start_secs(&now_dt);
        let yesterday0 = today0 - 86_400;
        let shift = |day0: i64| {
            vec![
                (day0 + 9 * 3600, EventKind::ClockIn),
                (day0 + 9 * 3600, EventKind::SegmentStart),
                (day0 + 10 * 3600, EventKind::SegmentEnd),
                (day0 + 10 * 3600, EventKind::ClockOut),
            ]
        };
        {
            let storage = ctx.storage.lock().unwrap();
            storage.insert_events(&shift(today0)).unwrap();
            storage.insert_events(&shift(yesterday0)).unwrap();
        }

        let s = stats_snapshot(&ctx).unwrap();
        assert_eq!(s.today_secs, 3600, "今日 = 当日管道工作秒");
        assert_eq!(s.all_secs, 7200, "累计 = 全历史逐日求和");
        let week0 = crate::period::week_start_secs(&now_dt);
        let yesterday_in_week = yesterday0 >= week0;
        assert_eq!(
            s.week_secs,
            if yesterday_in_week { 7200 } else { 3600 },
            "本周 = 本周 7 日管道求和（昨日是否在本周内随星期而定）"
        );
    }
}
