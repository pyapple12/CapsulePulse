//! 打卡命令层（PL005）：上班/下班/单日明细与自动下班检查——转发工作日状态机 + 落库 + 事件留痕。
//! 事务次序定案：库写入先行、状态转移殿后（中途失败不污染内存态，可安全重试）；
//! 自动下班以"上班 + N"回填时刻关班（发现可迟到、账目准时）。锁序见 AppContext 注释。

use std::time::Duration;

use chrono::{Datelike, Local, TimeZone};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use super::{lock, now_secs, poison, AppContext, CommandError};
use crate::period;
use crate::session::Clock;
use crate::storage::{MigrationReport, RuntimeMode, RuntimeState};
use crate::workday::{
    auto_out_pending, auto_out_step, decide_recovery, duty_windows_for_day, nested_clock_in_fixes,
    reduce_day, AutoOutStep, DaySummary, EventKind, RecoveryDecision, RecoveryInput, WorkdayError,
    WorkdayState, OVERTIME_CAP_HOURS,
};

/// 上班打卡：流水入馆（`clock_in` + `segment_start`，单事务），再转移状态；会话归零并**立即进入
/// 工作态**（PATCH002：打卡即工作，大计时器从 0 起跑；此前累计均已落库，无数据丢失）。
/// # 错误
/// 已在岗返回 [`WorkdayError::AlreadyOnDuty`]；锁中毒/存储失败照常上抛。
pub(super) fn clock_in_inner<C: Clock>(ctx: &AppContext<C>, at: i64) -> Result<(), CommandError> {
    // workday 锁全程持有：check-then-act 原子化（防并发双击开出两行）。
    // PL027.6 双源守卫：内存态**或**库中"最后一条打卡未配对"即在岗——旧实现只查内存，
    // 一旦内存态与库不同步（异常路径 / 双实例）就能开出重叠班（今天库里的 id 36 即此形态）。
    let mut workday = poison("工作日", ctx.workday.lock())?;
    if workday.is_on_duty()
        || poison("存储", ctx.storage.lock())?
            .open_duty_clock_in()?
            .is_some()
    {
        return Err(CommandError::Workday(WorkdayError::AlreadyOnDuty));
    }
    // 预算上下文在**插入打卡事件之前**取（PATCH005.3 到顶守卫）：此刻尚无开窗 ⇒
    // `used_total_secs` 就是"本班之前已累计"，守卫与武装共用同一值（只取一次数）。
    let bc = day_budget_context(ctx, at, at)?;
    let used_before = bc.used_total_secs;
    // 到顶守卫（PATCH005.3）：今日已累计到 12h 硬顶 → 本数据日不再受理打卡
    // （用户定案：前端 pill 置灰点不开；后端再兜一层，防 devtools / DEV 基座绕过）。
    if used_before >= i64::from(OVERTIME_CAP_HOURS) * 3_600 {
        return Err(CommandError::DayBudgetExhausted(OVERTIME_CAP_HOURS));
    }
    {
        // 打卡即工作（PATCH002）：归零 + 立即 Running，同一 guard 内完成不留中间态；
        // 锁序 workday → session → storage 单向（见 AppContext 注释）。
        let mut session = lock(ctx)?;
        session.reset();
        session.start()?;
    }
    poison("存储", ctx.storage.lock())?.insert_events(&[
        // 打卡即工作（PATCH002）：同一事务同时记 clock_in 与段起点——图谱/明细从打卡那一刻
        // 就有工作段（否则该窗口会整段算休息）
        (at, EventKind::ClockIn),
        (at, EventKind::SegmentStart),
    ])?;
    workday.clock_in(at)?;
    // 武装自动下班基准（PATCH005.3）：到点时刻 = 本班打卡 + **今日剩余**（今日上限 − 本班前已累计）；
    // 若今日预算已用满 → 本次打卡即**隐式加班**：落 `overtime` 并按下一步步进武装（用户定案：
    // 打卡框不加解释性文案，看蓝条即知）。兜底线程由命令层 spawn。
    let mut steps = bc.overtime_steps;
    let mut budget = crate::workday::day_budget_hours(bc.basis_hours, steps);
    if crate::workday::shift_remaining_secs(budget, used_before) == 0 {
        poison("存储", ctx.storage.lock())?.insert_event(at, EventKind::Overtime)?;
        steps += 1;
        budget = crate::workday::day_budget_hours(bc.basis_hours, steps);
    }
    let remaining = crate::workday::shift_remaining_secs(budget, used_before);
    set_auto_out_watch(ctx, crate::workday::shift_deadline(at, remaining))?;
    // 打卡即刷心跳（PL027.3）：崩溃恢复据此知道"在岗段从哪起、最后活着是什么时候"
    beat_after_action(ctx);
    Ok(())
}

/// 下班打卡：班内运行段先收段（自动下班路径段末随下班记回填时刻），下班事件入馆
/// （at 为记账时刻——手动 = 当前、自动 = 上班 + N 回填），再转移状态；会话复位 Idle。
/// # 错误
/// 未上班返回 [`WorkdayError::NotOnDuty`]；锁中毒/存储失败照常上抛。
pub(super) fn clock_out_inner<C: Clock>(
    ctx: &AppContext<C>,
    at: i64,
    kind: EventKind,
) -> Result<(), CommandError> {
    let mut workday = poison("工作日", ctx.workday.lock())?;
    let WorkdayState::OnDuty { clock_in_at: _ } = *workday else {
        return Err(CommandError::Workday(WorkdayError::NotOnDuty));
    };
    // 段末事件与下班时刻取同一基准（PL027：段时长由流水配对推出，两者必须同刻）；
    // 手动下班 = 当前时刻、自动下班 = 上班 + N 回填时刻。
    let pause_at = Some(at);
    super::session::close_running_segment(ctx, pause_at)?;
    poison("存储", ctx.storage.lock())?.insert_events(&[(at, kind)])?;
    workday.clock_out()?;
    {
        let mut session = lock(ctx)?;
        session.reset();
    }
    // 下班即撤基准（PL025.8）：代次 +1 使兜底线程退出
    clear_auto_out_watch(ctx)?;
    // 下班清运行态缓存（PL027.3）：未上班无在岗段可恢复，缓存留着反而会被误读
    if let Err(err) = poison("存储", ctx.storage.lock())?.runtime_clear() {
        crate::warn_diag(&format!("运行态缓存清理失败（忽略）：{err}"));
    }
    // 下班后顺手滚一轮按天汇总（PL027.7）：本班所在数据日此刻仍算"今天"不会滚，
    // 但更早的未滚日会被带走；失败仅落档（汇总只是加速层）
    if let Err(err) = seal_days_inner(ctx, at) {
        crate::diag::log(&format!("按天汇总失败（下班后忽略）：{err}"));
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

/// 单日摘要管道（day/week 两视图单源，FIX005.4）：日界 → 由流水推在岗窗口 → 事件区间 → 归约，
/// 四步只此一处——口径调整改这里即两视图同变，杜绝逐字复制漂移。
/// PL027.6 单一事实源：**窗口由流水推**（`duty_windows_for_day`：打卡开窗、下班事件关窗；
/// 未关的下一班打卡视为前班在该刻结束；末班未关闭于 now），不再读 workdays 行；
/// 取事件区间 = 本数据日零点起，至"日末 + 自动下班上限 12h"（跨零点班的关窗事件因此天然落在
/// 区间内；不按 now 夹取——生产不存在未来事件，夹取只会误裁同日闭窗），再按窗口起止取归约段事件。
pub(super) fn day_summary_inner<C: Clock>(
    ctx: &AppContext<C>,
    offset: i64,
    now_secs: i64,
) -> Result<DaySummary, CommandError> {
    let (day_start, day_end) = day_bounds(offset, now_secs)?;
    let window_fetch_end = day_end + i64::from(OVERTIME_CAP_HOURS) * 3_600 + 1;
    let duty_events = poison("存储", ctx.storage.lock())?
        .events_between(day_start, window_fetch_end.max(day_start + 1))?;
    let (windows, open_tail) = duty_windows_for_day(&duty_events, day_start, day_end, now_secs);
    let Some(&(fetch_start, _)) = windows.first() else {
        return Ok(reduce_day(&[], &[], false));
    };
    let fetch_end = windows.last().map(|&(_, end)| end).unwrap_or(now_secs) + 1;
    let seg_events = poison("存储", ctx.storage.lock())?.events_between(fetch_start, fetch_end)?;
    // PATCH005.3 / PATCH005.4：预算字段（上限 / 已用 / 剩余）填**该 offset 是否为当前数据日**——
    // 判据取自**数据本身**（该日是否有未闭合的班 = `open_tail`）而非工作日锁：`clock_out_inner`
    // 持有「工作日」锁时经 `seal_days_inner` 调到这里，若再取该锁即自死锁（Mutex 非重入）。
    // 口径：跨零点夜班的数据日是"打卡那一天"（−1，末窗未关）；`used_secs` 在"日历今日或末窗未关"
    // 时填（到顶置灰的取数源），`budget/remaining` 仅在岗（末窗未关）时填——暂下班不画蓝条、
    // 轴末取末段末尾。
    let mut summary = reduce_day(&windows, &seg_events, open_tail);
    if offset == 0 || open_tail {
        summary.used_secs = crate::workday::day_used_secs(&windows);
        if open_tail {
            if let Some(&(shift_start, _)) = windows.last() {
                let bc = day_budget_context(ctx, shift_start, now_secs)?;
                let budget = crate::workday::day_budget_hours(bc.basis_hours, bc.overtime_steps);
                summary.budget_hours = budget;
                summary.remaining_secs =
                    crate::workday::shift_remaining_secs(budget, bc.used_before_shift_secs);
            }
        }
    }
    Ok(summary)
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
/// 每一天**已封口且滚过 → 读 `days` 汇总**（PL027.7 加速），否则走 day_summary_inner 现算
/// （今天 / 未封口日）——两条路同一口径（口径与 day_detail 代码级单源，FIX005.4）；
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
            let rollup = poison("存储", ctx.storage.lock())?
                .day_get(&start_dt.format("%Y-%m-%d").to_string())?;
            let (work_secs, duty_secs) = match rollup {
                Some(row) => (row.work_secs, row.duty_secs),
                None => {
                    let summary = day_summary_inner(ctx, day_offset, now_secs)?;
                    (summary.work_secs, summary.duty_secs)
                }
            };
            Ok(WeekDay {
                date: start_dt.format("%m-%d").to_string(),
                weekday: (i + 1) as u32,
                work_secs,
                duty_secs,
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

/// 全历史聚合（serde 单一来源，TS 侧镜像 WorkdayTotal）：三值总和 + 有数据天数。
/// 前端据三值总和 ÷ 天数得各值平均（PL022 总日均取 work_secs；PL024.6 三值百分比取三者）。
#[derive(Debug, Serialize)]
pub struct WorkdayTotal {
    /// 全历史在岗总秒数（各日 duty_secs 之和；口径与 day_summary_inner 代码级单源）。
    pub duty_secs: i64,
    /// 全历史工作总秒数（各日 work_secs 之和）。
    pub work_secs: i64,
    /// 全历史休息总秒数（各日 rest_secs 之和）。
    pub rest_secs: i64,
    /// 有数据天数（当日存在打卡/计时记录，即 duty_secs > 0 的本地日；空日不计）。
    pub days: u32,
    /// 最早记录所在本地日相对今天的偏移（负值或 0；空库 None）——统计"往前翻"的下界
    /// （PL025.1：最早一条记录就是往前翻的最后一条）。
    pub first_offset: Option<i64>,
}

/// 任意时刻所在本地日的零点。
pub(super) fn local_day_start(at: i64) -> Result<i64, CommandError> {
    let dt = Local
        .timestamp_opt(at, 0)
        .single()
        .ok_or(CommandError::Clock)?;
    Ok(period::day_start_secs(&dt))
}

/// 当前数据日偏移（PATCH005.4 **单一来源**）：在岗 → 该班打卡日相对 `now` 所在日历日的天数偏移
/// （0 = 同日、−1 = 跨零点夜班锚定昨天；未在岗 → 0）。判据 = **有未闭合的班**——时钟与统计据此
/// 锚定"进行中的数据日"，跨零点夜班只要没下班就不切换（`now` 由调用方给出，与本次取数同刻，
/// 避免与显式时间基准错位）。**注意：不在地锁持有期内调用**（`clock_out_inner` 持「工作日」锁
/// 经 `seal_days_inner` → `day_summary_inner` 的路径不再取此锁，见该处注释）。
pub(super) fn data_day_offset<C: Clock>(
    ctx: &AppContext<C>,
    now: i64,
) -> Result<i64, CommandError> {
    let clock_in_at = match *poison("工作日", ctx.workday.lock())? {
        WorkdayState::OnDuty { clock_in_at } => clock_in_at,
        WorkdayState::Off => return Ok(0),
    };
    Ok((local_day_start(clock_in_at)? - local_day_start(now)?) / 86_400)
}

/// 惰性按天汇总（PL027.7）：把"数据日已封口但还没滚"的过去日滚进 `days` 表——**过滤条件 =
/// 该打卡日的所有班都已下班（`duty_ended_at.is_some()`）且不是今天**；空日不占行；
/// 已滚过跳过（幂等）；逐日单事务。触发点 = 启动 + 每次下班后（不设定时器）。
/// 汇总只作加速：与 `day_summary_inner` 逐日现算全等，单日视图/图谱仍走流水。
/// # 返回
/// 本轮新滚的天数。
pub(crate) fn seal_days_inner<C: Clock>(
    ctx: &AppContext<C>,
    now_secs: i64,
) -> Result<u32, CommandError> {
    let Some(first_at) = poison("存储", ctx.storage.lock())?.first_record_at()? else {
        return Ok(0);
    };
    let first_day = local_day_start(first_at)?;
    let today = local_day_start(now_secs)?;
    let span = Local
        .timestamp_opt(today, 0)
        .single()
        .ok_or(CommandError::Clock)?
        .date_naive()
        .signed_duration_since(
            Local
                .timestamp_opt(first_day, 0)
                .single()
                .ok_or(CommandError::Clock)?
                .date_naive(),
        )
        .num_days();

    let mut rolled = 0u32;
    for step in 1..=span {
        let offset = -step;
        let (day_start, _) = day_bounds(offset, now_secs)?;
        let date = Local
            .timestamp_opt(day_start, 0)
            .single()
            .ok_or(CommandError::Clock)?
            .format("%Y-%m-%d")
            .to_string();
        if poison("存储", ctx.storage.lock())?
            .day_get(&date)?
            .is_some()
        {
            continue; // 已滚过（幂等）
        }
        let summary = day_summary_inner(ctx, offset, now_secs)?;
        if summary.duty_started_at.is_none() || summary.duty_ended_at.is_none() {
            continue; // 空日 / 末窗未关（未封口）不滚
        }
        poison("存储", ctx.storage.lock())?.day_upsert(&crate::storage::DayRollup {
            date,
            duty_secs: summary.duty_secs,
            work_secs: summary.work_secs,
            rest_secs: summary.rest_secs,
            first_in_at: summary.duty_started_at,
            last_out_at: summary.duty_ended_at,
            sealed_at: now_secs,
        })?;
        rolled += 1;
    }
    Ok(rolled)
}

/// 全历史聚合（PL022 总日均 / PL024.6 三值平均）：**已封口日读 `days` 汇总（加速）**，
/// 未滚的日子只有两类——今天，以及"末窗未关"的那一天（跨零点在岗班；PL027.9 归一化后
/// 至多一个）——它们走 `day_summary_inner` 现算；天数按"该日 duty_secs > 0"计，空日不计。
pub(super) fn workday_total_inner<C: Clock>(
    ctx: &AppContext<C>,
    now_secs: i64,
) -> Result<WorkdayTotal, CommandError> {
    let Some(first_at) = poison("存储", ctx.storage.lock())?.first_record_at()? else {
        return Ok(WorkdayTotal {
            duty_secs: 0,
            work_secs: 0,
            rest_secs: 0,
            days: 0,
            first_offset: None,
        });
    };
    // 先滚一轮（惰性）：保证"过去日已封口的都进了汇总"
    seal_days_inner(ctx, now_secs)?;
    let totals = poison("存储", ctx.storage.lock())?.days_totals()?;
    let mut total = WorkdayTotal {
        duty_secs: totals.duty_secs,
        work_secs: totals.work_secs,
        rest_secs: totals.rest_secs,
        days: totals.days,
        first_offset: Some(-{
            let first_day = local_day_start(first_at)?;
            let today = local_day_start(now_secs)?;
            Local
                .timestamp_opt(today, 0)
                .single()
                .ok_or(CommandError::Clock)?
                .date_naive()
                .signed_duration_since(
                    Local
                        .timestamp_opt(first_day, 0)
                        .single()
                        .ok_or(CommandError::Clock)?
                        .date_naive(),
                )
                .num_days()
        }),
    };
    // 今天（永不滚）+ 未封口的在岗班所在日（可能是过去日：跨零点班）
    let today = local_day_start(now_secs)?;
    let open_day = poison("存储", ctx.storage.lock())?
        .open_duty_clock_in()?
        .map(local_day_start)
        .transpose()?;
    let mut live_days: Vec<i64> = vec![today];
    if let Some(day) = open_day {
        if !live_days.contains(&day) {
            live_days.push(day);
        }
    }
    for day in live_days {
        let offset = (day - today) / 86_400;
        let summary = day_summary_inner(ctx, offset, now_secs)?;
        total.duty_secs += summary.duty_secs;
        total.work_secs += summary.work_secs;
        total.rest_secs += summary.rest_secs;
        if summary.duty_secs > 0 {
            total.days += 1;
        }
    }
    Ok(total)
}

/// 脏数据归一化（PL027.9）：把历史/异常路径遗留的"嵌套打卡"收敛为"前班在新打卡刻结束"——
/// 追加纠正事件（段末 + 下班，同一时刻）+ 写 `diag` 报告。启动时跑一次；
/// **幂等**（纠正后再跑为空）。失败不阻断启动（由调用方降级）。
/// # 返回
/// 追加的纠正事件条数（0 = 干净）。
pub(crate) fn normalize_dirty_data_inner<C: Clock>(
    ctx: &AppContext<C>,
) -> Result<u32, CommandError> {
    let events = poison("存储", ctx.storage.lock())?.events_between(i64::MIN / 2, i64::MAX)?;
    let fixes = nested_clock_in_fixes(&events);
    if fixes.is_empty() {
        return Ok(0);
    }
    poison("存储", ctx.storage.lock())?.insert_events(&fixes)?;
    crate::warn_diag(&format!(
        "脏数据归一化：追加 {} 条纠正事件（嵌套打卡收敛为「前班在新打卡刻结束」）：{fixes:?}",
        fixes.len()
    ));
    Ok(fixes.len() as u32)
}

/// 启动迁移（PL027.1b）：把旧模型（`workdays` + `sessions`）一次性迁到 events 单表——
/// ① `VACUUM INTO` 一致快照备份成 `data/legacy-model-v1-<时间戳>.db`；② 校验"每条旧班行都有
/// 同刻 `clock_in` 事件"（不过即中止、旧表原样）；③ 删旧两表；④ 回填按天汇总 + 写对账日志
/// （旧 `sessions` 求和 vs 新口径汇总，差异属预期：旧起点为倒推）；⑤ 置 `model_version = 2`。
/// **幂等**（已置 2 直接返回 None）；失败不置版本、下次启动可重试；备份失败亦不迁。
/// # 返回
/// 迁移报告；None = 无需迁移（已迁过 / 老库无旧表）。
pub(crate) fn migrate_inner<C: Clock>(
    ctx: &AppContext<C>,
    now_secs: i64,
) -> Result<Option<MigrationReport>, CommandError> {
    let to_storage_io =
        |err: std::io::Error| CommandError::Storage(crate::storage::StorageError::Io(err));
    let db = crate::paths::default_db_path().map_err(to_storage_io)?;
    let dir = db
        .parent()
        .ok_or_else(|| CommandError::Storage(crate::storage::StorageError::MigrationPath))?;
    let stamp = Local
        .timestamp_opt(now_secs, 0)
        .single()
        .ok_or(CommandError::Clock)?
        .format("%Y%m%d-%H%M%S")
        .to_string();
    let backup = dir.join(format!("legacy-model-v1-{stamp}.db"));

    let report = poison("存储", ctx.storage.lock())?.migrate_events_only(&backup)?;
    if !report.migrated {
        return Ok(None);
    }
    // 回填汇总（已封口日），失败不阻断迁移落章（汇总可重算；数据已在流水）
    if let Err(err) = seal_days_inner(ctx, now_secs) {
        crate::warn_diag(&format!("迁移后按天汇总回填失败（可重算）：{err}"));
    }
    let totals = poison("存储", ctx.storage.lock())?.days_totals()?;
    let reconcile = match report.sessions_secs_before {
        Some(before) => format!(
            "旧 sessions 求和 {before}s vs 新口径汇总 {}s（差 {}s；旧口径起点为倒推，差异属预期）",
            totals.work_secs,
            totals.work_secs - before
        ),
        None => "无旧 sessions 表".to_string(),
    };
    crate::warn_diag(&format!(
        "PL027 迁移完成：备份 {}；旧班行 {} 行；{reconcile}",
        backup.display(),
        report.workdays_rows
    ));
    poison("存储", ctx.storage.lock())?.schema_set("model_version", "2")?;
    Ok(Some(report))
}

/// 心跳间隔（秒）：在岗期间每 60 秒刷新一次运行态缓存（PL027.3）。
pub(crate) const BEAT_INTERVAL_SECS: u64 = 60;

/// 心跳落盘（PL027.3）：把"当前态 + 最后一次活着时刻"写进 `runtime` 单行缓存。
/// 字段来源：**在岗态取自流水**（`open_duty_clock_in`：最后一条未配对打卡——与库同源，
/// 且**不取 `workday` 锁**，避免被"打卡/下班"这类持有该锁的调用方重入死锁）、
/// 会话态（工作·休息 / 段起点 / 累计）、自动下班基准；`last_beat_at = at`
/// （崩溃恢复判定"停机多久"的唯一依据）。**未上班不写**（无在岗段可恢复）；
/// **失败只落档不阻断业务**（容错白名单：缓存可由流水重建）。
pub(crate) fn beat_inner<C: Clock>(ctx: &AppContext<C>, at: i64) -> Result<(), CommandError> {
    let duty_clock_in_at = poison("存储", ctx.storage.lock())?.open_duty_clock_in()?;
    let Some(duty_clock_in_at) = duty_clock_in_at else {
        return Ok(()); // 未上班：无在岗段可恢复，不写
    };
    let (mode, segment_start_at, accumulated_secs) = {
        let session = lock(ctx)?;
        let running = matches!(
            session.state(),
            crate::session::SessionState::Running { .. }
        );
        (
            if running {
                RuntimeMode::Work
            } else {
                RuntimeMode::Rest
            },
            // 段起点（墙钟）= 现在 − 本段实时；休息态无在岗段起点
            running.then(|| at - session.segment_secs().as_secs() as i64),
            session.total().as_secs() as i64,
        )
    };
    let auto_out_at = poison("自动下班", ctx.auto_out.lock())?.due_at;
    poison("存储", ctx.storage.lock())?.runtime_upsert(RuntimeState {
        on_duty: true,
        mode,
        segment_start_at,
        duty_clock_in_at: Some(duty_clock_in_at),
        auto_out_at,
        accumulated_secs,
        last_beat_at: at,
    })?;
    Ok(())
}

/// 心跳线程（PL027.3）：每 60 秒刷新一次运行态缓存（线程常驻，未上班时空转）。
/// 无调用方兜底——出错只落档、绝不 panic（与自动下班兜底线程同款纪律）。
pub(crate) fn spawn_beat_thread(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_secs(BEAT_INTERVAL_SECS));
        let ctx = app.state::<AppContext>();
        let at = match now_secs(&ctx) {
            Ok(at) => at,
            Err(err) => {
                crate::warn_diag(&format!("心跳时钟异常，跳过本轮：{err}"));
                continue;
            }
        };
        if let Err(err) = beat_inner(&ctx, at) {
            crate::warn_diag(&format!("心跳写入失败（忽略）：{err}"));
        }
    });
}

/// 状态变化后顺手刷新心跳（PL027.3）：失败只落档、不影响动作本身。
pub(crate) fn beat_after_action<C: Clock>(ctx: &AppContext<C>) {
    let at = match now_secs(ctx) {
        Ok(at) => at,
        Err(err) => {
            crate::warn_diag(&format!("心跳时钟异常（跳过）：{err}"));
            return;
        }
    };
    if let Err(err) = beat_inner(ctx, at) {
        crate::warn_diag(&format!("心跳写入失败（忽略）：{err}"));
    }
}

/// 启动迁移的失败处置：**不阻断启动**（新代码只读流水，旧表留着无害），仅落档、下次重试。
pub(crate) fn migrate_on_start<C: Clock>(ctx: &AppContext<C>, now_secs: i64) {
    match migrate_inner(ctx, now_secs) {
        Ok(Some(report)) => {
            crate::warn_diag(&format!("启动迁移：已迁旧班行 {} 行", report.workdays_rows));
        }
        Ok(None) => {}
        Err(err) => crate::warn_diag(&format!("启动迁移未完成（旧表保留，下次启动重试）：{err}")),
    }
}

/// 启动恢复结果（serde 单一来源；PL027.8 前端提示条消费）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RecoveryOutcome {
    /// 被补下班的记账时刻（None = 正常启动 / 无缝续）。
    pub closed_at: Option<i64>,
    /// 该次补下班是否以"自动下班"名义（越过自动下班点）。
    pub auto: bool,
    /// 无缝续时的读数（= 断开前累计 + 停机时长；None = 未续）。
    pub resumed_secs: Option<i64>,
}

/// 启动对账（PL027.4；**崩溃 / 正常退出皆走**——判据只看停机时长 `now − last_beat_at`，
/// 与进程是怎么没的无关）：读运行态缓存 → 三分支执行——
/// `Nothing` 原样（无未闭合打卡 / 无缓存，交给常规启动路径）；
/// `Continue` 恢复工作日态与会话（停机时长计入当前段）并刷心跳，**零流水写入**；
/// `CloseAt` 以 `at` 补段末事件（工作态）与下班事件（`crash_clock_out`；越过自动下班点则
/// `auto_clock_out`）+ 清缓存 + 内存回未上班，随后顺手滚一轮按天汇总。
/// **幂等**：二次执行读到新心跳即走 `Nothing`；失败由调用方决定降级（启动不因此中断）。
pub(crate) fn recover_on_startup<C: Clock>(
    ctx: &AppContext<C>,
    now: i64,
) -> Result<RecoveryOutcome, CommandError> {
    let cached = poison("存储", ctx.storage.lock())?.runtime_get()?;
    let last_beat_at = cached.as_ref().map(|row| row.last_beat_at);
    let input = cached.map(|row| RecoveryInput {
        last_beat_at: row.last_beat_at,
        duty_clock_in_at: row.duty_clock_in_at.unwrap_or(row.last_beat_at),
        working: row.mode == RuntimeMode::Work,
        segment_start_at: row.segment_start_at,
        accumulated_secs: row.accumulated_secs,
        auto_out_at: row.auto_out_at,
    });
    match decide_recovery(input.as_ref(), now) {
        RecoveryDecision::Nothing => Ok(RecoveryOutcome {
            closed_at: None,
            auto: false,
            resumed_secs: None,
        }),
        RecoveryDecision::Continue {
            duty_clock_in_at,
            working,
            segment_start_at,
            accumulated_secs,
        } => {
            *poison("工作日", ctx.workday.lock())? =
                WorkdayState::restore_on_duty(duty_clock_in_at);
            // 缓存里的 accumulated_secs = 心跳那一刻的**大计时器读数**（累计 + 本段实时）；
            // 恢复时须扣掉"本段在心跳前已走的部分"，否则与本段 elapsed 双计
            let total_at_beat = accumulated_secs.max(0);
            let beat = last_beat_at.unwrap_or(now);
            let elapsed = if working {
                (now - segment_start_at.unwrap_or(now)).max(0)
            } else {
                0
            };
            let before = if working {
                (total_at_beat - (beat - segment_start_at.unwrap_or(beat)).max(0)).max(0)
            } else {
                total_at_beat
            };
            let resumed = {
                let mut session = lock(ctx)?;
                if working {
                    session.restore_running_elapsed(
                        Duration::from_secs(elapsed as u64),
                        Duration::from_secs(before as u64),
                    );
                } else {
                    session.restore_paused(Duration::from_secs(before as u64));
                }
                session.total().as_secs() as i64
            };
            beat_after_action(ctx);
            crate::warn_diag(&format!(
                "启动恢复：无缝续（停机 {}s，读数 {}s）",
                now - last_beat_at.unwrap_or(now),
                resumed
            ));
            Ok(RecoveryOutcome {
                closed_at: None,
                auto: false,
                resumed_secs: Some(resumed),
            })
        }
        RecoveryDecision::CloseAt {
            at,
            auto,
            close_segment,
        } => {
            let kind = if auto {
                EventKind::AutoClockOut
            } else {
                EventKind::CrashClockOut
            };
            let mut events: Vec<(i64, EventKind)> = Vec::new();
            if close_segment {
                events.push((at, EventKind::SegmentEnd));
            }
            events.push((at, kind));
            poison("存储", ctx.storage.lock())?.insert_events(&events)?;
            poison("存储", ctx.storage.lock())?.runtime_clear()?;
            *poison("工作日", ctx.workday.lock())? = WorkdayState::Off;
            lock(ctx)?.reset();
            if let Err(err) = seal_days_inner(ctx, now) {
                crate::warn_diag(&format!("启动恢复后按天汇总失败（忽略）：{err}"));
            }
            crate::warn_diag(&format!(
                "启动恢复：补下班 {at}（{}），停机 {}s",
                if auto {
                    "自动下班名义"
                } else {
                    "崩溃补记"
                },
                now - last_beat_at.unwrap_or(now)
            ));
            Ok(RecoveryOutcome {
                closed_at: Some(at),
                auto,
                resumed_secs: None,
            })
        }
    }
}

/// 自动下班检查（挂 session_status 评估口）：**基准到点**（PL025.8；PATCH005.3 口径 =
/// 本班打卡 + **今日剩余**，加班后按 +4h 步进重排）→ 以基准时刻执行下班，返回该时刻供调用方发
/// workday-auto-out 事件；
/// 未武装（无在岗班）或未到点 → None。口径：执行时刻恒 = 基准本身（与发现时刻无关，
/// "发现可迟到、账目准时"），**与段状态无关**（在休息态照点执行）。
pub(super) fn try_auto_clock_out<C: Clock>(
    ctx: &AppContext<C>,
) -> Result<Option<i64>, CommandError> {
    let due_at = poison("自动下班", ctx.auto_out.lock())?.due_at;
    let Some(due) = due_at else {
        return Ok(None);
    };
    if now_secs(ctx)? < due {
        return Ok(None);
    }
    // 若恰被并发手动下班，此处严格报错、下一 tick 基准已撤自然消停
    clock_out_inner(ctx, due, EventKind::AutoClockOut).map(|_| Some(due))
}

/// 今日预算上下文（PATCH005.3 纯逻辑 + 存储读）：基准 / 加班步数 / 已累计两口径。
/// 口径：数据日 = 打卡日（跨零点不切分）；按**在岗**累计（班内休息计入、班间空档不计）；
/// 加班步数 = 该数据日 `overtime` 事件条数（只追加流水，账可重推）。
/// 已累计两口径：`used_before_shift_secs` 把**末窗视为当前班**而排除（在岗时调用）；
/// `used_total_secs` 为全部窗口之和——**打卡前**（尚无开窗）必须用后者，它才是"本班之前"。
struct DayBudgetContext {
    /// 今日基准小时（当天第一次打卡时的自动下班小时数；整日锁定 ⇒ 直取设置值，无需快照事件）。
    basis_hours: u32,
    /// 加班步数（本数据日 `overtime` 事件条数）。
    overtime_steps: u32,
    /// 本班（末窗）之前的今日已累计在岗秒。
    used_before_shift_secs: i64,
    /// 今日已累计在岗秒（全部窗口之和；打卡前 = 全部已关窗口）。
    used_total_secs: i64,
}

/// 今日预算上下文取数（见 [`DayBudgetContext`]）。
fn day_budget_context<C: Clock>(
    ctx: &AppContext<C>,
    clock_in_at: i64,
    now: i64,
) -> Result<DayBudgetContext, CommandError> {
    let basis = poison("设置", ctx.settings.lock())?.workday_auto_out_hours;
    let day_start = local_day_start(clock_in_at)?;
    let day_end = day_start + 86_400;
    let events = poison("存储", ctx.storage.lock())?.events_between(
        day_start,
        day_end + i64::from(OVERTIME_CAP_HOURS) * 3_600 + 1,
    )?;
    let steps = events
        .iter()
        .filter(|(_, kind)| *kind == EventKind::Overtime)
        .count() as u32;
    let (windows, _) = duty_windows_for_day(&events, day_start, day_end, now);
    // 末窗 = 本班（在岗窗）；"本班之前的已累计"须把它排除
    let used_before = crate::workday::day_used_secs(&windows[..windows.len().saturating_sub(1)]);
    Ok(DayBudgetContext {
        basis_hours: basis,
        overtime_steps: steps,
        used_before_shift_secs: used_before,
        used_total_secs: crate::workday::day_used_secs(&windows),
    })
}

/// 写入自动下班基准（PL025.8）：代次 +1 使旧兜底线程让位、待决提示复位；返回新代次供命令层 spawn。
pub(super) fn set_auto_out_watch<C: Clock>(
    ctx: &AppContext<C>,
    due_at: i64,
) -> Result<u64, CommandError> {
    let mut watch = poison("自动下班", ctx.auto_out.lock())?;
    watch.due_at = Some(due_at);
    watch.generation = watch.generation.wrapping_add(1);
    watch.pending_notified = false;
    Ok(watch.generation)
}

/// 撤销自动下班基准（下班/退出）：代次 +1 让兜底线程退出。
pub(super) fn clear_auto_out_watch<C: Clock>(ctx: &AppContext<C>) -> Result<(), CommandError> {
    let mut watch = poison("自动下班", ctx.auto_out.lock())?;
    watch.due_at = None;
    watch.generation = watch.generation.wrapping_add(1);
    Ok(())
}

/// 读当前基准与代次（兜底线程用）：锁中毒按"无基准"处理并落档——线程无调用方可上抛。
fn read_auto_out_watch(app: &AppHandle) -> Option<(Option<i64>, u64)> {
    let ctx = app.state::<AppContext>();
    let read = match ctx.auto_out.lock() {
        Ok(watch) => Some((watch.due_at, watch.generation)),
        Err(_) => {
            crate::warn_diag("自动下班基准锁中毒，兜底线程退出");
            None
        }
    };
    read
}

/// 自动下班待决判定（PL025.8）：到点前 60s 内且本轮尚未提示过 → 置提示标志并返回到点时刻
/// （同一基准只提示一次；基准变更时标志复位）。无基准（未武装）不提示。
pub(super) fn auto_out_pending_inner<C: Clock>(
    ctx: &AppContext<C>,
    now: i64,
) -> Result<Option<i64>, CommandError> {
    let mut watch = poison("自动下班", ctx.auto_out.lock())?;
    if watch.pending_notified {
        return Ok(None);
    }
    let Some(due) = auto_out_pending(watch.due_at, now) else {
        return Ok(None);
    };
    watch.pending_notified = true;
    Ok(Some(due))
}

/// 加班（PATCH005.3 步进版）：本班上限 **+4h**（硬顶 12h）+ 写 `overtime` 事件（留痕）；
/// **到顶**（上限已 12h）→ `AlreadyAtOvertimeCap`；**跨日**（now 的日历日 ≠ 打卡日）→
/// `OvertimeAcrossMidnight`（跨日只给下班）。**不动段事件**——用户可能正在工作段或休息段，
/// 计时与图谱不受影响；返回新代次供命令层重排兜底线程。
pub(super) fn overtime_inner<C: Clock>(ctx: &AppContext<C>, now: i64) -> Result<u64, CommandError> {
    let clock_in_at = {
        let workday = poison("工作日", ctx.workday.lock())?;
        match *workday {
            WorkdayState::OnDuty { clock_in_at } => clock_in_at,
            WorkdayState::Off => return Err(CommandError::Workday(WorkdayError::NotOnDuty)),
        }
    };
    let bc = day_budget_context(ctx, clock_in_at, now)?;
    let budget = crate::workday::day_budget_hours(bc.basis_hours, bc.overtime_steps);
    let decision = crate::workday::overtime_decision(
        local_day_start(now)?,
        local_day_start(clock_in_at)?,
        budget,
    );
    match decision {
        crate::workday::OvertimeDecision::Allow => {}
        crate::workday::OvertimeDecision::AtCap => {
            return Err(CommandError::AlreadyAtOvertimeCap(OVERTIME_CAP_HOURS))
        }
        crate::workday::OvertimeDecision::AcrossMidnight => {
            return Err(CommandError::OvertimeAcrossMidnight)
        }
    }
    poison("存储", ctx.storage.lock())?.insert_event(now, EventKind::Overtime)?;
    let next_budget = crate::workday::day_budget_hours(bc.basis_hours, bc.overtime_steps + 1);
    let remaining = crate::workday::shift_remaining_secs(next_budget, bc.used_before_shift_secs);
    set_auto_out_watch(ctx, crate::workday::shift_deadline(clock_in_at, remaining))
}

/// 启动武装（PATCH005.3）：在岗恢复时按"本班打卡 + **今日剩余**"重建基准并返回代次（未在岗 None）。
/// 重启后基准随 AppContext 清零，不重建则兜底线程无基准可依——与打卡路径同源（arm → spawn）。
pub(crate) fn arm_auto_out_on_start<C: Clock>(
    ctx: &AppContext<C>,
) -> Result<Option<u64>, CommandError> {
    let clock_in_at = {
        let workday = poison("工作日", ctx.workday.lock())?;
        match *workday {
            WorkdayState::OnDuty { clock_in_at } => clock_in_at,
            WorkdayState::Off => return Ok(None),
        }
    };
    let now = now_secs(ctx)?;
    let bc = day_budget_context(ctx, clock_in_at, now)?;
    let budget = crate::workday::day_budget_hours(bc.basis_hours, bc.overtime_steps);
    let remaining = crate::workday::shift_remaining_secs(budget, bc.used_before_shift_secs);
    set_auto_out_watch(ctx, crate::workday::shift_deadline(clock_in_at, remaining)).map(Some)
}

/// 兜底自动下班线程（PL025.8）：到点自动下班——前端被关/崩溃也必执行。
/// 循环读基准按 [`AutoOutStep`] 行动：未到点睡到点（远期按 60s 上限轮询，基准变更 1 分钟内被感知），
/// 到点以基准时刻执行；基准换代（加班/下班/重新打卡）即让位，由新线程接手。
pub(crate) fn spawn_auto_out_watcher(app: &AppHandle, generation: u64) {
    let app = app.clone();
    std::thread::spawn(move || loop {
        let Some((due_at, current)) = read_auto_out_watch(&app) else {
            return;
        };
        let now = match now_secs(&app.state::<AppContext>()) {
            Ok(now) => now,
            Err(err) => {
                crate::warn_diag(&format!("兜底自动下班时钟异常，线程退出：{err}"));
                return;
            }
        };
        match auto_out_step(due_at, generation, current, now) {
            AutoOutStep::Stop | AutoOutStep::Yield => return,
            AutoOutStep::Wait(secs) => std::thread::sleep(std::time::Duration::from_secs(secs)),
            AutoOutStep::Fire(at) => {
                let ctx = app.state::<AppContext>();
                match clock_out_inner(&ctx, at, EventKind::AutoClockOut) {
                    Ok(()) => {
                        if let Err(err) = app.emit("workday-auto-out", at) {
                            crate::warn_diag(&format!("兜底自动下班事件发送失败：{err}"));
                        }
                    }
                    // 并发手动下班后到点：状态已 Off，报错属预期
                    Err(err) => crate::warn_diag(&format!("兜底自动下班未执行：{err}")),
                }
                return;
            }
        }
    });
}
/// 计时门禁：未上班严格拒绝（前后端双保险的后端面；托盘/热键/UI 共用）。
pub(super) fn require_on_duty<C: Clock>(ctx: &AppContext<C>) -> Result<(), CommandError> {
    let workday = poison("工作日", ctx.workday.lock())?;
    if !workday.is_on_duty() {
        return Err(CommandError::Workday(WorkdayError::NotOnDuty));
    }
    Ok(())
}

/// 取一次性恢复提示（PL027.8）：返回并清空启动恢复结果——前端挂载时调用一次；
/// 仅"判下班"分支有值（无缝续分支静默，不打扰）。
#[tauri::command]
pub fn take_recovery_notice(
    handle: State<'_, AppContext>,
) -> Result<Option<RecoveryOutcome>, CommandError> {
    take_recovery_notice_inner(&handle)
}

/// 取一次性恢复提示的内核（可单测）：`take` 语义——取过即空。
pub(super) fn take_recovery_notice_inner<C: Clock>(
    ctx: &AppContext<C>,
) -> Result<Option<RecoveryOutcome>, CommandError> {
    Ok(poison("恢复提示", ctx.recovery.lock())?.take())
}

/// 上班打卡（前端确认框确认后调用）；顺路武装兜底自动下班线程（PL025.8：前端死也必下班）。
#[tauri::command]
pub fn clock_in(handle: State<'_, AppContext>, app: AppHandle) -> Result<(), CommandError> {
    clock_in_inner(&handle, now_secs(&handle)?)?;
    let generation = poison("自动下班", handle.auto_out.lock())?.generation;
    spawn_auto_out_watcher(&app, generation);
    Ok(())
}

/// 下班打卡（前端确认框确认后调用；按当前时刻记账）。
#[tauri::command]
pub fn clock_out(handle: State<'_, AppContext>) -> Result<(), CommandError> {
    clock_out_inner(&handle, now_secs(&handle)?, EventKind::ClockOut)?;
    Ok(())
}

/// 加班（PL025.9）：把本班下班基准拉满到 12h；顺路重排兜底线程（代次换代 → 旧线程让位）。
#[tauri::command]
pub fn overtime(handle: State<'_, AppContext>, app: AppHandle) -> Result<(), CommandError> {
    let generation = overtime_inner(&handle, now_secs(&handle)?)?;
    spawn_auto_out_watcher(&app, generation);
    Ok(())
}

/// 单日明细（offset 缺省 0 = 今日；前后日翻看传 ±N，越界严格报错）。
/// async：events 全表扫 + 归约移出主线程（FIX002.8）。
#[tauri::command]
pub async fn day_detail(
    handle: State<'_, AppContext>,
    offset: Option<i64>,
) -> Result<DaySummary, CommandError> {
    day_summary_inner(&handle, checked_offset(offset)?, now_secs(&handle)?)
}

/// 周视图（PL022）：查看日所在自然周（周一~周日），随 offset 联动（前端必传）。
/// async：同 day_detail 移出主线程。
#[tauri::command]
pub async fn week_detail(
    handle: State<'_, AppContext>,
    offset: i64,
) -> Result<Vec<WeekDay>, CommandError> {
    week_detail_inner(&handle, checked_offset(Some(offset))?, now_secs(&handle)?)
}

/// 总日均（PL022）：全历史工作时长总和 + 有数据天数。async：同 day_detail 移出主线程。
#[tauri::command]
pub async fn workday_total(handle: State<'_, AppContext>) -> Result<WorkdayTotal, CommandError> {
    workday_total_inner(&handle, now_secs(&handle)?)
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
            .events_between(i64::MIN / 2, i64::MAX)
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
            ctx.storage.lock().unwrap().open_duty_clock_in().unwrap(),
            Some(1_000)
        );
        assert!(event_kinds(&ctx).contains(&EventKind::ClockIn));
        assert_eq!(
            *ctx.workday.lock().unwrap(),
            WorkdayState::OnDuty { clock_in_at: 1_000 }
        );
        // 打卡即工作（PATCH002）：会话直接处于 Running，无需再 start
        assert_eq!(
            super::super::session::status_snapshot(&ctx)
                .unwrap()
                .0
                .state,
            "running",
            "打卡即工作"
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
            duty.storage.lock().unwrap().open_duty_clock_in().unwrap(),
            Some(1_000),
            "拒绝的上班不改在岗态"
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
        clock.advance(secs(60));
        clock_out_inner(&ctx, 1_060, EventKind::ClockOut).unwrap();

        {
            let storage = ctx.storage.lock().unwrap();
            assert_eq!(
                storage.all_total().unwrap(),
                60,
                "段末与下班同刻 → 段时长 60s（由流水配对推出）"
            );
            assert_eq!(storage.open_duty_clock_in().unwrap(), None, "下班后离岗");
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

    /// PL027.7：惰性按天汇总——过去日（已封口）滚进 `days`；今天不滚；幂等；与逐日现算全等。
    #[test]
    fn seal_days_rolls_past_days_equals_live_pipeline() {
        let ctx = ctx(FakeClock::new());
        let now = now_secs(&ctx).unwrap();
        let today0 = local_day_start(now).unwrap();
        let yesterday0 = today0 - 86_400;
        {
            let storage = ctx.storage.lock().unwrap();
            storage
                .insert_events(&[
                    (yesterday0 + 9 * 3_600, EventKind::ClockIn),
                    (yesterday0 + 9 * 3_600, EventKind::SegmentStart),
                    (yesterday0 + 10 * 3_600, EventKind::SegmentEnd),
                    (yesterday0 + 10 * 3_600, EventKind::ClockOut),
                ])
                .unwrap();
            storage
                .insert_events(&[
                    (today0 + 9 * 3_600, EventKind::ClockIn),
                    (today0 + 9 * 3_600, EventKind::SegmentStart),
                    (today0 + 10 * 3_600, EventKind::SegmentEnd),
                    (today0 + 10 * 3_600, EventKind::ClockOut),
                ])
                .unwrap();
        }
        assert_eq!(seal_days_inner(&ctx, now).unwrap(), 1, "只滚昨天");
        assert_eq!(seal_days_inner(&ctx, now).unwrap(), 0, "幂等：再滚 0 天");

        let date = |day0: i64| {
            Local
                .timestamp_opt(day0, 0)
                .single()
                .unwrap()
                .format("%Y-%m-%d")
                .to_string()
        };
        let rollup = ctx
            .storage
            .lock()
            .unwrap()
            .day_get(&date(yesterday0))
            .unwrap()
            .unwrap();
        let live = day_summary_inner(&ctx, -1, now).unwrap();
        assert_eq!(rollup.duty_secs, live.duty_secs);
        assert_eq!(rollup.work_secs, live.work_secs);
        assert_eq!(rollup.rest_secs, live.rest_secs);
        assert_eq!(rollup.work_secs, 3_600, "昨天 1h 工作");
        assert!(
            ctx.storage
                .lock()
                .unwrap()
                .day_get(&date(today0))
                .unwrap()
                .is_none(),
            "今天不滚（走流水）"
        );
    }

    /// PL027.7：周卡与总日均**读汇总**——改汇总值即反映（汇总只是缓存，允许被重算覆盖）。
    #[test]
    fn week_and_total_read_rollup() {
        let ctx = ctx(FakeClock::new());
        let now = now_secs(&ctx).unwrap();
        let today0 = local_day_start(now).unwrap();
        let yesterday0 = today0 - 86_400;
        ctx.storage
            .lock()
            .unwrap()
            .insert_events(&[
                (yesterday0 + 9 * 3_600, EventKind::ClockIn),
                (yesterday0 + 9 * 3_600, EventKind::SegmentStart),
                (yesterday0 + 10 * 3_600, EventKind::SegmentEnd),
                (yesterday0 + 10 * 3_600, EventKind::ClockOut),
            ])
            .unwrap();
        assert_eq!(seal_days_inner(&ctx, now).unwrap(), 1);
        let date = Local
            .timestamp_opt(yesterday0, 0)
            .single()
            .unwrap()
            .format("%Y-%m-%d")
            .to_string();
        let mut row = ctx.storage.lock().unwrap().day_get(&date).unwrap().unwrap();
        row.work_secs = 7_200;
        row.duty_secs = 8_100;
        ctx.storage.lock().unwrap().day_upsert(&row).unwrap();

        let week = week_detail_inner(&ctx, 0, now).unwrap();
        assert!(week.iter().any(|d| d.work_secs == 7_200), "周卡读到汇总值");
        let total = workday_total_inner(&ctx, now).unwrap();
        assert_eq!(total.work_secs, 7_200, "总额取汇总值（今天无记录）");
        assert_eq!(total.days, 1, "有数据天数按汇总行计");
    }

    /// PL027.7：末窗未关的过去日不滚（跨零点在岗班 → 该打卡日未封口）。
    #[test]
    fn seal_skips_unsealed_day() {
        let ctx = ctx(FakeClock::new());
        let now = now_secs(&ctx).unwrap();
        let today0 = local_day_start(now).unwrap();
        let yesterday0 = today0 - 86_400;
        ctx.storage
            .lock()
            .unwrap()
            .insert_events(&[
                (yesterday0 + 9 * 3_600, EventKind::ClockIn),
                (yesterday0 + 9 * 3_600, EventKind::SegmentStart),
            ])
            .unwrap();
        assert_eq!(seal_days_inner(&ctx, now).unwrap(), 0, "未下班 → 不滚");
        let date = Local
            .timestamp_opt(yesterday0, 0)
            .single()
            .unwrap()
            .format("%Y-%m-%d")
            .to_string();
        assert!(ctx
            .storage
            .lock()
            .unwrap()
            .day_get(&date)
            .unwrap()
            .is_none());
    }

    /// PL027.3：心跳写入运行态缓存——字段取自三处状态；未上班不写；下班清缓存；
    /// 每个状态动作（打卡/休息/下班）都顺手刷一次。
    #[test]
    fn beat_writes_runtime_state_and_skips_off_duty() {
        let clock = FakeClock::new();
        let ctx = ctx(clock.clone());
        // 未上班：不写
        beat_inner(&ctx, 1_000).unwrap();
        assert_eq!(ctx.storage.lock().unwrap().runtime_get().unwrap(), None);

        // 打卡即工作：打卡动作本身已刷一次心跳
        clock_in_inner(&ctx, 1_000).unwrap();
        let first = ctx.storage.lock().unwrap().runtime_get().unwrap().unwrap();
        assert!(first.on_duty);
        assert_eq!(first.mode, RuntimeMode::Work);
        assert_eq!(first.segment_start_at, Some(1_000));
        assert_eq!(first.duty_clock_in_at, Some(1_000));
        assert_eq!(first.auto_out_at, Some(1_000 + 8 * 3_600));
        assert_eq!(first.last_beat_at, 1_000);

        // 走 120s 后再刷：段起点不变、读数 = 120、心跳时刻前进
        clock.advance(secs(120));
        let at = now_secs(&ctx).unwrap();
        beat_inner(&ctx, at).unwrap();
        let row = ctx.storage.lock().unwrap().runtime_get().unwrap().unwrap();
        assert_eq!(row.segment_start_at, Some(1_000));
        assert_eq!(row.accumulated_secs, 120);
        assert_eq!(row.last_beat_at, at);

        // 休息（按休息动作本身刷心跳）：模式 = Rest、段起点清空、累计停在 120
        super::super::session::set_run_mode_inner(&ctx, super::super::session::RunMode::Rest)
            .unwrap();
        let rest = ctx.storage.lock().unwrap().runtime_get().unwrap().unwrap();
        assert_eq!(rest.mode, RuntimeMode::Rest);
        assert_eq!(rest.segment_start_at, None);
        assert_eq!(rest.accumulated_secs, 120);

        // 下班：清缓存（未上班无在岗段可恢复）
        clock_out_inner(&ctx, at, EventKind::ClockOut).unwrap();
        assert_eq!(ctx.storage.lock().unwrap().runtime_get().unwrap(), None);
    }

    /// PL027.4：启动恢复三分支——无缝续（≤10 分钟：零流水写入、读数含停机）/ 按最后心跳
    /// 补下班（>10 分钟）/ 无事（无缓存）。
    #[test]
    fn recovery_continues_within_grace_and_closes_beyond() {
        // ① 无缝续：打卡 → 走 100s → 心跳 → 停机 300s → 恢复
        let clock = FakeClock::new();
        let make = |c: FakeClock| ctx(c);
        let ctx = make(clock.clone());
        clock_in_inner(&ctx, 1_000).unwrap();
        clock.advance(secs(100));
        let beat_at = now_secs(&ctx).unwrap();
        beat_inner(&ctx, beat_at).unwrap();
        let before = ctx
            .storage
            .lock()
            .unwrap()
            .events_between(i64::MIN / 2, i64::MAX)
            .unwrap()
            .len();

        clock.advance(secs(300));
        let now = now_secs(&ctx).unwrap();
        let outcome = recover_on_startup(&ctx, now).unwrap();
        assert_eq!(outcome.closed_at, None, "≤10 分钟不补下班");
        assert_eq!(
            outcome.resumed_secs,
            Some(400),
            "读数 = 段起 1_000 → now（停机计入）"
        );
        assert!(ctx.workday.lock().unwrap().is_on_duty(), "恢复在岗");
        let status = super::super::session::status_snapshot(&ctx).unwrap().0;
        assert_eq!(status.state, "running", "恢复运行态（无缝续）");
        assert_eq!(status.total_ms, 400_000, "大计时器含停机时长");
        assert_eq!(
            ctx.storage
                .lock()
                .unwrap()
                .events_between(i64::MIN / 2, i64::MAX)
                .unwrap()
                .len(),
            before,
            "无缝续零流水写入"
        );
        let row = ctx.storage.lock().unwrap().runtime_get().unwrap().unwrap();
        assert_eq!(row.last_beat_at, now, "恢复后心跳已刷新");
        assert_eq!(row.segment_start_at, Some(1_000), "段起点原样");

        // ② 超宽限：新班 → 心跳 → 停机 1 小时 → 恢复 → 按最后心跳补下班
        let clock2 = FakeClock::new();
        let ctx2 = make(clock2.clone());
        clock_in_inner(&ctx2, 2_000).unwrap();
        clock2.advance(secs(100));
        let beat2 = now_secs(&ctx2).unwrap();
        beat_inner(&ctx2, beat2).unwrap();
        clock2.advance(secs(3_600));
        let out2 = recover_on_startup(&ctx2, now_secs(&ctx2).unwrap()).unwrap();
        assert_eq!(out2.closed_at, Some(beat2), "下班时刻 = 最后心跳");
        assert!(!out2.auto, "未越自动下班点 → 崩溃补记");
        assert_eq!(
            *ctx2.workday.lock().unwrap(),
            WorkdayState::Off,
            "内存回未上班"
        );
        assert_eq!(
            ctx2.storage.lock().unwrap().runtime_get().unwrap(),
            None,
            "缓存已清"
        );
        let rows2 = ctx2
            .storage
            .lock()
            .unwrap()
            .events_between(i64::MIN / 2, i64::MAX)
            .unwrap();
        assert!(
            rows2.contains(&(beat2, EventKind::SegmentEnd)),
            "工作态补段末：{rows2:?}"
        );
        assert!(rows2.contains(&(beat2, EventKind::CrashClockOut)));
        assert_eq!(
            super::super::session::status_snapshot(&ctx2)
                .unwrap()
                .0
                .state,
            "idle"
        );

        // ③ 无事：无缓存
        let ctx3 = make(FakeClock::new());
        let out3 = recover_on_startup(&ctx3, 5_000).unwrap();
        assert_eq!(out3.closed_at, None);
        assert_eq!(out3.resumed_secs, None);
    }

    /// PL027.4：纯判定边界——恰好宽限内 = 无缝续、宽限外 = 补下班、越过自动下班点取该点。
    #[test]
    fn decide_recovery_boundaries() {
        let base = RecoveryInput {
            last_beat_at: 1_000,
            duty_clock_in_at: 1_000,
            working: true,
            segment_start_at: Some(1_000),
            accumulated_secs: 0,
            auto_out_at: Some(30_000),
        };
        assert_eq!(decide_recovery(None, 5_000), RecoveryDecision::Nothing);
        let grace = crate::workday::CRASH_GRACE_SECS;
        assert!(
            matches!(
                decide_recovery(Some(&base), 1_000 + grace),
                RecoveryDecision::Continue { .. }
            ),
            "恰好宽限内 = 无缝续"
        );
        assert!(matches!(
            decide_recovery(Some(&base), 1_000 + grace + 1),
            RecoveryDecision::CloseAt {
                at: 1_000,
                auto: false,
                close_segment: true
            }
        ));
        let overdue = RecoveryInput {
            last_beat_at: 2_000,
            auto_out_at: Some(1_500),
            working: false,
            segment_start_at: None,
            ..base
        };
        assert_eq!(
            decide_recovery(Some(&overdue), 9_999),
            RecoveryDecision::CloseAt {
                at: 1_500,
                auto: true,
                close_segment: false
            },
            "越过自动下班点：收口取该点 + 自动下班名义"
        );
    }

    /// PL027.9：脏数据归一化——今日形态（嵌套打卡）归一后窗口不再重叠、班账正常、幂等。
    #[test]
    fn normalize_dirty_data_fixes_overlap_windows() {
        let clock = FakeClock::new();
        let ctx = ctx(clock);
        let now = now_secs(&ctx).unwrap();
        let today0 = local_day_start(now).unwrap();
        ctx.storage
            .lock()
            .unwrap()
            .insert_events(&[
                (today0 + 9 * 3_600, EventKind::ClockIn),
                (today0 + 9 * 3_600, EventKind::SegmentStart),
                (today0 + 10 * 3_600, EventKind::ClockIn), // 嵌套打卡（脏）
                (today0 + 10 * 3_600, EventKind::SegmentStart),
                (today0 + 11 * 3_600, EventKind::SegmentEnd),
                (today0 + 11 * 3_600, EventKind::ClockOut),
            ])
            .unwrap();

        assert_eq!(
            normalize_dirty_data_inner(&ctx).unwrap(),
            2,
            "补段末 + 补下班"
        );
        assert_eq!(normalize_dirty_data_inner(&ctx).unwrap(), 0, "幂等");

        let summary = day_summary_inner(&ctx, 0, now).unwrap();
        assert_eq!(
            summary.duty_secs,
            2 * 3_600 - 1,
            "两班各约 1 小时、不重叠（前班在新打卡前一秒收口）"
        );
        let events = ctx
            .storage
            .lock()
            .unwrap()
            .events_between(i64::MIN / 2, i64::MAX)
            .unwrap();
        assert!(events.contains(&(today0 + 10 * 3_600 - 1, EventKind::SegmentEnd)));
        assert!(events.contains(&(today0 + 10 * 3_600 - 1, EventKind::ClockOut)));
    }

    /// PL027.8：一次性恢复提示——取一次即空（"判下班"分支留痕、无缝续静默）。
    #[test]
    fn recovery_notice_is_one_shot() {
        let ctx = ctx(FakeClock::new());
        assert_eq!(take_recovery_notice_inner(&ctx).unwrap(), None);
        *ctx.recovery.lock().unwrap() = Some(RecoveryOutcome {
            closed_at: Some(12_345),
            auto: false,
            resumed_secs: None,
        });
        assert_eq!(
            take_recovery_notice_inner(&ctx).unwrap(),
            Some(RecoveryOutcome {
                closed_at: Some(12_345),
                auto: false,
                resumed_secs: None
            })
        );
        assert_eq!(take_recovery_notice_inner(&ctx).unwrap(), None, "取过即空");
    }

    /// FIX002.4：自动下班路径段末事件随下班记回填时刻（与纯归约契约测试口径一致）。
    #[test]
    fn auto_pause_marks_segment_end_at_backfill() {
        let ctx = ctx(FakeClock::new());
        let in_at = now_secs(&ctx).unwrap() - 9 * 3_600;
        clock_in_inner(&ctx, in_at).unwrap();
        let out_at = in_at + 8 * 3_600;
        assert!(try_auto_clock_out(&ctx).unwrap().is_some());
        let rows = ctx
            .storage
            .lock()
            .unwrap()
            .events_between(i64::MIN / 2, i64::MAX)
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

    /// W4：day_detail 装配——种子归约为三值与区块（在岗 8h/工作 3h/休息 5h）。
    /// PL025.6：在岗窗口来自 workdays 行（数据日 = 打卡日），事件只补段末。
    #[test]
    fn day_detail_reduces_seeded_events() {
        let ctx = ctx(FakeClock::new());
        let now = Local::now().timestamp();
        let day_start = period::day_start_secs(&Local::now());
        {
            let storage = ctx.storage.lock().unwrap();
            let id = storage
                .workday_open_with_events(
                    day_start + 9 * 3_600,
                    &[EventKind::ClockIn, EventKind::SegmentStart],
                )
                .unwrap();
            storage
                .insert_event(day_start + 12 * 3_600, EventKind::SegmentEnd)
                .unwrap();
            storage
                .workday_close_with_event(id, day_start + 17 * 3_600, EventKind::ClockOut)
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

    /// day_detail 前后日翻看：昨日班只在 offset = -1 可见，今日为空日。
    #[test]
    fn day_detail_offset_browses_days() {
        let ctx = ctx(FakeClock::new());
        let now = Local::now().timestamp();
        let day_start = period::day_start_secs(&Local::now());
        let yesterday_in = day_start - 86_400 + 9 * 3_600;
        {
            let storage = ctx.storage.lock().unwrap();
            let id = storage
                .workday_open_with_events(yesterday_in, &[EventKind::ClockIn])
                .unwrap();
            storage
                .workday_close_with_event(id, yesterday_in + 3_600, EventKind::ClockOut)
                .unwrap();
        }
        assert_eq!(day_summary_inner(&ctx, 0, now).unwrap().duty_secs, 0);
        assert_eq!(day_summary_inner(&ctx, -1, now).unwrap().duty_secs, 3_600);
    }

    /// PL025.6：跨零点班归打卡日——23:00 打卡、次日 07:00 下班，次日视图全零。
    #[test]
    fn cross_midnight_shift_belongs_to_clock_in_day() {
        let ctx = ctx(FakeClock::new());
        let now = Local::now().timestamp();
        let day_start = period::day_start_secs(&Local::now());
        let in_at = day_start - 86_400 + 23 * 3_600;
        let out_at = in_at + 8 * 3_600;
        {
            let storage = ctx.storage.lock().unwrap();
            let id = storage
                .workday_open_with_events(in_at, &[EventKind::ClockIn, EventKind::SegmentStart])
                .unwrap();
            storage.insert_event(out_at, EventKind::SegmentEnd).unwrap();
            storage
                .workday_close_with_event(id, out_at, EventKind::ClockOut)
                .unwrap();
        }
        let day = day_summary_inner(&ctx, -1, now).unwrap();
        assert_eq!(day.duty_secs, 8 * 3_600, "整班归打卡日（不按自然日切分）");
        assert_eq!(day.work_secs, 8 * 3_600);
        assert_eq!(day.duty_ended_at, Some(out_at), "终点越过日界保留");
        let today = day_summary_inner(&ctx, 0, now).unwrap();
        assert_eq!(today.duty_secs, 0, "次日无打卡行 → 无窗口");
    }

    /// PL025.6：同日两班（打卡→下班→打卡）→ 两个在岗窗口各自成段，三值不重不漏。
    #[test]
    fn same_day_two_shifts_are_two_windows() {
        let ctx = ctx(FakeClock::new());
        let now = Local::now().timestamp();
        let day_start = period::day_start_secs(&Local::now());
        {
            let storage = ctx.storage.lock().unwrap();
            let first = storage
                .workday_open_with_events(
                    day_start + 9 * 3_600,
                    &[EventKind::ClockIn, EventKind::SegmentStart],
                )
                .unwrap();
            storage
                .insert_event(day_start + 12 * 3_600, EventKind::SegmentEnd)
                .unwrap();
            storage
                .workday_close_with_event(first, day_start + 12 * 3_600, EventKind::ClockOut)
                .unwrap();
            let second = storage
                .workday_open_with_events(
                    day_start + 14 * 3_600,
                    &[EventKind::ClockIn, EventKind::SegmentStart],
                )
                .unwrap();
            storage
                .insert_event(day_start + 18 * 3_600, EventKind::SegmentEnd)
                .unwrap();
            storage
                .workday_close_with_event(second, day_start + 18 * 3_600, EventKind::ClockOut)
                .unwrap();
        }
        let sum = day_summary_inner(&ctx, 0, now).unwrap();
        assert_eq!(sum.duty_secs, 7 * 3_600);
        assert_eq!(sum.work_secs, 7 * 3_600);
        assert_eq!(sum.rest_secs, 0);
        assert_eq!(sum.duty_started_at, Some(day_start + 9 * 3_600));
        assert_eq!(sum.duty_ended_at, Some(day_start + 18 * 3_600));
        assert_eq!(sum.blocks.len(), 2, "两班各一块（班间空档不是块）");
    }

    /// PL025.6：打卡即工作首段——clock_in 事务里那条 segment_start 生效，首块即 Work。
    #[test]
    fn clock_in_first_segment_is_work() {
        let ctx = ctx(FakeClock::new());
        let now = Local::now().timestamp();
        let day_start = period::day_start_secs(&Local::now());
        clock_in_inner(&ctx, day_start + 60).unwrap(); // 取当日极早时刻，窗口必闭于 now
        let sum = day_summary_inner(&ctx, 0, now).unwrap();
        assert_eq!(
            sum.blocks.first().map(|b| b.kind),
            Some(crate::workday::BlockKind::Work)
        );
        assert!(sum.work_secs > 0, "打卡那刻起就有工作段");
        assert_eq!(sum.duty_ended_at, None, "未下班 → 在岗中");
    }

    /// PATCH004.1 表盘底轨锚点回归：当天已有一段班、又在**打卡当秒**开新班——新班必须立刻
    /// 在归约里产出区块。否则前端"当前班起点 = 末个班间空档之后的第一块（无空档则当天首块）"
    /// 会退回当天首块，底轨蓝条短暂锚到上一班起点，下一秒才自纠。
    #[test]
    fn day_summary_keeps_block_for_shift_opened_this_second() {
        let ctx = ctx(FakeClock::new());
        let day_start = period::day_start_secs(&Local::now());
        // 固定时刻（12:00）——本用例只依赖"数据日"、**不依赖真实钟表**：原先取 Local::now()，
        // 在 00:00–00:15 之间运行时"第一班"（day_start+600/+900 = 00:10/00:15）会晚于 now ⇒
        // 第二班打卡在时间序上倒排到前面 → 被当成"前班未关又出现新打卡" → 末窗变成第一班、
        // duty_ended_at 非 None，断言失败（跨零点 15 分钟失败窗口；2026-10-14 00:0x 实测抓到）。
        let now = day_start + 12 * 3_600;
        // 第一班取"now 之前"（恒在 12:00 之前，绝不晚于 now ⇒ 新班恒为末窗）
        let first_in = day_start + 600;
        let first_out = day_start + 900;
        {
            let storage = ctx.storage.lock().unwrap();
            let first = storage
                .workday_open_with_events(first_in, &[EventKind::ClockIn, EventKind::SegmentStart])
                .unwrap();
            storage
                .insert_event(first_out, EventKind::SegmentEnd)
                .unwrap();
            storage
                .workday_close_with_event(first, first_out, EventKind::ClockOut)
                .unwrap();
            // 第二班开在"现在"这一秒：now == start（同秒窗口）
            storage
                .workday_open_with_events(now, &[EventKind::ClockIn, EventKind::SegmentStart])
                .unwrap();
        }
        let sum = day_summary_inner(&ctx, 0, now).unwrap();
        assert_eq!(sum.duty_ended_at, None, "新班未关 → 在岗");
        assert_eq!(sum.blocks.len(), 2, "两班各一块：新班不得因同秒零长被丢");
        assert_eq!(
            sum.blocks.last().map(|b| b.start),
            Some(now),
            "新班块起点 = 打卡刻（前端底轨锚点）"
        );
    }

    // —— PATCH005.3 集成：今日预算驱动自动下班武装 ——

    /// 同日第二班：武装 = 本班打卡 + **今日剩余**（首班 4h 已用、基准 8h → 只剩 4h）。
    #[test]
    fn clock_in_arms_second_shift_with_day_remaining() {
        let ctx = ctx(FakeClock::new());
        let day_start = period::day_start_secs(&Local::now());
        clock_in_inner(&ctx, day_start + 9 * 3_600).unwrap();
        clock_out_inner(&ctx, day_start + 13 * 3_600, EventKind::ClockOut).unwrap();
        // 第二班 14:00 打卡：上限 8h − 已用 4h = 剩余 4h → 到点 18:00（旧口径会算成 22:00）
        clock_in_inner(&ctx, day_start + 14 * 3_600).unwrap();
        assert_eq!(
            ctx.auto_out.lock().unwrap().due_at,
            Some(day_start + 18 * 3_600),
            "第二班只剩余额 4h"
        );
    }

    /// 预算用满后再打卡 = **隐式加班**：落一条 `overtime`、上限 8h → 12h、本班剩 4h。
    #[test]
    fn clock_in_after_budget_exhausted_is_implicit_overtime() {
        let ctx = ctx(FakeClock::new());
        let day_start = period::day_start_secs(&Local::now());
        clock_in_inner(&ctx, day_start + 9 * 3_600).unwrap();
        clock_out_inner(&ctx, day_start + 17 * 3_600, EventKind::ClockOut).unwrap();
        clock_in_inner(&ctx, day_start + 18 * 3_600).unwrap();
        assert_eq!(
            ctx.auto_out.lock().unwrap().due_at,
            Some(day_start + 22 * 3_600),
            "隐式加班：上限 12h、本班剩 4h"
        );
        let events = ctx
            .storage
            .lock()
            .unwrap()
            .events_between(0, 10_000_000_000)
            .unwrap();
        assert_eq!(
            events
                .iter()
                .filter(|(_, k)| *k == EventKind::Overtime)
                .count(),
            1,
            "隐式加班落且仅落一条 overtime"
        );
    }

    /// 点「加班」= +4h 步进（落 `overtime`、基准 +4h）；到顶后拒绝（PATCH005.3）。
    #[test]
    fn overtime_adds_four_hours_then_caps() {
        let ctx = ctx(FakeClock::new());
        let day_start = period::day_start_secs(&Local::now());
        clock_in_inner(&ctx, day_start + 9 * 3_600).unwrap();
        let now = day_start + 10 * 3_600;
        overtime_inner(&ctx, now).unwrap();
        assert_eq!(
            ctx.auto_out.lock().unwrap().due_at,
            Some(day_start + 21 * 3_600),
            "基准 8h + 4h = 12h → 到点 = 09:00 + 12h"
        );
        assert!(
            matches!(
                overtime_inner(&ctx, now),
                Err(CommandError::AlreadyAtOvertimeCap(12))
            ),
            "上限已 12h：再点加班拒绝"
        );
    }

    /// PATCH005.3：DTO 预算字段——用户例（基准 6h、第一班 4h → 第二班 `remaining_secs` = 2h）。
    #[test]
    fn day_summary_exposes_shift_remaining() {
        let ctx = ctx(FakeClock::new());
        ctx.settings.lock().unwrap().workday_auto_out_hours = 6;
        let day_start = period::day_start_secs(&Local::now());
        clock_in_inner(&ctx, day_start + 9 * 3_600).unwrap();
        clock_out_inner(&ctx, day_start + 13 * 3_600, EventKind::ClockOut).unwrap();
        let now = day_start + 14 * 3_600;
        clock_in_inner(&ctx, now).unwrap();
        let sum = day_summary_inner(&ctx, 0, now).unwrap();
        assert_eq!(sum.budget_hours, 6);
        assert_eq!(
            sum.remaining_secs,
            2 * 3_600,
            "第二班只剩 2h（上限 − 本班前已用）"
        );
        // used_secs 含本班开窗（PATCH004.1 保底 1 秒），故取容差
        assert!(
            sum.used_secs >= 4 * 3_600 && sum.used_secs <= 4 * 3_600 + 60,
            "今日已累计 ≈ 第一班 4h（含本班开窗 1s），实得 {}",
            sum.used_secs
        );
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
    /// PL025.6：在岗窗口来自 workdays 行（数据日 = 打卡日），事件补段起止与打卡留痕。
    fn seed_day<C: Clock>(ctx: &AppContext<C>, day_shift: i64, work: i64, duty: i64) {
        let base = period::day_start_secs(&Local::now()) + day_shift * 86_400 + 9 * 3_600;
        let storage = ctx.storage.lock().unwrap();
        let id = storage
            .workday_open_with_events(base, &[EventKind::ClockIn, EventKind::SegmentStart])
            .unwrap();
        storage
            .insert_event(base + work, EventKind::SegmentEnd)
            .unwrap();
        storage
            .workday_close_with_event(id, base + duty, EventKind::ClockOut)
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
            let id = storage
                .workday_open_with_events(
                    last_sunday + 9 * 3_600,
                    &[EventKind::ClockIn, EventKind::SegmentStart],
                )
                .unwrap();
            storage
                .insert_event(last_sunday + 10 * 3_600, EventKind::SegmentEnd)
                .unwrap();
            storage
                .workday_close_with_event(id, last_sunday + 11 * 3_600, EventKind::ClockOut)
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

    /// PL022.1 / PL024.6：全历史聚合——三值总和 + 有数据天数（空日不计；空库零）。
    #[test]
    fn workday_total_sums_all_history() {
        let empty_ctx = ctx(FakeClock::new());
        let total = workday_total_inner(&empty_ctx, Local::now().timestamp()).unwrap();
        assert_eq!(total.duty_secs, 0);
        assert_eq!(total.work_secs, 0);
        assert_eq!(total.rest_secs, 0);
        assert_eq!(total.days, 0, "空库零值不 panic");

        let ctx = ctx(FakeClock::new());
        // 在岗 3h / 工作 2h / 休息 1h
        seed_day(&ctx, -2, 2 * 3_600, 3 * 3_600);
        // 中间（-1 日）为空日：不计入天数
        seed_day(&ctx, 0, 3_600, 2 * 3_600); // 在岗 2h / 工作 1h / 休息 1h
        let total = workday_total_inner(&ctx, Local::now().timestamp()).unwrap();
        assert_eq!(total.duty_secs, 5 * 3_600);
        assert_eq!(total.work_secs, 3 * 3_600);
        assert_eq!(total.rest_secs, 2 * 3_600);
        assert_eq!(total.days, 2, "仅两个有数据日");
    }

    /// PL025.1：最早记录所在本地日相对今天的偏移（统计"往前翻"的下界；空库 None）。
    #[test]
    fn workday_total_first_offset_bounds_history() {
        let now = Local::now().timestamp();
        let empty_ctx = ctx(FakeClock::new());
        assert_eq!(
            workday_total_inner(&empty_ctx, now).unwrap().first_offset,
            None,
            "空库无下界（前端按 0 处理）"
        );

        let ctx = ctx(FakeClock::new());
        seed_day(&ctx, -2, 2 * 3_600, 3 * 3_600);
        seed_day(&ctx, 0, 3_600, 2 * 3_600);
        assert_eq!(
            workday_total_inner(&ctx, now).unwrap().first_offset,
            Some(-2),
            "最早记录在前 2 日 → 下界 -2（再往前翻应被拦住）"
        );
    }

    /// 自动下班：到点以回填时刻（上班 + N）关班，迟到发现账目准时；未到点/未上班不动。
    #[test]
    fn auto_clock_out_backfills_at_threshold() {
        let ctx = ctx(FakeClock::new());
        let in_at = now_secs(&ctx).unwrap() - 9 * 3_600;
        clock_in_inner(&ctx, in_at).unwrap();
        assert_eq!(try_auto_clock_out(&ctx).unwrap(), Some(in_at + 8 * 3_600));
        let backfill = in_at + 8 * 3_600;
        assert!(
            ctx.storage
                .lock()
                .unwrap()
                .events_between(backfill, backfill + 1)
                .unwrap()
                .iter()
                .any(|(_, kind)| *kind == EventKind::AutoClockOut),
            "记账时刻 = 上班 + 8h，与发现时刻无关"
        );
        assert_eq!(
            ctx.storage.lock().unwrap().open_duty_clock_in().unwrap(),
            None
        );
        assert_eq!(*ctx.workday.lock().unwrap(), WorkdayState::Off);

        // 新班 30 分钟前上班：未到点不触发（与上一班回填时刻错开，避免同秒歧义）
        clock_in_inner(&ctx, now_secs(&ctx).unwrap() - 1_800).unwrap();
        assert_eq!(try_auto_clock_out(&ctx).unwrap(), None);
        assert_eq!(
            ctx.storage.lock().unwrap().open_duty_clock_in().unwrap(),
            Some(now_secs(&ctx).unwrap() - 1_800),
            "新班在岗：未配对打卡 = 30 分钟前"
        );

        // 未上班：不动
        clock_out_inner(&ctx, now_secs(&ctx).unwrap(), EventKind::ClockOut).unwrap();
        assert_eq!(try_auto_clock_out(&ctx).unwrap(), None);
    }

    /// PL025.8：打卡武装基准（到点 = 上班 + N）；待决提示同一基准只发一次；下班撤基准。
    #[test]
    fn clock_in_arms_and_clock_out_clears_auto_out() {
        let ctx = ctx(FakeClock::new());
        clock_in_inner(&ctx, 1_000).unwrap();
        let due = 1_000 + 8 * 3_600;
        assert_eq!(
            ctx.auto_out.lock().unwrap().due_at,
            Some(due),
            "打卡武装到点 = 上班 + N（设置口径）"
        );
        assert_eq!(auto_out_pending_inner(&ctx, due - 61).unwrap(), None);
        assert_eq!(
            auto_out_pending_inner(&ctx, due - 60).unwrap(),
            Some(due),
            "恰 60s 进待决窗口"
        );
        assert_eq!(
            auto_out_pending_inner(&ctx, due - 20).unwrap(),
            None,
            "同一基准只提示一次"
        );
        clock_out_inner(&ctx, 2_000, EventKind::ClockOut).unwrap();
        assert_eq!(ctx.auto_out.lock().unwrap().due_at, None, "下班撤基准");
    }

    /// PL025.9 + PATCH005.3：加班 = **+4h 步进**（此例基准 8h → 上限 12h）+ 写 overtime 事件 + 换代；
    /// 不动段事件；未上班 / 到顶严格报错。
    #[test]
    fn overtime_sets_cap_and_errors() {
        let off = ctx(FakeClock::new());
        assert!(matches!(
            overtime_inner(&off, 1_000),
            Err(CommandError::Workday(WorkdayError::NotOnDuty))
        ));

        let ctx = ctx(FakeClock::new());
        let in_at = now_secs(&ctx).unwrap();
        clock_in_inner(&ctx, in_at).unwrap();
        let starts_before = event_kinds(&ctx)
            .iter()
            .filter(|k| **k == EventKind::SegmentStart)
            .count();
        let generation_before = ctx.auto_out.lock().unwrap().generation;
        let generation = overtime_inner(&ctx, in_at + 3_600).unwrap();
        assert!(generation > generation_before, "加班换代 → 旧兜底线程让位");
        assert_eq!(
            ctx.auto_out.lock().unwrap().due_at,
            Some(in_at + 12 * 3_600),
            "基准 8h + 4h = 12h → 到点 = 上班 + 12h"
        );
        let kinds = event_kinds(&ctx);
        assert!(kinds.contains(&EventKind::Overtime), "加班事件入馆（留痕）");
        assert_eq!(
            kinds
                .iter()
                .filter(|k| **k == EventKind::SegmentStart)
                .count(),
            starts_before,
            "加班不动段事件"
        );
        assert!(matches!(
            overtime_inner(&ctx, in_at + 12 * 3_600),
            Err(CommandError::AlreadyAtOvertimeCap(12))
        ));
    }

    /// PL025.8：重启武装——在岗恢复时按"本班打卡 + 今日剩余"重建基准；未在岗返回 None。
    #[test]
    fn arm_on_start_rebuilds_baseline() {
        let ctx = ctx(FakeClock::new());
        assert_eq!(arm_auto_out_on_start(&ctx).unwrap(), None, "未在岗不武装");
        clock_in_inner(&ctx, 1_000).unwrap();
        {
            // 模拟重启后 AppContext 清零
            let mut watch = ctx.auto_out.lock().unwrap();
            watch.due_at = None;
        }
        let generation = arm_auto_out_on_start(&ctx).unwrap().unwrap();
        assert_eq!(ctx.auto_out.lock().unwrap().due_at, Some(1_000 + 8 * 3_600));
        assert!(generation > 0, "重建即换代");
    }

    /// PATCH005.3：重启后**多班**仍按今日剩余重推（原用例只覆盖首班 1000 + 8h）。
    #[test]
    fn arm_on_start_rebuilds_shift_remaining_after_restart() {
        let ctx = ctx(FakeClock::new());
        let day_start = period::day_start_secs(&Local::now());
        clock_in_inner(&ctx, day_start + 9 * 3_600).unwrap();
        clock_out_inner(&ctx, day_start + 13 * 3_600, EventKind::ClockOut).unwrap();
        clock_in_inner(&ctx, day_start + 14 * 3_600).unwrap();
        // 模拟重启：AppContext 的基准随进程清零
        ctx.auto_out.lock().unwrap().due_at = None;
        arm_auto_out_on_start(&ctx).unwrap().unwrap();
        assert_eq!(
            ctx.auto_out.lock().unwrap().due_at,
            Some(day_start + 18 * 3_600),
            "重启后按今日剩余 4h 重推，而非本班打卡 + 8h"
        );
    }

    /// PATCH005.3：重启后**加班步数**同样由流水重推（上限 12h）。
    #[test]
    fn arm_on_start_rebuilds_after_overtime_restart() {
        let ctx = ctx(FakeClock::new());
        let day_start = period::day_start_secs(&Local::now());
        clock_in_inner(&ctx, day_start + 9 * 3_600).unwrap();
        overtime_inner(&ctx, day_start + 10 * 3_600).unwrap();
        ctx.auto_out.lock().unwrap().due_at = None;
        arm_auto_out_on_start(&ctx).unwrap().unwrap();
        assert_eq!(
            ctx.auto_out.lock().unwrap().due_at,
            Some(day_start + 21 * 3_600),
            "重启后加班仍按 +4h 步进（09:00 + 12h）"
        );
    }

    /// PATCH005.3：跨日拒加班（命令层分支）——同日内放行、过零点拒绝。
    #[test]
    fn overtime_rejected_across_midnight_but_allowed_same_day() {
        let ctx = ctx(FakeClock::new());
        let day_start = period::day_start_secs(&Local::now());
        let in_at = day_start - 86_400 + 22 * 3_600; // 昨日 22:00 打卡（夜班）
        clock_in_inner(&ctx, in_at).unwrap();
        // 昨日 23:00：同日 → 放行（基准 8h + 4h = 12h → 到点 = 打卡 + 12h）
        overtime_inner(&ctx, day_start - 3_600).unwrap();
        assert_eq!(
            ctx.auto_out.lock().unwrap().due_at,
            Some(in_at + 12 * 3_600),
            "同日加班放行并按新上限重排"
        );
        // 今日 02:00：已跨日 → 拒绝（跨日只给下班）
        assert!(matches!(
            overtime_inner(&ctx, day_start + 2 * 3_600),
            Err(CommandError::OvertimeAcrossMidnight)
        ));
    }

    /// PATCH005.3 到顶守卫：今日累计 12h 后再打卡 → 拒绝且**不落任何事件**（前端 pill 同步置灰）。
    #[test]
    fn clock_in_after_cap_is_rejected() {
        let ctx = ctx(FakeClock::new());
        ctx.settings.lock().unwrap().workday_auto_out_hours = 12;
        let day_start = period::day_start_secs(&Local::now());
        clock_in_inner(&ctx, day_start + 9 * 3_600).unwrap();
        clock_out_inner(&ctx, day_start + 21 * 3_600, EventKind::ClockOut).unwrap();
        let before = event_kinds(&ctx).len();
        assert!(matches!(
            clock_in_inner(&ctx, day_start + 22 * 3_600),
            Err(CommandError::DayBudgetExhausted(12))
        ));
        assert_eq!(event_kinds(&ctx).len(), before, "拒绝时不落任何事件");
    }

    /// PATCH005.4：当前数据日偏移 = 有未闭合的班 → 该班打卡日；无 → 日历今日（时钟与统计的锚点）。
    #[test]
    fn data_day_offset_follows_open_shift() {
        let duty_ctx = ctx(FakeClock::new());
        let day_start = period::day_start_secs(&Local::now());
        let at_9 = day_start + 9 * 3_600;
        assert_eq!(
            data_day_offset(&duty_ctx, at_9).unwrap(),
            0,
            "未上班 = 日历今日"
        );
        clock_in_inner(&duty_ctx, at_9).unwrap();
        assert_eq!(
            data_day_offset(&duty_ctx, at_9 + 1_800).unwrap(),
            0,
            "当日班 = 0"
        );
        clock_out_inner(&duty_ctx, at_9 + 3_600, EventKind::ClockOut).unwrap();
        assert_eq!(
            data_day_offset(&duty_ctx, at_9 + 4 * 3_600).unwrap(),
            0,
            "下班后回到日历今日"
        );

        // 跨零点夜班：昨日 22:00 打卡、未下班 → 锚定打卡日（昨天）
        let night_ctx = ctx(FakeClock::new());
        clock_in_inner(&night_ctx, day_start - 2 * 3_600).unwrap();
        assert_eq!(
            data_day_offset(&night_ctx, day_start + 2 * 3_600).unwrap(),
            -1,
            "夜班锚定打卡日"
        );
    }

    /// PATCH005.4：预算字段只填**当前数据日**——跨零点夜班时数据日（−1）有值、日历今日（0）仍为 0。
    #[test]
    fn day_summary_fills_budget_for_current_data_day() {
        let ctx = ctx(FakeClock::new());
        ctx.settings.lock().unwrap().workday_auto_out_hours = 6;
        let day_start = period::day_start_secs(&Local::now());
        let in_at = day_start - 2 * 3_600; // 昨日 22:00
        let now = day_start + 2 * 3_600; // 今日 02:00
        clock_in_inner(&ctx, in_at).unwrap();

        let night = day_summary_inner(&ctx, -1, now).unwrap();
        assert_eq!(night.budget_hours, 6, "数据日 = 昨天 → 预算字段有值");
        assert_eq!(
            night.remaining_secs,
            6 * 3_600,
            "本班剩余 = 上限 − 本班前已累计(0)"
        );
        assert_eq!(night.used_secs, 4 * 3_600, "已用 = 22:00 → 02:00");
        assert_eq!(night.duty_ended_at, None, "末窗未关 = 在岗");

        let today = day_summary_inner(&ctx, 0, now).unwrap();
        assert_eq!(today.budget_hours, 0, "日历今日不是数据日 → 字段保持 0");
        assert_eq!(today.remaining_secs, 0);
        assert_eq!(today.used_secs, 0);
    }
}
