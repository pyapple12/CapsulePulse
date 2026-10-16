//! 会话命令：开始/暂停/继续/重开/快照——转发状态机 + 落库 + 提醒评估快照。
//! 快照与提醒决策在此产出（纯逻辑可测），提醒副作用（emit/通知）交提醒子模块执行。
//! PL005：计时门禁（未上班严格拒绝）+ 段起止事件留痕（按下时间点，图谱数据源）。

use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

use super::workday::require_on_duty;
use super::{lock, now_secs, poison, AppContext, CommandError};
use crate::reminder::ReminderConfig;
use crate::session::{Clock, SessionState, TimerAction};
use crate::workday::EventKind;

/// status 命令返回体：前端展示所需会话快照（serde 结构单一来源，TS 侧镜像）。
#[derive(Debug, Serialize)]
pub struct SessionStatus {
    /// 状态标识：idle / running / paused。
    pub state: &'static str,
    /// 累计工作毫秒数（Running 态为现算值；十分秒位显示的取数源，2026-09-08 用户定案）。
    pub total_ms: u64,
    /// 是否在岗中（未上班时前端置灰计时按钮；后端门禁是第二道保险）。
    pub on_duty: bool,
    /// 当前数据日偏移（PATCH005.4）：在岗 → 该班打卡日相对日历今日的天数偏移（正常 0 / −1）；
    /// 未在岗 → 0。时钟与统计据此锚定"进行中的数据日"（跨零点夜班没下班就不切换）。
    pub data_day_offset: i64,
    /// 本段连续工作毫秒（PATCH010.6）：`segment_secs` 语义——**仅 Running 非零**，
    /// Paused/Idle 恒为 0。沙漏水位与设置板阈值下限的**单一事实源**。
    pub segment_ms: u64,
    /// 本段是否已触发过提醒（PATCH010.6）：设置板据此**锁定阈值行**（铃响后只有暂停才能改）。
    pub reminder_fired: bool,
}

/// 提醒决策（评估纯逻辑的输出载体；副作用由提醒子模块执行）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReminderDecision {
    /// 是否应触发提醒。
    pub(super) fire: bool,
    /// 当前阈值（分钟，供通知文案）。
    pub(super) threshold_min: u32,
    /// 系统通知开关。
    pub(super) notify_enabled: bool,
}

/// 落一个段末事件（PL027：段时长由流水配对推出，**不再写 sessions 行**；单事务写入）。
/// at 为 None = 记账当下（自动下班传回填时刻）。
fn persist_session_with_end<C: Clock>(
    ctx: &AppContext<C>,
    _segment: Duration,
    at: Option<i64>,
) -> Result<(), CommandError> {
    let at = at.unwrap_or(now_secs(ctx)?);
    poison("存储", ctx.storage.lock())?.insert_event(at, EventKind::SegmentEnd)?;
    Ok(())
}

/// 清除提醒触发状态（暂停/开始/重开 = 新段；"暂停即重置"定案的接线点）；锁中毒严格报错。
fn clear_reminder_fire<C: Clock>(ctx: &AppContext<C>) -> Result<(), CommandError> {
    poison("提醒", ctx.fire.lock())?.clear();
    Ok(())
}

/// 记录段起止事件（按下时间点留痕；时刻取当前真实时钟，供时间图谱归约）。
fn mark_segment<C: Clock>(ctx: &AppContext<C>, kind: EventKind) -> Result<(), CommandError> {
    let at = now_secs(ctx)?;
    poison("存储", ctx.storage.lock())?.insert_event(at, kind)?;
    Ok(())
}

/// 开始新会话：仅 Idle 且在岗合法（未上班严格拒绝，前后端双保险的后端面）；
/// 提醒触发状态清零（新段），段起点留痕；留痕失败回滚回 Idle（FIX002.1）。
pub(super) fn start_session<C: Clock>(ctx: &AppContext<C>) -> Result<(), CommandError> {
    require_on_duty(ctx)?;
    lock(ctx)?.start()?;
    let outcome =
        mark_segment(ctx, EventKind::SegmentStart).and_then(|()| clear_reminder_fire(ctx));
    if let Err(err) = outcome {
        lock(ctx)?.reset(); // 回滚：留痕失败回到 start 前态（Idle，累计恒零），计时未开始
        return Err(err);
    }
    super::workday::beat_after_action(ctx);
    Ok(())
}

/// 暂停（at = None 记账当下；自动下班收段传 Some(回填时刻)——FIX002.4 口径）。
/// 本段时长 > 0 则落库；提醒触发状态清零（暂停即重置）；段终点留痕。
/// 落库/留痕失败回滚状态回 Running{原起点} 并扣回本段——数据未落库，计时继续（FIX002.1）。
pub(super) fn pause_session_at<C: Clock>(
    ctx: &AppContext<C>,
    at: Option<i64>,
) -> Result<(), CommandError> {
    // 状态先绑定再 match：match 暂存值（scrutinee）里的锁临时会活到 match 结束，
    // 非_running 分支内再取同一把锁即自锁死锁（FIX002 实测教训）
    let snapshot = *lock(ctx)?.state();
    let start = match snapshot {
        SessionState::Running { start } => start,
        // 非 Running：转发 pause() 的权威错误（NotRunning）
        _ => {
            lock(ctx)?.pause()?;
            return Ok(());
        }
    };
    let segment = lock(ctx)?.pause()?;
    let outcome =
        clear_reminder_fire(ctx).and_then(|()| persist_session_with_end(ctx, segment, at));
    if let Err(err) = outcome {
        // 回滚后数据未落库——落诊断日志（容错白名单 ⑥ 通道；FIX002.7）
        crate::diag::log(&format!("暂停落库/留痕失败已回滚（计时继续）：{err}"));
        lock(ctx)?.undo_pause(SessionState::Running { start }, segment);
        return Err(err);
    }
    super::workday::beat_after_action(ctx);
    Ok(())
}

/// 暂停（记账当下；托盘/热键/前端共用入口）。
pub(super) fn pause_session<C: Clock>(ctx: &AppContext<C>) -> Result<(), CommandError> {
    pause_session_at(ctx, None)
}

/// 继续：仅 Paused 合法，累计被继承；提醒触发状态清零（新段起算）；段起点留痕；
/// 留痕失败回滚回 Paused（FIX002.1——resume 不动累计，快照即完整体）。
/// （在岗不变量：Paused 只会出现在在岗期间——下班时会话已复位 Idle，故无需再过门禁。）
fn resume_session<C: Clock>(ctx: &AppContext<C>) -> Result<(), CommandError> {
    let snapshot = *lock(ctx)?.state();
    lock(ctx)?.resume()?;
    let outcome =
        mark_segment(ctx, EventKind::SegmentStart).and_then(|()| clear_reminder_fire(ctx));
    if let Err(err) = outcome {
        lock(ctx)?.restore_state(snapshot);
        return Err(err);
    }
    super::workday::beat_after_action(ctx);
    Ok(())
}

/// 运行态设置意图（PATCH002）：前端只发"工作 / 休息"意图，`start / resume / pause` 的裁决
/// 全在 Rust 侧——原实现由前端按自身缓存的会话态自选命令，快照不同步即撞 `NotIdle`
/// （本次"三段切不动、只能靠重置"BUG 的根因之一）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RunMode {
    /// 工作：Idle→start（新段）、Paused→resume（累计继承）、Running→幂等。
    Work,
    /// 休息：Running→pause（收段落库）、Paused/Idle→幂等。
    Rest,
}

/// 设置运行态（PATCH002）：按当前态裁决，**幂等**（重复设置不报错、不动累计、不重复留痕）。
/// 注意：先把 state 绑定出来再 match——match 暂存值里的锁临时会活到 match 结束，
/// 分支内再取同一把锁即自锁死锁（FIX002 实测教训）。
pub(super) fn set_run_mode_inner<C: Clock>(
    ctx: &AppContext<C>,
    mode: RunMode,
) -> Result<(), CommandError> {
    let state = *lock(ctx)?.state();
    match (mode, state) {
        (RunMode::Work, SessionState::Idle) => start_session(ctx),
        (RunMode::Work, SessionState::Paused { .. }) => resume_session(ctx),
        (RunMode::Work, SessionState::Running { .. }) => Ok(()),
        (RunMode::Rest, SessionState::Running { .. }) => pause_session(ctx),
        (RunMode::Rest, SessionState::Paused { .. } | SessionState::Idle) => Ok(()),
    }
}

/// 计时切换（托盘菜单与全局热键共用）：按当前态执行 start/pause/resume，返回实际执行的动作。
/// （未上班时 start 分支被门禁拒绝：托盘/热键在托盘日志可见错误，UI 按钮已置灰。）
pub(crate) fn toggle_session<C: Clock>(ctx: &AppContext<C>) -> Result<TimerAction, CommandError> {
    let action = lock(ctx)?.state().toggle_action();
    match action {
        TimerAction::Start => start_session(ctx)?,
        TimerAction::Pause => pause_session(ctx)?,
        TimerAction::Resume => resume_session(ctx)?,
    }
    Ok(action)
}

/// 班内有运行段则收段落库（FIX002.14：下班与退出收尾共用；at 语义见 pause_session_at）；
/// Paused/Idle 无事。
pub(crate) fn close_running_segment<C: Clock>(
    ctx: &AppContext<C>,
    at: Option<i64>,
) -> Result<(), CommandError> {
    if matches!(lock(ctx)?.state(), SessionState::Running { .. }) {
        pause_session_at(ctx, at)?;
    }
    Ok(())
}

/// 退出收尾：班内有运行段先收段落库（数据不丢定案），Paused 段已在最近一次 pause 落库、Idle 无事。
pub(crate) fn persist_before_quit<C: Clock>(ctx: &AppContext<C>) -> Result<(), CommandError> {
    close_running_segment(ctx, None)
}

/// 重开（先落库再归零；PATCH002 语义更新）：班内运行段先经 `close_running_segment` 收段落库
/// （复用 pause_session_at 的落库/留痕/回滚链，FIX002.14），再在流水里留一条 `session_reset`
/// 标记（PL027 定案 B5 / PATCH004.2：归约忽略它，但它是"累计基准 = 最后一条 `session_reset`
/// 或本次打卡"的锚点），最后把会话重置为**归零休息**（`reset_to_paused`）——大计时器清零且
/// **不自动开跑**，等用户按「工作」再计。未上班严格拒绝（重开隐含"在岗"前提）。
/// 沿革：原实现 reset+start（立即开跑）并额外写一条 SegmentStart；新语义下重置落在休息态、
/// 无工作段开始，故该事件写入与其留痕回滚分支（FIX003.9）随本次语义变更退役——标记写入改为
/// 收段之后、归零之前的独立一步，写失败整令报错（严格抛错主线：段已收、计时未归零）。
fn restart_session<C: Clock>(ctx: &AppContext<C>) -> Result<(), CommandError> {
    require_on_duty(ctx)?;
    close_running_segment(ctx, None)?;
    mark_segment(ctx, EventKind::SessionReset)?;
    lock(ctx)?.reset_to_paused();
    super::workday::beat_after_action(ctx);
    Ok(())
}

/// 三态 → 前端状态标识（serde 泄露面最小化：内部计时字段不出 IPC）。
fn state_key(state: &SessionState) -> &'static str {
    match state {
        SessionState::Idle => "idle",
        SessionState::Running { .. } => "running",
        SessionState::Paused { .. } => "paused",
    }
}

/// 会话快照 + 提醒决策（评估纯逻辑；副作用由命令包装层执行）。
/// 注意：session 与 workday 两锁不嵌套持有（各自独立作用域顺序取用，规避反向锁序）。
/// PATCH010.6：快照增 `segment_ms`（本段时长）与 `reminder_fired`（本段是否已触发过提醒）——
/// 前端沙漏水位与设置板阈值行的**单一事实源**；锁序仍守 workday → session → settings → fire。
pub(super) fn status_snapshot<C: Clock>(
    ctx: &AppContext<C>,
) -> Result<(SessionStatus, ReminderDecision), CommandError> {
    let (state, total_ms, segment) = {
        let session = lock(ctx)?;
        (
            state_key(session.state()),
            session.total().as_millis() as u64,
            session.segment_secs(),
        )
    };
    let on_duty_now = poison("工作日", ctx.workday.lock())?.is_on_duty();
    let (threshold_min, notify_enabled, reminder_fired, fire_now) = {
        let settings = poison("设置", ctx.settings.lock())?;
        let config = ReminderConfig::from_minutes(settings.threshold_min);
        let mut fire = poison("提醒", ctx.fire.lock())?;
        let should = fire.evaluate(segment, &config);
        // 取值在 evaluate 之后：本拍刚触发的这一帧也如实报告"已触发过"（与 reminder-due 事件同拍）
        (
            settings.threshold_min,
            settings.notify_enabled,
            fire.last_fired().is_some(),
            should,
        )
    };
    let status = SessionStatus {
        state,
        total_ms,
        on_duty: on_duty_now,
        data_day_offset: super::workday::data_day_offset(ctx, now_secs(ctx)?)?,
        segment_ms: segment.as_millis() as u64,
        reminder_fired,
    };
    let decision = ReminderDecision {
        fire: fire_now,
        threshold_min,
        notify_enabled,
    };
    Ok((status, decision))
}

/// 开始新会话（仅 Idle 合法）。
#[tauri::command]
pub fn session_start(handle: State<'_, AppContext>) -> Result<(), CommandError> {
    start_session(&handle)
}

/// 暂停进行中的会话（本段时长 > 0 时落库）。
#[tauri::command]
pub fn session_pause(handle: State<'_, AppContext>) -> Result<(), CommandError> {
    pause_session(&handle)
}

/// 继续暂停的会话（累计继承）。
#[tauri::command]
pub fn session_resume(handle: State<'_, AppContext>) -> Result<(), CommandError> {
    resume_session(&handle)
}

/// 重开（重置）：先收段落库，再归零并**回到休息态**（PATCH002：不再自动开跑）。
#[tauri::command]
pub fn session_restart(handle: State<'_, AppContext>) -> Result<(), CommandError> {
    restart_session(&handle)
}

/// 设置运行态（工作 / 休息）——三段控件入口（PATCH002：前端只发意图，裁决在 Rust 侧）。
#[tauri::command]
pub fn session_set_run_mode(
    handle: State<'_, AppContext>,
    mode: RunMode,
) -> Result<(), CommandError> {
    set_run_mode_inner(&handle, mode)
}

/// 查询会话快照（前端 100ms tick 拉取；顺路执行提醒评估与自动下班检查）。
/// async：评估链移出主线程（FIX002.8，本命令是最高频调用口）。
#[tauri::command]
pub async fn session_status(
    handle: State<'_, AppContext>,
    app: AppHandle,
) -> Result<SessionStatus, CommandError> {
    let (status, decision) = status_snapshot(&handle)?;
    super::reminder::deliver_reminder(&app, decision)?;
    // 自动下班待决（PL025.8）：到点前 60s 提示一次——发事件（前端弹倒计时框）+ 把窗口拉到前台
    if let Some(due_at) = super::workday::auto_out_pending_inner(&handle, now_secs(&handle)?)? {
        app.emit("workday-auto-out-pending", due_at)
            .map_err(|e| CommandError::Event(e.to_string()))?;
        crate::show_main_window(&app);
    }
    // 自动下班检查挂现有 tick 评估口（与提醒同构）：到点以基准时刻下班并发事件；
    // 本帧 status 可能仍是关闭前的快照，前端下一 tick（100ms）自然对齐
    if let Some(out_at) = super::workday::try_auto_clock_out(&handle)? {
        super::workday::deliver_auto_out(&app, &handle, out_at)?;
    }
    Ok(status)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::test_support::{begin_duty, ctx, secs, FakeClock};
    use crate::session::SessionError;
    use crate::workday::WorkdayError;

    /// 状态映射三态正确。
    #[test]
    fn state_key_maps_three_states() {
        assert_eq!(state_key(&SessionState::Idle), "idle");
        assert_eq!(
            state_key(&SessionState::Running {
                start: Duration::ZERO
            }),
            "running"
        );
        assert_eq!(
            state_key(&SessionState::Paused {
                elapsed: Duration::ZERO
            }),
            "paused"
        );
    }

    /// PL005 门禁：未上班时 start/restart 严格拒绝（后端保险面），库与事件零副作用。
    #[test]
    fn timer_actions_off_duty_are_rejected() {
        let ctx = ctx(FakeClock::new());
        assert!(matches!(
            start_session(&ctx),
            Err(CommandError::Workday(WorkdayError::NotOnDuty))
        ));
        assert!(matches!(
            restart_session(&ctx),
            Err(CommandError::Workday(WorkdayError::NotOnDuty))
        ));
        assert_eq!(ctx.storage.lock().unwrap().session_count().unwrap(), 0);
        assert_eq!(
            ctx.storage
                .lock()
                .unwrap()
                .events_between(0, i64::MAX)
                .unwrap()
                .len(),
            0
        );
    }

    /// PL005 快照：on_duty 随打卡翻转（前端置灰取数源）。
    #[test]
    fn status_reports_on_duty_flag() {
        let ctx = ctx(FakeClock::new());
        assert!(!status_snapshot(&ctx).unwrap().0.on_duty);
        begin_duty(&ctx);
        assert!(status_snapshot(&ctx).unwrap().0.on_duty);
    }

    /// PL005 段事件留痕：start/resume 记 segment_start、pause 记 segment_end，零秒段仍留痕。
    #[test]
    fn timer_actions_mark_segment_events() {
        let clock = FakeClock::new();
        let ctx = ctx(clock.clone());
        begin_duty(&ctx);
        clock.advance(secs(30));
        pause_session(&ctx).unwrap();
        resume_session(&ctx).unwrap();
        let kinds: Vec<EventKind> = ctx
            .storage
            .lock()
            .unwrap()
            .events_between(0, i64::MAX)
            .unwrap()
            .into_iter()
            .map(|(_, kind)| kind)
            .collect();
        assert_eq!(
            kinds,
            [
                EventKind::ClockIn,
                EventKind::SegmentStart,
                EventKind::SegmentEnd,
                EventKind::SegmentStart
            ],
            "begin_duty 的 clock_in + 段起止留痕"
        );
    }

    /// T3：start → advance 100s → pause：库内一段（100s）且 status 报 paused。
    #[test]
    fn start_pause_persists_and_reports() {
        let clock = FakeClock::new();
        let ctx = ctx(clock.clone());
        begin_duty(&ctx);
        clock.advance(secs(100));
        pause_session(&ctx).unwrap();

        assert_eq!(ctx.storage.lock().unwrap().session_count().unwrap(), 1);
        assert_eq!(ctx.storage.lock().unwrap().all_total().unwrap(), 100);
        assert_eq!(status_snapshot(&ctx).unwrap().0.state, "paused");
    }

    /// 零秒段跳过落库（不写噪音行）。
    #[test]
    fn zero_segment_skips_persist() {
        let ctx = ctx(FakeClock::new());
        begin_duty(&ctx);
        pause_session(&ctx).unwrap();
        assert_eq!(ctx.storage.lock().unwrap().session_count().unwrap(), 0);
    }

    /// U2/T3：非法转发严格报错 + 合法路径放行——覆盖 Idle/Running/Paused 三态交叉。
    /// PATCH002 起打卡即工作：在岗的起始态是 **Running**，Idle 只出现在未上班。
    #[test]
    fn invalid_forwarding_errors() {
        // Idle（未上班）下 pause / resume 非法
        let idle_ctx = ctx(FakeClock::new());
        assert!(matches!(
            pause_session(&idle_ctx),
            Err(CommandError::Session(SessionError::NotRunning))
        ));
        assert!(matches!(
            resume_session(&idle_ctx),
            Err(CommandError::Session(SessionError::NotPaused))
        ));

        // 在岗：打卡即 Running → 重复 start 非法、pause 合法
        let ctx = ctx(FakeClock::new());
        begin_duty(&ctx);
        assert!(matches!(
            start_session(&ctx),
            Err(CommandError::Session(SessionError::NotIdle))
        ));
        assert!(pause_session(&ctx).is_ok());
        // Paused 下 start 非法（重开须走 restart）；resume 合法
        assert!(matches!(
            start_session(&ctx),
            Err(CommandError::Session(SessionError::NotIdle))
        ));
        assert!(resume_session(&ctx).is_ok());
    }

    /// T3：Running 态重开——本段落库后归零回**休息**（PATCH002：不再自动开跑）。
    #[test]
    fn restart_from_running_persists_segment() {
        let clock = FakeClock::new();
        let ctx = ctx(clock.clone());
        begin_duty(&ctx);
        clock.advance(secs(100));
        restart_session(&ctx).unwrap();

        assert_eq!(ctx.storage.lock().unwrap().session_count().unwrap(), 1);
        assert_eq!(ctx.storage.lock().unwrap().all_total().unwrap(), 100);
        let s = status_snapshot(&ctx).unwrap().0;
        assert_eq!(s.state, "paused", "重置后停在休息态（PATCH002）");
        assert_eq!(s.total_ms, 0, "大计时器归零");
    }

    /// T3：Paused 态重开不重复落库（段已在最近一次 pause 入库），直接归零回休息。
    #[test]
    fn restart_from_paused_does_not_duplicate() {
        let clock = FakeClock::new();
        let ctx = ctx(clock.clone());
        begin_duty(&ctx);
        clock.advance(secs(50));
        pause_session(&ctx).unwrap();
        restart_session(&ctx).unwrap();

        assert_eq!(ctx.storage.lock().unwrap().session_count().unwrap(), 1);
        assert_eq!(ctx.storage.lock().unwrap().all_total().unwrap(), 50);
        assert_eq!(status_snapshot(&ctx).unwrap().0.state, "paused");
    }

    /// PATCH004.2：重置必须落一条 `session_reset` 标记（PL027 定案 B5）。归约忽略它，但它是
    /// "大计时器累计基准 = 最后一条 `session_reset` 或本次打卡"的流水锚点——此前实现只收段、
    /// 无标记，于是重置后的累计在流水里无法重推（只存在于 runtime 缓存）。
    #[test]
    fn restart_appends_session_reset_marker() {
        let clock = FakeClock::new();
        let ctx = ctx(clock.clone());
        begin_duty(&ctx);
        clock.advance(secs(100));
        restart_session(&ctx).unwrap();

        let events = ctx
            .storage
            .lock()
            .unwrap()
            .events_between(0, 10_000)
            .unwrap();
        let kinds: Vec<EventKind> = events.into_iter().map(|(_, k)| k).collect();
        assert_eq!(
            kinds
                .iter()
                .filter(|k| **k == EventKind::SessionReset)
                .count(),
            1,
            "重置落且仅落一条 session_reset"
        );
        assert!(kinds.contains(&EventKind::SegmentEnd), "收段落库照旧");
    }

    /// T3+N1：提醒决策随 status 评估——49 分钟不发、50 分钟发、5 分钟内不重发；动作清零触发点。
    #[test]
    fn reminder_decision_fires_and_resets() {
        let clock = FakeClock::new();
        let ctx = ctx(clock.clone());
        begin_duty(&ctx);
        clock.advance(secs(49 * 60));
        let (_, d) = status_snapshot(&ctx).unwrap();
        assert!(!d.fire);

        clock.advance(secs(60));
        let (_, d) = status_snapshot(&ctx).unwrap();
        assert!(d.fire);

        // 5 分钟间隔内不重发
        let (_, d) = status_snapshot(&ctx).unwrap();
        assert!(!d.fire);

        // 暂停（暂停即重置）→ 继续 → 未达新阈值不触发
        pause_session(&ctx).unwrap();
        resume_session(&ctx).unwrap();
        let (_, d) = status_snapshot(&ctx).unwrap();
        assert!(!d.fire);

        clock.advance(secs(50 * 60));
        let (_, d) = status_snapshot(&ctx).unwrap();
        assert!(d.fire);
        assert_eq!(d.threshold_min, 50);
    }

    /// FIX002.1：storage 锁污染（持久化失败）时 pause 回滚状态回 Running——数据未落库计时继续。
    #[test]
    fn pause_rolls_back_running_on_persist_failure() {
        let clock = FakeClock::new();
        let ctx = ctx(clock.clone());
        begin_duty(&ctx);
        clock.advance(secs(60));
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = ctx.storage.lock().unwrap();
            panic!("污染 storage 锁");
        }));
        assert!(matches!(
            pause_session(&ctx),
            Err(CommandError::Poisoned("存储"))
        ));
        let s = status_snapshot(&ctx).unwrap().0;
        assert_eq!(s.state, "running", "落库失败回滚：计时继续而非停在 Paused");
    }

    /// FIX002.1：start 留痕失败回滚回 Idle——计时未开始。
    /// PATCH002 起打卡即工作，在岗常态是 Running；本用例把会话显式复位回 Idle（"未上班"语义）
    /// 以保留 start 回滚分支的覆盖。
    #[test]
    fn start_rolls_back_to_idle_on_mark_failure() {
        let ctx = ctx(FakeClock::new());
        begin_duty(&ctx);
        lock(&ctx).unwrap().reset(); // 复位回 Idle（PATCH002 后仅在未上班出现）
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = ctx.storage.lock().unwrap();
            panic!("污染 storage 锁");
        }));
        assert!(matches!(
            start_session(&ctx),
            Err(CommandError::Poisoned("存储"))
        ));
        assert_eq!(status_snapshot(&ctx).unwrap().0.state, "idle");
    }

    /// FIX002.1：resume 留痕失败回滚回 Paused——累计保持、仍在暂停态。
    #[test]
    fn resume_rolls_back_to_paused_on_mark_failure() {
        let clock = FakeClock::new();
        let ctx = ctx(clock.clone());
        begin_duty(&ctx);
        clock.advance(secs(30));
        pause_session(&ctx).unwrap();
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = ctx.storage.lock().unwrap();
            panic!("污染 storage 锁");
        }));
        assert!(matches!(
            resume_session(&ctx),
            Err(CommandError::Poisoned("存储"))
        ));
        assert_eq!(status_snapshot(&ctx).unwrap().0.state, "paused");
    }

    /// PATCH002：Running 态重开在落库步失败——收段经 pause 的回滚链保持 **Running**（段未落库、
    /// 计时继续，非数据丢失态），重置整体不生效（严格抛错）。
    /// 沿革：原实现（FIX003.9）重开是"reset+start"，落库失败停 Paused；现改为复用
    /// `close_running_segment`（含 pause 回滚链），故停 Running。
    #[test]
    fn restart_running_keeps_running_on_persist_failure() {
        let clock = FakeClock::new();
        let ctx = ctx(clock.clone());
        begin_duty(&ctx);
        clock.advance(secs(60));
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = ctx.storage.lock().unwrap();
            panic!("污染 storage 锁");
        }));
        assert!(matches!(
            restart_session(&ctx),
            Err(CommandError::Poisoned("存储"))
        ));
        assert_eq!(
            status_snapshot(&ctx).unwrap().0.state,
            "running",
            "收段落库失败回滚：计时继续（段未丢）"
        );
    }

    /// 锁中毒严格报错：fire 锁被污染后动作命令传播 Poisoned，而非静默跳过清零（回归锚）。
    #[test]
    fn poisoned_fire_lock_is_strict_error() {
        let ctx = ctx(FakeClock::new());
        begin_duty(&ctx);
        // 持锁 panic 污染 fire 锁（catch_unwind 隔离，仅取中毒副作用）
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = ctx.fire.lock().unwrap();
            panic!("污染 fire 锁");
        }));
        assert!(matches!(
            pause_session(&ctx),
            Err(CommandError::Poisoned("提醒"))
        ));
    }

    /// 托盘/热键共用的计时切换：按当前态切换——PATCH002 起打卡即工作，在岗态只在这两态间循环
    /// （Running→pause、Paused→resume）；Idle→start 分支仅在未上班出现（被门禁拒绝，见上用例）。
    #[test]
    fn toggle_session_cycles_three_states() {
        let clock = FakeClock::new();
        let ctx = ctx(clock.clone());
        begin_duty(&ctx);
        assert_eq!(
            status_snapshot(&ctx).unwrap().0.state,
            "running",
            "打卡即工作"
        );
        clock.advance(secs(10));
        assert_eq!(toggle_session(&ctx).unwrap(), TimerAction::Pause);
        assert_eq!(status_snapshot(&ctx).unwrap().0.state, "paused");
        assert_eq!(toggle_session(&ctx).unwrap(), TimerAction::Resume);
        assert_eq!(status_snapshot(&ctx).unwrap().0.state, "running");
    }

    /// 退出收尾：Running 态先落库（数据不丢）；Paused 段已在 pause 落库、二次收尾幂等；Idle 无事。
    /// PATCH002 起打卡即工作：在岗初始态即 Running，Idle 只在未上班（用独立 ctx 覆盖）。
    #[test]
    fn persist_before_quit_persists_running_segment() {
        // Idle（未上班）：无事
        let idle_ctx = ctx(FakeClock::new());
        persist_before_quit(&idle_ctx).unwrap();
        assert_eq!(idle_ctx.storage.lock().unwrap().session_count().unwrap(), 0);

        // Running（打卡即工作）：先落库再收尾
        let clock = FakeClock::new();
        let ctx = ctx(clock.clone());
        begin_duty(&ctx);
        clock.advance(secs(30));
        persist_before_quit(&ctx).unwrap();
        assert_eq!(ctx.storage.lock().unwrap().all_total().unwrap(), 30);
        // 收尾后为 Paused，二次收尾幂等（不重复落库）
        persist_before_quit(&ctx).unwrap();
        assert_eq!(ctx.storage.lock().unwrap().all_total().unwrap(), 30);
    }

    /// PATCH002：`set_run_mode` 矩阵——前端只发意图，Rust 按当前态裁决且**幂等**。
    #[test]
    fn set_run_mode_resolves_from_any_state() {
        let clock = FakeClock::new();
        let ctx = ctx(clock.clone());
        begin_duty(&ctx); // 打卡即工作 → Running

        // Running + Work → 幂等（不报错、不动累计）
        clock.advance(secs(10));
        set_run_mode_inner(&ctx, RunMode::Work).unwrap();
        let s = status_snapshot(&ctx).unwrap().0;
        assert_eq!(s.state, "running");
        assert_eq!(s.total_ms, 10_000, "幂等不重置累计");

        // Running + Rest → Paused（收段落库）
        set_run_mode_inner(&ctx, RunMode::Rest).unwrap();
        assert_eq!(status_snapshot(&ctx).unwrap().0.state, "paused");
        assert_eq!(ctx.storage.lock().unwrap().all_total().unwrap(), 10);

        // Paused + Rest → 幂等
        set_run_mode_inner(&ctx, RunMode::Rest).unwrap();
        assert_eq!(status_snapshot(&ctx).unwrap().0.state, "paused");

        // Paused + Work → resume（累计继承；暂停期间不累计，故推进假钟放在 resume 之后）
        set_run_mode_inner(&ctx, RunMode::Work).unwrap();
        clock.advance(secs(5));
        let s = status_snapshot(&ctx).unwrap().0;
        assert_eq!(s.state, "running");
        assert_eq!(s.total_ms, 15_000, "累计继承（10 + 5）");

        // 未在岗：Work 被门禁拒绝（库零副作用），Rest 幂等
        //（全路径调用 ctx 助手：本用例已把 ctx 绑定为上下文变量，避免同名遮蔽）
        let off_ctx = crate::commands::test_support::ctx(FakeClock::new());
        assert!(matches!(
            set_run_mode_inner(&off_ctx, RunMode::Work),
            Err(CommandError::Workday(WorkdayError::NotOnDuty))
        ));
        set_run_mode_inner(&off_ctx, RunMode::Rest).unwrap();
        assert_eq!(off_ctx.storage.lock().unwrap().session_count().unwrap(), 0);
    }

    /// PATCH002：Idle + Work 走 start 分支（在岗但会话被复位回 Idle 的边界态）。
    #[test]
    fn set_run_mode_work_from_idle_starts() {
        let ctx = ctx(FakeClock::new());
        begin_duty(&ctx);
        lock(&ctx).unwrap().reset(); // 复位回 Idle（PATCH002 后仅在未上班出现）
        set_run_mode_inner(&ctx, RunMode::Work).unwrap();
        assert_eq!(status_snapshot(&ctx).unwrap().0.state, "running");
    }

    /// PATCH010.6：快照暴露"本段时长"与"本段是否已触发过提醒"——沙漏水位与设置板阈值行的
    /// 单一事实源（Running 有值 / Paused 归零；触发当拍即如实上报）。
    #[test]
    fn session_status_exposes_segment_and_fired() {
        let clock = FakeClock::new();
        let ctx = ctx(clock.clone());
        begin_duty(&ctx); // 打卡即工作 → Running

        // Running：本段实时；未达阈值（默认 50 分钟）⇒ 未触发
        clock.advance(secs(150));
        let s = status_snapshot(&ctx).unwrap().0;
        assert_eq!(s.state, "running");
        assert_eq!(s.segment_ms, 150_000);
        assert!(!s.reminder_fired, "未达阈值不得报已触发");

        // 恰达阈值：同一拍 fire = true 且 reminder_fired = true（与 reminder-due 事件同拍）
        clock.advance(secs(2_850)); // 累计 3 000s = 50 分钟
        let (s, d) = status_snapshot(&ctx).unwrap();
        assert!(d.fire);
        assert!(s.reminder_fired);
        assert_eq!(s.segment_ms, 3_000_000);

        // Paused：本段归零（沙漏据此沉底）且触发态被 clear（阈值随之解锁）
        set_run_mode_inner(&ctx, RunMode::Rest).unwrap();
        let s = status_snapshot(&ctx).unwrap().0;
        assert_eq!(s.state, "paused");
        assert_eq!(s.segment_ms, 0);
        assert!(!s.reminder_fired, "暂停即 clear（PATCH002 语义）");
    }
}
