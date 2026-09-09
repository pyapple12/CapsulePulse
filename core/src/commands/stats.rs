//! 统计命令：今日/本周/累计三值聚合（前端低频拉取）。

use chrono::Local;
use serde::Serialize;
use tauri::State;

use super::{poison, AppContext, CommandError};
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

/// 统计快照：三值聚合（统计按当前真实本地时间定界）。
fn stats_snapshot<C: Clock>(ctx: &AppContext<C>) -> Result<SessionStats, CommandError> {
    let storage = poison(ctx.storage.lock())?;
    let now = Local::now();
    Ok(SessionStats {
        today_secs: storage.today_total(&now)?,
        week_secs: storage.week_total(&now)?,
        all_secs: storage.all_total()?,
    })
}

/// 查询今日/本周/累计统计。
#[tauri::command]
pub fn session_stats(handle: State<'_, AppContext>) -> Result<SessionStats, CommandError> {
    stats_snapshot(&handle)
}

#[cfg(test)]
mod tests {
    use chrono::Local;

    use super::*;
    use crate::commands::test_support::{ctx, FakeClock};

    /// T3：stats 三值聚合——按当前真实本地时间定界注入边界内/外记录。
    #[test]
    fn stats_forwarding_aggregates_periods() {
        let ctx = ctx(FakeClock::new());
        let now = Local::now();
        let today = crate::period::day_start_secs(&now);
        let week = crate::period::week_start_secs(&now);
        ctx.storage
            .lock()
            .unwrap()
            .add_session(today + 3600, 100)
            .unwrap();
        ctx.storage
            .lock()
            .unwrap()
            .add_session(week + 7200, 200)
            .unwrap();
        ctx.storage.lock().unwrap().add_session(0, 400).unwrap();

        let s = stats_snapshot(&ctx).unwrap();
        assert_eq!(s.today_secs, 100);
        assert_eq!(s.week_secs, 300);
        assert_eq!(s.all_secs, 700);
    }
}
