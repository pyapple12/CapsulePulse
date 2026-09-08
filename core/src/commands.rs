//! Tauri 命令层：前端 invoke 的唯一入口，只做转发——状态机逻辑全在 session.rs，本层零业务判断。
//! 命令核心逻辑抽为接收 `&AppContext` 的自由函数（脱离 tauri::State），T3 可在 cargo test 下
//! 无窗口直测（注入内存库 + 假钟）；`#[tauri::command]` 包装仅做 State 解包。
//! 落库时间戳 started_at = wall_now − 段秒，SystemTime 只在本层出现，session.rs 纯度不破。
//! 注意：自定义命令不经 ACL 白名单（系列实证），但事件/窗口等能力仍需 capabilities 授权。

use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chrono::Local;
use serde::Serialize;
use tauri::State;
use thiserror::Error;

use crate::session::{Clock, RealClock, SessionError, SessionState, WorkSession};
use crate::storage::Storage;

/// 应用级共享上下文：会话状态机 + 存储。lib.rs 经 `.manage()` 注册，各命令经 State 访问。
/// 泛型默认 [`RealClock`]（生产具体化）；测试注入假钟以确定性驱动落库断言。
/// rusqlite Connection 非 Sync，故 Storage 亦入 Mutex；锁序恒为 session → storage，无反向路径。
pub struct AppContext<C: Clock = RealClock> {
    /// 会话状态机。
    pub session: Mutex<WorkSession<C>>,
    /// 会话存储（运行时文件库，测试内存库）。
    pub storage: Mutex<Storage>,
}

/// 命令层错误：跨 IPC 序列化为字符串（前端在 promise reject 的 message 中读到文案）。
#[derive(Debug, Error)]
pub enum CommandError {
    /// 状态机拒绝（非法状态操作），文案沿用 session.rs。
    #[error(transparent)]
    Session(#[from] SessionError),
    /// 存储层错误（落库/聚合失败）。
    #[error(transparent)]
    Storage(#[from] crate::storage::StorageError),
    /// 会话锁中毒（此前持锁线程 panic）——严格报错，不静默续行。
    #[error("会话锁已中毒（此前持锁线程异常终止）")]
    Poisoned,
    /// 系统时钟早于 Unix 纪元（时间戳不可用）。
    #[error("系统时钟异常（早于 Unix 纪元）")]
    Clock,
}

impl Serialize for CommandError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

/// status 命令返回体：前端展示所需会话快照（serde 结构单一来源，TS 侧镜像）。
#[derive(Debug, Serialize)]
pub struct SessionStatus {
    /// 状态标识：idle / running / paused。
    pub state: &'static str,
    /// 累计工作毫秒数（Running 态为现算值；十分秒位显示的取数源，2026-09-08 用户定案）。
    pub total_ms: u64,
}

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

/// 取会话锁：中毒严格报错（Poisoned），不 unwrap 不吞错。
fn lock<C: Clock>(ctx: &AppContext<C>) -> Result<MutexGuard<'_, WorkSession<C>>, CommandError> {
    ctx.session.lock().map_err(|_| CommandError::Poisoned)
}

/// 当前 Unix 秒（落库时间戳来源；时钟异常严格报错）。
fn wall_now_secs() -> Result<i64, CommandError> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| CommandError::Clock)?
        .as_secs() as i64)
}

/// 落一个工作段：零秒段跳过（不写噪音行）；started_at = wall_now − 段秒。
fn persist_segment(storage: &Mutex<Storage>, segment: Duration) -> Result<(), CommandError> {
    if segment.is_zero() {
        return Ok(());
    }
    let started_at = wall_now_secs()? - segment.as_secs() as i64;
    storage
        .lock()
        .map_err(|_| CommandError::Poisoned)?
        .add_session(started_at, segment.as_secs() as i64)?;
    Ok(())
}

/// 开始新会话：仅 Idle 合法。
fn start_session<C: Clock>(ctx: &AppContext<C>) -> Result<(), CommandError> {
    lock(ctx)?.start()?;
    Ok(())
}

/// 暂停：本段时长 > 0 则落库。
fn pause_session<C: Clock>(ctx: &AppContext<C>) -> Result<(), CommandError> {
    let segment = lock(ctx)?.pause()?;
    persist_segment(&ctx.storage, segment)
}

/// 继续：仅 Paused 合法，累计被继承。
fn resume_session<C: Clock>(ctx: &AppContext<C>) -> Result<(), CommandError> {
    lock(ctx)?.resume()?;
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

/// 会话快照：当前状态 + 累计毫秒。
fn status_snapshot<C: Clock>(ctx: &AppContext<C>) -> Result<SessionStatus, CommandError> {
    let session = lock(ctx)?;
    Ok(SessionStatus {
        state: state_key(session.state()),
        total_ms: session.total().as_millis() as u64,
    })
}

/// 统计快照：三值聚合（统计按当前真实本地时间定界）。
fn stats_snapshot<C: Clock>(ctx: &AppContext<C>) -> Result<SessionStats, CommandError> {
    let storage = ctx.storage.lock().map_err(|_| CommandError::Poisoned)?;
    let now = Local::now();
    Ok(SessionStats {
        today_secs: storage.today_total(&now)?,
        week_secs: storage.week_total(&now)?,
        all_secs: storage.all_total()?,
    })
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

/// 查询会话快照（前端 100ms tick 拉取）。
#[tauri::command]
pub fn session_status(handle: State<'_, AppContext>) -> Result<SessionStatus, CommandError> {
    status_snapshot(&handle)
}

/// 查询今日/本周/累计统计（前端低频拉取）。
#[tauri::command]
pub fn session_stats(handle: State<'_, AppContext>) -> Result<SessionStats, CommandError> {
    stats_snapshot(&handle)
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::rc::Rc;
    use std::sync::Mutex;
    use std::time::Duration;

    use chrono::Local;

    use super::*;

    /// 手拨假钟（同 session.rs 思路）：clone 与 AppContext 共享同一时间轴。
    #[derive(Clone)]
    struct FakeClock {
        offset: Rc<Cell<Duration>>,
    }

    impl FakeClock {
        fn new() -> Self {
            Self {
                offset: Rc::new(Cell::new(Duration::ZERO)),
            }
        }

        fn advance(&self, d: Duration) {
            self.offset.set(self.offset.get() + d);
        }
    }

    impl Clock for FakeClock {
        fn now(&self) -> Duration {
            self.offset.get()
        }
    }

    /// 内存库 + 假钟的测试上下文。
    fn ctx(clock: FakeClock) -> AppContext<FakeClock> {
        AppContext {
            session: Mutex::new(WorkSession::new(clock)),
            storage: Mutex::new(Storage::open_in_memory().unwrap()),
        }
    }

    /// 秒数简写。
    fn secs(n: u64) -> Duration {
        Duration::from_secs(n)
    }

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
        assert_eq!(status_snapshot(&ctx).unwrap().state, "paused");
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
        let s = status_snapshot(&ctx).unwrap();
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
        assert_eq!(status_snapshot(&ctx).unwrap().state, "running");
    }

    /// T3：stats 三值聚合——按当前真实本地时间定界注入边界内/外记录。
    #[test]
    fn stats_forwarding_aggregates_periods() {
        let ctx = ctx(FakeClock::new());
        let now = Local::now();
        let today = crate::period::day_start_secs(&now);
        let week = crate::period::week_start_secs(&now);
        // 边界内注入：今日一条、本周早于今日一条、纪元一条（远早于本周）
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
