//! 会话命令：开始/暂停/继续/重开/快照——转发状态机 + 落库 + 提醒评估快照。
//! 快照与提醒决策在此产出（纯逻辑可测），提醒副作用（emit/通知）交提醒子模块执行。

use std::sync::Mutex;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, State};

use super::{lock, poison, wall_now_secs, AppContext, CommandError};
use crate::reminder::ReminderConfig;
use crate::session::{Clock, SessionState, TimerAction};
use crate::storage::Storage;

/// status 命令返回体：前端展示所需会话快照（serde 结构单一来源，TS 侧镜像）。
#[derive(Debug, Serialize)]
pub struct SessionStatus {
    /// 状态标识：idle / running / paused。
    pub state: &'static str,
    /// 累计工作毫秒数（Running 态为现算值；十分秒位显示的取数源，2026-09-08 用户定案）。
    pub total_ms: u64,
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

/// 落一个工作段：零秒段跳过（不写噪音行）；started_at = wall_now − 段秒。
fn persist_segment(storage: &Mutex<Storage>, segment: Duration) -> Result<(), CommandError> {
    if segment.is_zero() {
        return Ok(());
    }
    let started_at = wall_now_secs()? - segment.as_secs() as i64;
    poison(storage.lock())?.add_session(started_at, segment.as_secs() as i64)?;
    Ok(())
}

/// 清除提醒触发状态（暂停/开始/重开 = 新段；"暂停即重置"定案的接线点）；锁中毒严格报错。
fn clear_reminder_fire<C: Clock>(ctx: &AppContext<C>) -> Result<(), CommandError> {
    poison(ctx.fire.lock())?.clear();
    Ok(())
}

/// 开始新会话：仅 Idle 合法；提醒触发状态清零（新段）。
fn start_session<C: Clock>(ctx: &AppContext<C>) -> Result<(), CommandError> {
    lock(ctx)?.start()?;
    clear_reminder_fire(ctx)?;
    Ok(())
}

/// 暂停：本段时长 > 0 则落库；提醒触发状态清零（暂停即重置，用户定案）。
fn pause_session<C: Clock>(ctx: &AppContext<C>) -> Result<(), CommandError> {
    let segment = lock(ctx)?.pause()?;
    clear_reminder_fire(ctx)?;
    persist_segment(&ctx.storage, segment)
}

/// 继续：仅 Paused 合法，累计被继承；提醒触发状态清零（新段起算）。
fn resume_session<C: Clock>(ctx: &AppContext<C>) -> Result<(), CommandError> {
    lock(ctx)?.resume()?;
    clear_reminder_fire(ctx)?;
    Ok(())
}

/// 计时切换（托盘菜单与全局热键共用）：按当前态执行 start/pause/resume，返回实际执行的动作。
pub(crate) fn toggle_session<C: Clock>(ctx: &AppContext<C>) -> Result<TimerAction, CommandError> {
    let action = lock(ctx)?.state().toggle_action();
    match action {
        TimerAction::Start => start_session(ctx)?,
        TimerAction::Pause => pause_session(ctx)?,
        TimerAction::Resume => resume_session(ctx)?,
    }
    Ok(action)
}

/// 退出收尾：Running 态先落库（数据不丢定案），Paused 段已在最近一次 pause 落库、Idle 无事。
pub(crate) fn persist_before_quit<C: Clock>(ctx: &AppContext<C>) -> Result<(), CommandError> {
    if matches!(lock(ctx)?.state(), SessionState::Running { .. }) {
        pause_session(ctx)?;
    }
    Ok(())
}

/// 重开（先落库再重开，用户定案）：仅 Running 有未落库本段——Paused 的段已在
/// 最近一次 pause 落库、Paused 期间不累计；随后 reset + start 单锁原子完成。
fn restart_session<C: Clock>(ctx: &AppContext<C>) -> Result<(), CommandError> {
    let mut session = lock(ctx)?;
    if matches!(session.state(), SessionState::Running { .. }) {
        let segment = session.pause()?;
        persist_segment(&ctx.storage, segment)?;
    }
    session.reset();
    session.start()?;
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
fn status_snapshot<C: Clock>(
    ctx: &AppContext<C>,
) -> Result<(SessionStatus, ReminderDecision), CommandError> {
    let status = {
        let session = lock(ctx)?;
        SessionStatus {
            state: state_key(session.state()),
            total_ms: session.total().as_millis() as u64,
        }
    };
    let segment = lock(ctx)?.segment_secs();
    let decision = {
        let settings = poison(ctx.settings.lock())?;
        let config = ReminderConfig::from_minutes(settings.threshold_min);
        let should = poison(ctx.fire.lock())?.evaluate(segment, &config);
        ReminderDecision {
            fire: should,
            threshold_min: settings.threshold_min,
            notify_enabled: settings.notify_enabled,
        }
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

/// 重开：先落库未落库段，再归零并立即开始（任意态合法）。
#[tauri::command]
pub fn session_restart(handle: State<'_, AppContext>) -> Result<(), CommandError> {
    restart_session(&handle)
}

/// 查询会话快照（前端 100ms tick 拉取；顺路执行提醒评估与触发副作用）。
#[tauri::command]
pub fn session_status(
    handle: State<'_, AppContext>,
    app: AppHandle,
) -> Result<SessionStatus, CommandError> {
    let (status, decision) = status_snapshot(&handle)?;
    super::reminder::deliver_reminder(&app, decision)?;
    Ok(status)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::test_support::{ctx, secs, FakeClock};
    use crate::session::SessionError;

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

    /// T3：start → advance 100s → pause：库内一段（100s）且 status 报 paused。
    #[test]
    fn start_pause_persists_and_reports() {
        let clock = FakeClock::new();
        let ctx = ctx(clock.clone());
        start_session(&ctx).unwrap();
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
        start_session(&ctx).unwrap();
        pause_session(&ctx).unwrap();
        assert_eq!(ctx.storage.lock().unwrap().session_count().unwrap(), 0);
    }

    /// U2/T3：非法转发严格报错 + 合法路径放行——覆盖 Idle/Running/Paused 三态交叉。
    #[test]
    fn invalid_forwarding_errors() {
        let ctx = ctx(FakeClock::new());
        // Idle 下 pause / resume 非法
        assert!(matches!(
            pause_session(&ctx),
            Err(CommandError::Session(SessionError::NotRunning))
        ));
        assert!(matches!(
            resume_session(&ctx),
            Err(CommandError::Session(SessionError::NotPaused))
        ));
        // Running 下重复 start 非法；合法 pause 后再 start 仍非法（Paused ≠ Idle）
        start_session(&ctx).unwrap();
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

    /// T3：Running 态重开——本段落库后归零重走（先落库再重开定案）。
    #[test]
    fn restart_from_running_persists_segment() {
        let clock = FakeClock::new();
        let ctx = ctx(clock.clone());
        start_session(&ctx).unwrap();
        clock.advance(secs(100));
        restart_session(&ctx).unwrap();

        assert_eq!(ctx.storage.lock().unwrap().session_count().unwrap(), 1);
        assert_eq!(ctx.storage.lock().unwrap().all_total().unwrap(), 100);
        let s = status_snapshot(&ctx).unwrap().0;
        assert_eq!(s.state, "running");
        assert_eq!(s.total_ms, 0);
    }

    /// T3：Paused 态重开不重复落库（段已在最近一次 pause 入库），直接归零重走。
    #[test]
    fn restart_from_paused_does_not_duplicate() {
        let clock = FakeClock::new();
        let ctx = ctx(clock.clone());
        start_session(&ctx).unwrap();
        clock.advance(secs(50));
        pause_session(&ctx).unwrap();
        restart_session(&ctx).unwrap();

        assert_eq!(ctx.storage.lock().unwrap().session_count().unwrap(), 1);
        assert_eq!(ctx.storage.lock().unwrap().all_total().unwrap(), 50);
        assert_eq!(status_snapshot(&ctx).unwrap().0.state, "running");
    }

    /// T3+N1：提醒决策随 status 评估——49 分钟不发、50 分钟发、5 分钟内不重发；动作清零触发点。
    #[test]
    fn reminder_decision_fires_and_resets() {
        let clock = FakeClock::new();
        let ctx = ctx(clock.clone());
        start_session(&ctx).unwrap();
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

    /// 锁中毒严格报错：fire 锁被污染后动作命令传播 Poisoned，而非静默跳过清零（回归锚）。
    #[test]
    fn poisoned_fire_lock_is_strict_error() {
        let ctx = ctx(FakeClock::new());
        start_session(&ctx).unwrap();
        // 持锁 panic 污染 fire 锁（catch_unwind 隔离，仅取中毒副作用）
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = ctx.fire.lock().unwrap();
            panic!("污染 fire 锁");
        }));
        assert!(matches!(pause_session(&ctx), Err(CommandError::Poisoned)));
    }

    /// 托盘/热键共用的计时切换：Idle→start、Running→pause、Paused→resume 三态循环。
    #[test]
    fn toggle_session_cycles_three_states() {
        let clock = FakeClock::new();
        let ctx = ctx(clock.clone());
        assert_eq!(toggle_session(&ctx).unwrap(), TimerAction::Start);
        assert_eq!(status_snapshot(&ctx).unwrap().0.state, "running");
        clock.advance(secs(10));
        assert_eq!(toggle_session(&ctx).unwrap(), TimerAction::Pause);
        assert_eq!(status_snapshot(&ctx).unwrap().0.state, "paused");
        assert_eq!(toggle_session(&ctx).unwrap(), TimerAction::Resume);
        assert_eq!(status_snapshot(&ctx).unwrap().0.state, "running");
    }

    /// 退出收尾：Running 态先落库（数据不丢）；Paused 段已在 pause 落库、二次收尾幂等；Idle 无事。
    #[test]
    fn persist_before_quit_persists_running_segment() {
        let clock = FakeClock::new();
        let ctx = ctx(clock.clone());
        // Idle：无事
        persist_before_quit(&ctx).unwrap();
        assert_eq!(ctx.storage.lock().unwrap().session_count().unwrap(), 0);
        // Running：先落库再收尾
        start_session(&ctx).unwrap();
        clock.advance(secs(30));
        persist_before_quit(&ctx).unwrap();
        assert_eq!(ctx.storage.lock().unwrap().all_total().unwrap(), 30);
        // 收尾后为 Paused，二次收尾幂等（不重复落库）
        persist_before_quit(&ctx).unwrap();
        assert_eq!(ctx.storage.lock().unwrap().all_total().unwrap(), 30);
    }
}
