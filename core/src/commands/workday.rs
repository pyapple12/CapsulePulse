//! 打卡命令层（PL005）：上班/下班/单日明细与自动下班检查——转发工作日状态机 + 落库 + 事件留痕。
//! 事务次序定案：库写入先行、状态转移殿后（中途失败不污染内存态，可安全重试）；
//! 自动下班以"上班 + N"回填时刻关班（发现可迟到、账目准时）。锁序见 AppContext 注释。

use chrono::{Datelike, Local, TimeZone};
use serde::Serialize;
use tauri::State;

use super::{lock, poison, wall_now_secs, AppContext, CommandError};
use crate::period;
use crate::session::Clock;
use crate::workday::{auto_out_due, reduce_day, DaySummary, EventKind, WorkdayError, WorkdayState};

/// 上班打卡：开行 + clock_in 事件入馆（单事务，FIX002.9），再转移状态；会话复位 Idle
/// （上班 = 新一天计时从零，此前累计均已落库，无数据丢失）。
/// # 错误
/// 已在岗返回 [`WorkdayError::AlreadyOnDuty`]；锁中毒/存储失败照常上抛。
pub(super) fn clock_in_inner<C: Clock>(ctx: &AppContext<C>, at: i64) -> Result<i64, CommandError> {
    // workday 锁全程持有：check-then-act 原子化（防并发双击开出两行）
    let mut workday = poison("工作日", ctx.workday.lock())?;
    if workday.is_on_duty() {
        return Err(CommandError::Workday(WorkdayError::AlreadyOnDuty));
    }
    {
        let mut session = lock(ctx)?;
        session.reset();
    }
    let id = poison("存储", ctx.storage.lock())?.workday_open_with_event(at, EventKind::ClockIn)?;
    workday.clock_in(at, id)?;
    Ok(id)
}

/// 下班打卡：班内运行段先收段落库（FIX002.4：自动下班路径段末随下班记回填时刻），
/// 关行 + 下班事件单事务入馆（at 为记账时刻——手动 = 当前、自动 = 上班 + N 回填），
/// 再转移状态；会话复位 Idle（累计已全部落库）。
/// # 错误
/// 未上班返回 [`WorkdayError::NotOnDuty`]；锁中毒/存储失败照常上抛。
pub(super) fn clock_out_inner<C: Clock>(
    ctx: &AppContext<C>,
    at: i64,
    kind: EventKind,
) -> Result<(), CommandError> {
    let mut workday = poison("工作日", ctx.workday.lock())?;
    let WorkdayState::OnDuty { clock_in_at: _, id } = *workday else {
        return Err(CommandError::Workday(WorkdayError::NotOnDuty));
    };
    // 自动下班：段末事件随下班一并记回填时刻（与纯归约契约测试口径一致）
    let pause_at = matches!(kind, EventKind::AutoClockOut).then_some(at);
    super::session::close_running_segment(ctx, pause_at)?;
    poison("存储", ctx.storage.lock())?.workday_close_with_event(id, at, kind)?;
    workday.clock_out()?;
    {
        let mut session = lock(ctx)?;
        session.reset();
    }
    Ok(())
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
/// 单日事件拉取起点：在岗且上班时刻早于当日零点（跨夜班）时从上班时刻取（PL005 口径，
/// day_detail 与 week_detail 共用；取锁顺序恒 workday → storage，用后即还）
fn day_fetch_start<C: Clock>(ctx: &AppContext<C>, day_start: i64) -> Result<i64, CommandError> {
    let workday = poison("工作日", ctx.workday.lock())?;
    Ok(match *workday {
        WorkdayState::OnDuty { clock_in_at, .. } => day_start.min(clock_in_at),
        WorkdayState::Off => day_start,
    })
}

/// 单日摘要管道（day/week 两视图单源，FIX005.4）：日界 → 在岗起点 → 事件区间 → 归约，
/// 四步只此一处——口径调整改这里即两视图同变，杜绝逐字复制漂移。
pub(super) fn day_summary_inner<C: Clock>(
    ctx: &AppContext<C>,
    offset: i64,
    now_secs: i64,
) -> Result<DaySummary, CommandError> {
    let (day_start, day_end) = day_bounds(offset, now_secs)?;
    let fetch_start = day_fetch_start(ctx, day_start)?;
    let events = poison("存储", ctx.storage.lock())?.events_between(fetch_start, day_end)?;
    Ok(reduce_day(&events, day_start, day_end, now_secs))
}

/// 周视图行（serde 单一来源，TS 侧镜像 WeekDay）：日期标签 + 星期序 + workday 口径两值。
#[derive(Debug, Serialize)]
pub struct WeekDay {
    /// 日期标签（MM-DD）
    pub date: String,
    /// 星期序（1 = 周一 … 7 = 周日，前端映射单字标签）
    pub weekday: u32,
    /// 当日工作秒数
    pub work_secs: i64,
    /// 当日在岗秒数
    pub duty_secs: i64,
}

/// 周视图装配（PL022 自然周）：查看日所在自然周（周一锚定最左、周日最右），恒 7 行；
/// 逐日走 day_summary_inner 管道（口径与 day_detail 代码级单源，FIX005.4）；
/// 未来日无事件天然零值；日期标签在本层摘取，不跨日持锁。
pub(super) fn week_detail_inner<C: Clock>(
    ctx: &AppContext<C>,
    offset: i64,
    now_secs: i64,
) -> Result<Vec<WeekDay>, CommandError> {
    let (view_start, _) = day_bounds(offset, now_secs)?;
    let monday = monday_offset(offset, view_start)?;
    (0..7)
        .map(|i| {
            let day_offset = monday + i;
            let (day_start, _) = day_bounds(day_offset, now_secs)?;
            let start_dt = Local
                .timestamp_opt(day_start, 0)
                .single()
                .ok_or(CommandError::Clock)?;
            let summary = day_summary_inner(ctx, day_offset, now_secs)?;
            Ok(WeekDay {
                date: start_dt.format("%m-%d").to_string(),
                weekday: (i + 1) as u32,
                work_secs: summary.work_secs,
                duty_secs: summary.duty_secs,
            })
        })
        .collect()
}

/// 查看日所在自然周的周一偏移：view_offset − (查看日星期序 − 1)（周一=1 … 周日=7）。
fn monday_offset(view_offset: i64, view_day_start: i64) -> Result<i64, CommandError> {
    let dt = Local
        .timestamp_opt(view_day_start, 0)
        .single()
        .ok_or(CommandError::Clock)?;
    Ok(view_offset - (i64::from(dt.weekday().number_from_monday()) - 1))
}

/// 总日均聚合成品（serde 单一来源，TS 侧镜像 WorkdayTotal）：全历史工作总和 ÷ 有数据天数。
#[derive(Debug, Serialize)]
pub struct WorkdayTotal {
    /// 全历史工作总秒数（各日 work_secs 之和；口径与 day_summary_inner 代码级单源）。
    pub work_secs: i64,
    /// 有数据天数（当日存在打卡/计时记录，即 duty_secs > 0 的本地日；空日不计）。
    pub days: u32,
}

/// 任意时刻所在本地日的零点。
fn local_day_start(at: i64) -> Result<i64, CommandError> {
    let dt = Local
        .timestamp_opt(at, 0)
        .single()
        .ok_or(CommandError::Clock)?;
    Ok(period::day_start_secs(&dt))
}

/// 总日均（PL022）：从全历史首条记录所在本地日逐日走 day_summary_inner 累加——
/// 聚合落在命令层以复用同一日管道（真单源），storage 仅提供全历史起点；
/// 天数按"该日 duty_secs > 0"计（有打卡/计时记录的日子），空日不计。
pub(super) fn workday_total_inner<C: Clock>(
    ctx: &AppContext<C>,
    now_secs: i64,
) -> Result<WorkdayTotal, CommandError> {
    let Some(first_at) = poison("存储", ctx.storage.lock())?.first_record_at()? else {
        return Ok(WorkdayTotal {
            work_secs: 0,
            days: 0,
        });
    };
    let first_day = local_day_start(first_at)?;
    let today = local_day_start(now_secs)?;
    let first_dt = Local
        .timestamp_opt(first_day, 0)
        .single()
        .ok_or(CommandError::Clock)?;
    let today_dt = Local
        .timestamp_opt(today, 0)
        .single()
        .ok_or(CommandError::Clock)?;
    let span = today_dt
        .date_naive()
        .signed_duration_since(first_dt.date_naive())
        .num_days();

    let mut total = WorkdayTotal {
        work_secs: 0,
        days: 0,
    };
    for step in 0..=span {
        let summary = day_summary_inner(ctx, -step, now_secs)?;
        total.work_secs += summary.work_secs;
        if summary.duty_secs > 0 {
            total.days += 1;
        }
    }
    Ok(total)
}

/// 自动下班检查（挂 session_status 评估口）：在岗且满 N 小时 → 以回填时刻执行下班，
/// 返回回填时刻供调用方发 workday-auto-out 事件；未在岗/未到点返回 None。
pub(super) fn try_auto_clock_out<C: Clock>(
    ctx: &AppContext<C>,
) -> Result<Option<i64>, CommandError> {
    let due_at = {
        let workday = poison("工作日", ctx.workday.lock())?;
        match *workday {
            WorkdayState::OnDuty { clock_in_at, .. } => {
                let hours = poison("设置", ctx.settings.lock())?.workday_auto_out_hours;
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
    let workday = poison("工作日", ctx.workday.lock())?;
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

/// 单日明细（offset 缺省 0 = 今日；前后日翻看传 ±N，越界严格报错）。
/// async：events 全表扫 + 归约移出主线程（FIX002.8）。
#[tauri::command]
pub async fn day_detail(
    handle: State<'_, AppContext>,
    offset: Option<i64>,
) -> Result<DaySummary, CommandError> {
    day_summary_inner(&handle, checked_offset(offset)?, wall_now_secs()?)
}

/// 周视图（PL022）：查看日所在自然周（周一~周日），随 offset 联动（前端必传）。
/// async：同 day_detail 移出主线程。
#[tauri::command]
pub async fn week_detail(
    handle: State<'_, AppContext>,
    offset: i64,
) -> Result<Vec<WeekDay>, CommandError> {
    week_detail_inner(&handle, checked_offset(Some(offset))?, wall_now_secs()?)
}

/// 总日均（PL022）：全历史工作时长总和 + 有数据天数。async：同 day_detail 移出主线程。
#[tauri::command]
pub async fn workday_total(handle: State<'_, AppContext>) -> Result<WorkdayTotal, CommandError> {
    workday_total_inner(&handle, wall_now_secs()?)
}

/// 入参校验（FIX002.5）：offset 限定 ±366——极端值会使 chrono 日历加法 panic，
/// 前端正常只传 0/±1，此处拦的是 devtools/误用面。
fn checked_offset(offset: Option<i64>) -> Result<i64, CommandError> {
    let offset = offset.unwrap_or(0);
    if (-366..=366).contains(&offset) {
        Ok(offset)
    } else {
        Err(CommandError::InvalidOffset(offset))
    }
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

    /// FIX002.4：自动下班路径段末事件随下班记回填时刻（与纯归约契约测试口径一致）。
    #[test]
    fn auto_pause_marks_segment_end_at_backfill() {
        let ctx = ctx(FakeClock::new());
        let in_at = wall_now_secs().unwrap() - 9 * 3_600;
        clock_in_inner(&ctx, in_at).unwrap();
        super::super::session::start_session(&ctx).unwrap();
        let out_at = in_at + 8 * 3_600;
        assert!(try_auto_clock_out(&ctx).unwrap().is_some());
        let rows = ctx
            .storage
            .lock()
            .unwrap()
            .events_between(0, i64::MAX)
            .unwrap();
        assert!(
            rows.contains(&(out_at, EventKind::SegmentEnd)),
            "段末事件记回填时刻而非发现时刻：{rows:?}"
        );
        assert!(rows.contains(&(out_at, EventKind::AutoClockOut)));
    }

    /// FIX002.5：offset 越界严格报错（拦 chrono 日历加法 panic 面），合法值放行。
    #[test]
    fn checked_offset_bounds() {
        assert_eq!(checked_offset(None).unwrap(), 0);
        assert_eq!(checked_offset(Some(-366)).unwrap(), -366);
        assert_eq!(checked_offset(Some(366)).unwrap(), 366);
        assert!(matches!(
            checked_offset(Some(-367)),
            Err(CommandError::InvalidOffset(-367))
        ));
        assert!(matches!(
            checked_offset(Some(1_000_000)),
            Err(CommandError::InvalidOffset(1_000_000))
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
        let sum = day_summary_inner(&ctx, 0, now).unwrap();
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
        assert_eq!(day_summary_inner(&ctx, 0, now).unwrap().duty_secs, 0);
        assert_eq!(day_summary_inner(&ctx, -1, now).unwrap().duty_secs, 3_600);
    }

    /// 把某"今日零点 + N 日"的秒时间戳格式化为 MM-DD（周用例的期望值锚）。
    fn mmdd(ts: i64) -> String {
        Local
            .timestamp_opt(ts, 0)
            .single()
            .unwrap()
            .format("%m-%d")
            .to_string()
    }

    /// 本周三锚点：(零点, 周三 12:00)——以真实今日推算，用例不依赖运行日。
    fn wednesday_anchor() -> (i64, i64) {
        let today = period::day_start_secs(&Local::now());
        let idx = i64::from(Local::now().weekday().num_days_from_monday());
        let wed = today + (2 - idx) * 86_400;
        (wed, wed + 12 * 3_600)
    }

    /// 在"今日零点 + day_shift 日"种一个完整班：上班 09:00、工作 [09:00, +work)、下班 09:00+duty。
    fn seed_day<C: Clock>(ctx: &AppContext<C>, day_shift: i64, work: i64, duty: i64) {
        let base = period::day_start_secs(&Local::now()) + day_shift * 86_400 + 9 * 3_600;
        let storage = ctx.storage.lock().unwrap();
        storage.insert_event(base, EventKind::ClockIn).unwrap();
        storage.insert_event(base, EventKind::SegmentStart).unwrap();
        storage
            .insert_event(base + work, EventKind::SegmentEnd)
            .unwrap();
        storage
            .insert_event(base + duty, EventKind::ClockOut)
            .unwrap();
    }

    /// PL022.1：自然周锚定——查看日所在周固定返回周一~周日 7 行，行序即真实星期序。
    #[test]
    fn week_detail_natural_week_alignment() {
        let ctx = ctx(FakeClock::new());
        let (wed, now) = wednesday_anchor();
        let monday = wed - 2 * 86_400;
        let week = week_detail_inner(&ctx, 0, now).unwrap();
        assert_eq!(week.len(), 7, "自然周恒 7 行");
        for (i, row) in week.iter().enumerate() {
            assert_eq!(row.weekday, (i + 1) as u32, "行序 = 周一~周日");
            assert_eq!(row.date, mmdd(monday + i as i64 * 86_400));
        }
    }

    /// PL022.1：未来日零值——只种本周一，周三查看时本周余下各日（含未来）两值皆零。
    #[test]
    fn week_detail_future_days_zero() {
        let ctx = ctx(FakeClock::new());
        let (wed, now) = wednesday_anchor();
        let monday_shift = -i64::from(Local::now().weekday().num_days_from_monday());
        seed_day(&ctx, monday_shift, 2 * 3_600, 3 * 3_600);
        let week = week_detail_inner(&ctx, 0, now).unwrap();
        let monday = week[0].date.clone();
        assert_eq!(monday, mmdd(wed - 2 * 86_400));
        assert_eq!(week[0].work_secs, 2 * 3_600);
        assert_eq!(week[0].duty_secs, 3 * 3_600);
        assert!(
            week[1..]
                .iter()
                .all(|d| d.work_secs == 0 && d.duty_secs == 0),
            "本周余下各日全零：{week:?}"
        );
    }

    /// PL022.1：跨周联动——offset −7 整窗切到上一自然周（周一~周日），本周不受上周数据影响。
    #[test]
    fn week_detail_offset_cross_week() {
        let ctx = ctx(FakeClock::new());
        let (wed, now) = wednesday_anchor();
        // 上一自然周周日 = 本周一 − 1 日：工作 1h / 在岗 2h
        let last_sunday = wed - 2 * 86_400 - 86_400;
        {
            let storage = ctx.storage.lock().unwrap();
            storage
                .insert_event(last_sunday + 9 * 3_600, EventKind::ClockIn)
                .unwrap();
            storage
                .insert_event(last_sunday + 9 * 3_600, EventKind::SegmentStart)
                .unwrap();
            storage
                .insert_event(last_sunday + 10 * 3_600, EventKind::SegmentEnd)
                .unwrap();
            storage
                .insert_event(last_sunday + 11 * 3_600, EventKind::ClockOut)
                .unwrap();
        }
        let prev = week_detail_inner(&ctx, -7, now).unwrap();
        assert_eq!(prev.len(), 7);
        assert_eq!(prev[0].weekday, 1);
        assert_eq!(prev[0].date, mmdd(last_sunday - 6 * 86_400));
        assert_eq!(prev[6].weekday, 7);
        assert_eq!(prev[6].date, mmdd(last_sunday));
        assert_eq!(prev[6].work_secs, 3_600);
        assert_eq!(prev[6].duty_secs, 2 * 3_600);
        // 本周窗口不含上周数据
        let cur = week_detail_inner(&ctx, 0, now).unwrap();
        assert!(cur.iter().all(|d| d.work_secs == 0 && d.duty_secs == 0));
    }

    /// PL022.1：总日均聚合——全历史工作总秒数 + 有数据天数（空日不计；空库零）。
    #[test]
    fn workday_total_sums_all_history() {
        let empty_ctx = ctx(FakeClock::new());
        let total = workday_total_inner(&empty_ctx, Local::now().timestamp()).unwrap();
        assert_eq!(total.work_secs, 0);
        assert_eq!(total.days, 0, "空库零值不 panic");

        let ctx = ctx(FakeClock::new());
        seed_day(&ctx, -2, 2 * 3_600, 3 * 3_600);
        // 中间（-1 日）为空日：不计入天数
        seed_day(&ctx, 0, 3_600, 2 * 3_600);
        let total = workday_total_inner(&ctx, Local::now().timestamp()).unwrap();
        assert_eq!(total.work_secs, 3 * 3_600);
        assert_eq!(total.days, 2, "仅两个有数据日");
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
