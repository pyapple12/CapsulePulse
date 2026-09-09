//! 打卡命令层（PL005）：上班/下班/单日明细与自动下班检查——转发工作日状态机 + 落库 + 事件留痕。
//! 事务次序定案：库写入先行、状态转移殿后（中途失败不污染内存态，可安全重试）；
//! 自动下班以"上班 + N"回填时刻关班（发现可迟到、账目准时）。锁序见 AppContext 注释。

use chrono::{Local, TimeZone};
use tauri::State;

use super::session::pause_session;
use super::{lock, poison, wall_now_secs, AppContext, CommandError};
use crate::period;
use crate::session::{Clock, SessionState};
use crate::workday::{
    auto_out_due, reduce_day, DaySummary, DutySpan, EventKind, WorkdayError, WorkdayState,
};

/// 上班打卡：开行 + clock_in 事件入馆，再转移状态；会话复位 Idle（上班 = 新一天计时从零，
/// 此前累计均已落库，无数据丢失）。
/// # 错误
/// 已在岗返回 [`WorkdayError::AlreadyOnDuty`]；锁中毒/存储失败照常上抛。
pub(super) fn clock_in_inner<C: Clock>(ctx: &AppContext<C>, at: i64) -> Result<i64, CommandError> {
    // workday 锁全程持有：check-then-act 原子化（防并发双击开出两行）
    let mut workday = poison(ctx.workday.lock())?;
    if workday.is_on_duty() {
        return Err(CommandError::Workday(WorkdayError::AlreadyOnDuty));
    }
    {
        let mut session = lock(ctx)?;
        session.reset();
    }
    let id = {
        let storage = poison(ctx.storage.lock())?;
        let id = storage.workday_open(at)?;
        storage.insert_event(at, EventKind::ClockIn)?;
        id
    };
    workday.clock_in(at, id)?;
    Ok(id)
}

/// 班内有运行段则暂停落库（pause_session 含 segment_end 留痕与零段跳过）；Paused/Idle 无事。
fn close_running_segment<C: Clock>(ctx: &AppContext<C>) -> Result<(), CommandError> {
    if matches!(lock(ctx)?.state(), SessionState::Running { .. }) {
        pause_session(ctx)?;
    }
    Ok(())
}

/// 下班打卡：关行 + 下班事件入馆（at 为记账时刻——手动 = 当前、自动 = 上班 + N 回填），
/// 再转移状态；会话复位 Idle（累计已全部落库）。
/// # 错误
/// 未上班返回 [`WorkdayError::NotOnDuty`]；锁中毒/存储失败照常上抛。
pub(super) fn clock_out_inner<C: Clock>(
    ctx: &AppContext<C>,
    at: i64,
    kind: EventKind,
) -> Result<DutySpan, CommandError> {
    let mut workday = poison(ctx.workday.lock())?;
    let WorkdayState::OnDuty { clock_in_at, id } = *workday else {
        return Err(CommandError::Workday(WorkdayError::NotOnDuty));
    };
    close_running_segment(ctx)?;
    {
        let storage = poison(ctx.storage.lock())?;
        storage.workday_close(id, at)?;
        storage.insert_event(at, kind)?;
    }
    workday.clock_out()?;
    {
        let mut session = lock(ctx)?;
        session.reset();
    }
    Ok(DutySpan { clock_in_at, id })
}

/// 本地日边界（offset 为日偏移：0 = 今日、-1 = 昨日）：返回 (零点, 次日零点)。
fn day_bounds(offset: i64, now_secs: i64) -> Result<(i64, i64), CommandError> {
    let dt = Local
        .timestamp_opt(now_secs, 0)
        .single()
        .ok_or(CommandError::Clock)?;
    let shifted = dt + chrono::Duration::days(offset);
    let start = period::day_start_secs(&shifted);
    Ok((start, start + 86_400))
}

/// 单日明细：events 拉取 + 事件归约（图谱与三值唯一装配点，前端零业务）。
/// 在岗中且上班时刻早于本日零点（跨夜班）时从上班时刻取事件，归约侧裁剪到本日。
pub(super) fn day_detail_inner<C: Clock>(
    ctx: &AppContext<C>,
    offset: i64,
    now_secs: i64,
) -> Result<DaySummary, CommandError> {
    let (day_start, day_end) = day_bounds(offset, now_secs)?;
    let fetch_start = {
        let workday = poison(ctx.workday.lock())?;
        match *workday {
            WorkdayState::OnDuty { clock_in_at, .. } => day_start.min(clock_in_at),
            WorkdayState::Off => day_start,
        }
    };
    let events = poison(ctx.storage.lock())?.events_between(fetch_start, day_end)?;
    Ok(reduce_day(&events, day_start, day_end, now_secs))
}

/// 自动下班检查（挂 session_status 评估口）：在岗且满 N 小时 → 以回填时刻执行下班，
/// 返回回填时刻供调用方发 workday-auto-out 事件；未在岗/未到点返回 None。
pub(super) fn try_auto_clock_out<C: Clock>(
    ctx: &AppContext<C>,
) -> Result<Option<i64>, CommandError> {
    let due_at = {
        let workday = poison(ctx.workday.lock())?;
        match *workday {
            WorkdayState::OnDuty { clock_in_at, .. } => {
                let hours = poison(ctx.settings.lock())?.workday_auto_out_hours;
                auto_out_due(wall_now_secs()?, clock_in_at, hours)
            }
            WorkdayState::Off => None,
        }
    };
    match due_at {
        // 若恰被并发手动下班，此处严格报错、下一 tick 状态已 Off 自然消停
        Some(out_at) => clock_out_inner(ctx, out_at, EventKind::AutoClockOut).map(|_| Some(out_at)),
        None => Ok(None),
    }
}

/// 计时门禁：未上班严格拒绝（前后端双保险的后端面；托盘/热键/UI 共用）。
pub(super) fn require_on_duty<C: Clock>(ctx: &AppContext<C>) -> Result<(), CommandError> {
    let workday = poison(ctx.workday.lock())?;
    if !workday.is_on_duty() {
        return Err(CommandError::Workday(WorkdayError::NotOnDuty));
    }
    Ok(())
}

/// 上班打卡（前端确认框确认后调用）。
#[tauri::command]
pub fn clock_in(handle: State<'_, AppContext>) -> Result<(), CommandError> {
    clock_in_inner(&handle, wall_now_secs()?)?;
    Ok(())
}

/// 下班打卡（前端确认框确认后调用；按当前时刻记账）。
#[tauri::command]
pub fn clock_out(handle: State<'_, AppContext>) -> Result<(), CommandError> {
    clock_out_inner(&handle, wall_now_secs()?, EventKind::ClockOut)?;
    Ok(())
}

/// 单日明细（offset 缺省 0 = 今日；前后日翻看传 ±N）。
#[tauri::command]
pub fn day_detail(
    handle: State<'_, AppContext>,
    offset: Option<i64>,
) -> Result<DaySummary, CommandError> {
    day_detail_inner(&handle, offset.unwrap_or(0), wall_now_secs()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::test_support::{begin_duty, ctx, secs, FakeClock};

    /// 库内全部事件类型（按时间序）。
    fn event_kinds<C: Clock>(ctx: &AppContext<C>) -> Vec<EventKind> {
        ctx.storage
            .lock()
            .unwrap()
            .events_between(0, i64::MAX)
            .unwrap()
            .into_iter()
            .map(|(_, kind)| kind)
            .collect()
    }

    /// W4：clock_in 开行 + 事件 + 状态 OnDuty，且计时门禁随之放行。
    #[test]
    fn clock_in_opens_workday_and_unlocks_timer() {
        let ctx = ctx(FakeClock::new());
        clock_in_inner(&ctx, 1_000).unwrap();
        assert_eq!(
            ctx.storage.lock().unwrap().workday_latest().unwrap(),
            Some((1, 1_000, None))
        );
        assert!(event_kinds(&ctx).contains(&EventKind::ClockIn));
        assert_eq!(
            *ctx.workday.lock().unwrap(),
            WorkdayState::OnDuty {
                clock_in_at: 1_000,
                id: 1
            }
        );
        assert!(
            super::super::session::start_session(&ctx).is_ok(),
            "上班后计时放行"
        );
    }

    /// 重复上班严格报错；未上班计时被严格拒绝且零副作用。
    #[test]
    fn double_clock_in_and_ungated_start_error() {
        let duty = ctx(FakeClock::new());
        clock_in_inner(&duty, 1_000).unwrap();
        assert!(matches!(
            clock_in_inner(&duty, 2_000),
            Err(CommandError::Workday(WorkdayError::AlreadyOnDuty))
        ));
        assert_eq!(
            duty.storage.lock().unwrap().workday_latest().unwrap(),
            Some((1, 1_000, None)),
            "拒绝的上班不开新行"
        );

        let fresh = ctx(FakeClock::new());
        assert!(matches!(
            super::super::session::start_session(&fresh),
            Err(CommandError::Workday(WorkdayError::NotOnDuty))
        ));
        assert_eq!(fresh.storage.lock().unwrap().session_count().unwrap(), 0);
    }

    /// W4：下班全链——运行段落库、关行 + clock_out 事件、状态回 Off、会话复位 Idle。
    #[test]
    fn clock_out_closes_segment_and_workday() {
        let clock = FakeClock::new();
        let ctx = ctx(clock.clone());
        begin_duty(&ctx);
        super::super::session::start_session(&ctx).unwrap();
        clock.advance(secs(60));
        clock_out_inner(&ctx, 2_000, EventKind::ClockOut).unwrap();

        {
            let storage = ctx.storage.lock().unwrap();
            assert_eq!(storage.all_total().unwrap(), 60, "运行段先落库");
            assert_eq!(
                storage.workday_latest().unwrap(),
                Some((1, 1_000, Some(2_000)))
            );
        }
        let kinds = event_kinds(&ctx);
        assert!(kinds.contains(&EventKind::SegmentStart));
        assert!(kinds.contains(&EventKind::SegmentEnd));
        assert!(kinds.contains(&EventKind::ClockOut));
        assert_eq!(*ctx.workday.lock().unwrap(), WorkdayState::Off);
        assert_eq!(
            super::super::session::status_snapshot(&ctx)
                .unwrap()
                .0
                .state,
            "idle"
        );
    }

    /// 未上班下班严格报错（含自动路径复用的同一入口）。
    #[test]
    fn clock_out_off_duty_errors() {
        let ctx = ctx(FakeClock::new());
        assert!(matches!(
            clock_out_inner(&ctx, 2_000, EventKind::ClockOut),
            Err(CommandError::Workday(WorkdayError::NotOnDuty))
        ));
    }

    /// W4：day_detail 装配——种子事件归约为三值与区块（在岗 8h/工作 3h/休息 5h）。
    #[test]
    fn day_detail_reduces_seeded_events() {
        let ctx = ctx(FakeClock::new());
        let now = Local::now().timestamp();
        let day_start = period::day_start_secs(&Local::now());
        {
            let storage = ctx.storage.lock().unwrap();
            storage
                .insert_event(day_start + 9 * 3_600, EventKind::ClockIn)
                .unwrap();
            storage
                .insert_event(day_start + 9 * 3_600, EventKind::SegmentStart)
                .unwrap();
            storage
                .insert_event(day_start + 12 * 3_600, EventKind::SegmentEnd)
                .unwrap();
            storage
                .insert_event(day_start + 17 * 3_600, EventKind::ClockOut)
                .unwrap();
        }
        let sum = day_detail_inner(&ctx, 0, now).unwrap();
        assert_eq!(sum.duty_secs, 8 * 3_600);
        assert_eq!(sum.work_secs, 3 * 3_600);
        assert_eq!(sum.rest_secs, 5 * 3_600);
        assert_eq!(sum.blocks.len(), 2);
        assert_eq!(sum.duty_started_at, Some(day_start + 9 * 3_600));
        assert_eq!(sum.duty_ended_at, Some(day_start + 17 * 3_600));
    }

    /// day_detail 前后日翻看：昨日事件只在 offset = -1 可见，今日为空日。
    #[test]
    fn day_detail_offset_browses_days() {
        let ctx = ctx(FakeClock::new());
        let now = Local::now().timestamp();
        let day_start = period::day_start_secs(&Local::now());
        let yesterday_in = day_start - 86_400 + 9 * 3_600;
        {
            let storage = ctx.storage.lock().unwrap();
            storage
                .insert_event(yesterday_in, EventKind::ClockIn)
                .unwrap();
            storage
                .insert_event(yesterday_in + 3_600, EventKind::ClockOut)
                .unwrap();
        }
        assert_eq!(day_detail_inner(&ctx, 0, now).unwrap().duty_secs, 0);
        assert_eq!(day_detail_inner(&ctx, -1, now).unwrap().duty_secs, 3_600);
    }

    /// 自动下班：到点以回填时刻（上班 + N）关班，迟到发现账目准时；未到点/未上班不动。
    #[test]
    fn auto_clock_out_backfills_at_threshold() {
        let ctx = ctx(FakeClock::new());
        let in_at = wall_now_secs().unwrap() - 9 * 3_600;
        clock_in_inner(&ctx, in_at).unwrap();
        assert_eq!(try_auto_clock_out(&ctx).unwrap(), Some(in_at + 8 * 3_600));
        assert_eq!(
            ctx.storage.lock().unwrap().workday_latest().unwrap(),
            Some((1, in_at, Some(in_at + 8 * 3_600))),
            "记账时刻 = 上班 + 8h，与发现时刻无关"
        );
        assert_eq!(*ctx.workday.lock().unwrap(), WorkdayState::Off);

        // 新班 1 小时前上班：未到点不触发
        clock_in_inner(&ctx, wall_now_secs().unwrap() - 3_600).unwrap();
        assert_eq!(try_auto_clock_out(&ctx).unwrap(), None);
        assert_eq!(
            ctx.storage.lock().unwrap().workday_latest().unwrap(),
            Some((2, wall_now_secs().unwrap() - 3_600, None))
        );

        // 未上班：不动
        clock_out_inner(&ctx, wall_now_secs().unwrap(), EventKind::ClockOut).unwrap();
        assert_eq!(try_auto_clock_out(&ctx).unwrap(), None);
    }
}
