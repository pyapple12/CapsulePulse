//! Tauri 命令层：前端 invoke 的唯一入口，只做转发——状态机/提醒逻辑全在 core 各模块，本层零业务判断。
//! 命令核心逻辑抽为接收 `&AppContext` 的自由函数（脱离 tauri::State），T3 可在 cargo test 下
//! 无窗口直测（注入内存库 + 假钟）；`#[tauri::command]` 包装仅做 State 解包与副作用执行。
//! 提醒评估随 session_status 顺路执行（100ms tick 拉取架构）：评估结果与副作用（emit/通知）分离，
//! 前者可测、后者薄层。落库时间戳 started_at = wall_now − 段秒，SystemTime 只在本层出现。
//! 注意：自定义命令不经 ACL 白名单（系列实证），但通知/事件等能力仍需 capabilities 授权。

use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chrono::Local;
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_notification::NotificationExt;
use thiserror::Error;

use crate::reminder::{ReminderConfig, ReminderFire};
use crate::session::{Clock, RealClock, SessionError, SessionState, WorkSession};
use crate::settings::{ReminderSettings, SettingsError};
use crate::storage::Storage;

/// 应用级共享上下文：会话状态机 + 存储 + 提醒设置与触发状态。
/// lib.rs 经 `.manage()` 注册，各命令经 State 访问。泛型默认 [`RealClock`]；测试注入假钟。
/// rusqlite Connection 非 Sync，故 Storage 亦入 Mutex；锁序恒 session → storage/settings/fire 单向。
pub struct AppContext<C: Clock = RealClock> {
    /// 会话状态机。
    pub session: Mutex<WorkSession<C>>,
    /// 会话存储（运行时文件库，测试内存库）。
    pub storage: Mutex<Storage>,
    /// 提醒设置（内存态，持久化于 settings_path）。
    pub settings: Mutex<ReminderSettings>,
    /// 提醒段内触发状态（暂停/开始/重开时 clear）。
    pub fire: Mutex<ReminderFire>,
    /// 设置文件路径（set_settings 持久化目标）。
    pub settings_path: PathBuf,
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
    /// 设置层错误（载入/保存/校验失败）。
    #[error(transparent)]
    Settings(#[from] SettingsError),
    /// 会话锁中毒（此前持锁线程 panic）——严格报错，不静默续行。
    #[error("会话锁已中毒（此前持锁线程异常终止）")]
    Poisoned,
    /// 系统时钟早于 Unix 纪元（时间戳不可用）。
    #[error("系统时钟异常（早于 Unix 纪元）")]
    Clock,
    /// 事件发送失败（提醒通道不可用）。
    #[error("事件发送失败：{0}")]
    Event(String),
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

/// 提醒决策（评估纯逻辑的输出载体；emit/通知副作用由命令包装层执行）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReminderDecision {
    /// 是否应触发提醒。
    fire: bool,
    /// 当前阈值（分钟，供通知文案）。
    threshold_min: u32,
    /// 系统通知开关。
    notify_enabled: bool,
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

/// 开始新会话：仅 Idle 合法；提醒触发状态清零（新段）。
fn start_session<C: Clock>(ctx: &AppContext<C>) -> Result<(), CommandError> {
    lock(ctx)?.start()?;
    clear_reminder_fire(ctx);
    Ok(())
}

/// 暂停：本段时长 > 0 则落库；提醒触发状态清零（暂停即重置，用户定案）。
fn pause_session<C: Clock>(ctx: &AppContext<C>) -> Result<(), CommandError> {
    let segment = lock(ctx)?.pause()?;
    clear_reminder_fire(ctx);
    persist_segment(&ctx.storage, segment)
}

/// 继续：仅 Paused 合法，累计被继承；提醒触发状态清零（新段起算）。
fn resume_session<C: Clock>(ctx: &AppContext<C>) -> Result<(), CommandError> {
    lock(ctx)?.resume()?;
    clear_reminder_fire(ctx);
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

/// 清除提醒触发状态（暂停/开始/重开 = 新段；"暂停即重置"定案的接线点）。
fn clear_reminder_fire<C: Clock>(ctx: &AppContext<C>) {
    if let Ok(mut fire) = ctx.fire.lock() {
        fire.clear();
    }
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
        let settings = ctx.settings.lock().map_err(|_| CommandError::Poisoned)?;
        let config = ReminderConfig::from_minutes(settings.threshold_min);
        let should = ctx
            .fire
            .lock()
            .map_err(|_| CommandError::Poisoned)?
            .evaluate(segment, &config);
        ReminderDecision {
            fire: should,
            threshold_min: settings.threshold_min,
            notify_enabled: settings.notify_enabled,
        }
    };
    Ok((status, decision))
}

/// 提醒副作用：系统通知（notify_enabled 门控；失败降级仅声音——容错白名单）+ reminder-due 事件。
fn deliver_reminder(app: &AppHandle, decision: ReminderDecision) -> Result<(), CommandError> {
    if !decision.fire {
        return Ok(());
    }
    if decision.notify_enabled {
        send_notification(app, decision.threshold_min);
    }
    app.emit("reminder-due", decision.threshold_min)
        .map_err(|e| CommandError::Event(e.to_string()))
}

/// 系统通知（容错白名单：发送失败降级仅声音——错误落日志，不阻断事件）。
fn send_notification(app: &AppHandle, threshold_min: u32) -> bool {
    app.notification()
        .builder()
        .title("CapsulePulse")
        .body(format!("已连续工作 {threshold_min} 分钟，休息一下吧"))
        .show()
        .inspect_err(|err| eprintln!("系统通知发送失败（降级仅声音）：{err}"))
        .is_ok()
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

/// 设置读取。
fn get_settings_inner<C: Clock>(ctx: &AppContext<C>) -> Result<ReminderSettings, CommandError> {
    Ok(ctx
        .settings
        .lock()
        .map_err(|_| CommandError::Poisoned)?
        .clone())
}

/// 设置保存：校验 → 持久化 → 更新内存态（任一步失败不污染内存态）。
fn set_settings_inner<C: Clock>(
    ctx: &AppContext<C>,
    settings: &ReminderSettings,
) -> Result<(), CommandError> {
    settings.save(&ctx.settings_path)?;
    let mut current = ctx.settings.lock().map_err(|_| CommandError::Poisoned)?;
    *current = settings.clone();
    Ok(())
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
    deliver_reminder(&app, decision)?;
    Ok(status)
}

/// 查询今日/本周/累计统计（前端低频拉取）。
#[tauri::command]
pub fn session_stats(handle: State<'_, AppContext>) -> Result<SessionStats, CommandError> {
    stats_snapshot(&handle)
}

/// 读取提醒设置。
#[tauri::command]
pub fn get_settings(handle: State<'_, AppContext>) -> Result<ReminderSettings, CommandError> {
    get_settings_inner(&handle)
}

/// 保存提醒设置（校验 → 持久化 → 更新内存态，即时生效）。
#[tauri::command]
pub fn set_settings(
    handle: State<'_, AppContext>,
    settings: ReminderSettings,
) -> Result<(), CommandError> {
    set_settings_inner(&handle, &settings)
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::path::PathBuf;
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

    /// 测试设置文件路径（临时目录，禁触真实用户数据）。
    fn temp_settings_path(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("capsule-pulse-cmd-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(format!("{tag}.json"))
    }

    /// 内存库 + 假钟的测试上下文。
    fn ctx(clock: FakeClock) -> AppContext<FakeClock> {
        AppContext {
            session: Mutex::new(WorkSession::new(clock)),
            storage: Mutex::new(Storage::open_in_memory().unwrap()),
            settings: Mutex::new(ReminderSettings::default()),
            fire: Mutex::new(ReminderFire::default()),
            settings_path: temp_settings_path("ctx"),
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

    /// T4a：set_settings 校验 + 持久化 + 内存态更新；非法阈值被拒。
    #[test]
    fn settings_set_persists_and_validates() {
        let ctx = ctx(FakeClock::new());
        let s = ReminderSettings {
            threshold_min: 30,
            sound_enabled: false,
            notify_enabled: true,
        };
        set_settings_inner(&ctx, &s).unwrap();
        assert_eq!(get_settings_inner(&ctx).unwrap(), s);
        assert!(std::fs::read_to_string(&ctx.settings_path)
            .unwrap()
            .contains("\"threshold_min\": 30"));

        let bad = ReminderSettings {
            threshold_min: 0,
            sound_enabled: true,
            notify_enabled: true,
        };
        assert!(matches!(
            set_settings_inner(&ctx, &bad),
            Err(CommandError::Settings(SettingsError::InvalidThreshold(0)))
        ));
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
}
