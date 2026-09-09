//! Tauri 命令层：前端 invoke 的唯一入口——按职责拆分（会话/统计/提醒设置），本层只做转发与副作用。
//! `AppContext`、`CommandError` 与共享助手集中在本模块；`#[tauri::command]` 包装在各子模块，
//! 经 `pub use` 再导出给 lib.rs。命令核心逻辑抽为接收 `&AppContext` 的自由函数，
//! T3 可在 cargo test 下无窗口直测（注入内存库 + 假钟，见 test_support）。
//! 落库时间戳 started_at = wall_now − 段秒，SystemTime 只在本层出现，session.rs 纯度不破。
//! 注意：自定义命令不经 ACL 白名单（系列实证），但通知/事件等能力仍需 capabilities 授权。

pub mod reminder;
pub mod session;
pub mod stats;
pub mod workday;

use std::path::PathBuf;
use std::sync::{LockResult, Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use thiserror::Error;

use crate::reminder::ReminderFire;
use crate::session::{Clock, RealClock, WorkSession};
use crate::settings::ReminderSettings;
use crate::storage::Storage;
use crate::workday::WorkdayState;

/// 应用级共享上下文：工作日状态 + 会话状态机 + 存储 + 提醒设置与触发状态。
/// lib.rs 经 `.manage()` 注册，各命令经 State 访问。泛型默认 [`RealClock`]；测试注入假钟。
/// rusqlite Connection 非 Sync，故 Storage 亦入 Mutex；
/// 锁序恒 workday → session → storage/settings/fire 单向（禁反向嵌套，防死锁）。
pub struct AppContext<C: Clock = RealClock> {
    /// 工作日状态机（Off/OnDuty，PL005）。
    pub workday: Mutex<WorkdayState>,
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
    Session(#[from] crate::session::SessionError),
    /// 工作日状态机拒绝（重复上班/未上班下班），文案沿用 workday.rs。
    #[error(transparent)]
    Workday(#[from] crate::workday::WorkdayError),
    /// 存储层错误（落库/聚合失败）。
    #[error(transparent)]
    Storage(#[from] crate::storage::StorageError),
    /// 设置层错误（载入/保存/校验失败）。
    #[error(transparent)]
    Settings(#[from] crate::settings::SettingsError),
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

/// 锁结果统一收敛：Mutex 中毒（此前持锁线程 panic）严格报错（Poisoned），不静默续行。
fn poison<T>(lock: LockResult<T>) -> Result<T, CommandError> {
    lock.map_err(|_| CommandError::Poisoned)
}

/// 取会话锁：中毒严格报错（Poisoned），不 unwrap 不吞错。
fn lock<C: Clock>(ctx: &AppContext<C>) -> Result<MutexGuard<'_, WorkSession<C>>, CommandError> {
    poison(ctx.session.lock())
}

/// 当前 Unix 秒（落库时间戳来源；时钟异常严格报错）。
pub(super) fn wall_now_secs() -> Result<i64, CommandError> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| CommandError::Clock)?
        .as_secs() as i64)
}

#[cfg(test)]
pub(crate) mod test_support {
    //! 跨子模块测试共享：假钟 + 内存库上下文（仅测试编译期存在）。

    use std::cell::Cell;
    use std::path::PathBuf;
    use std::rc::Rc;
    use std::sync::Mutex;
    use std::time::Duration;

    use super::*;
    use crate::settings::ReminderSettings;
    use crate::workday::WorkdayState;

    /// 手拨假钟：clone 与 AppContext 共享同一时间轴。
    #[derive(Clone)]
    pub struct FakeClock {
        offset: Rc<Cell<Duration>>,
    }

    impl FakeClock {
        pub fn new() -> Self {
            Self {
                offset: Rc::new(Cell::new(Duration::ZERO)),
            }
        }

        pub fn advance(&self, d: Duration) {
            self.offset.set(self.offset.get() + d);
        }
    }

    impl Clock for FakeClock {
        fn now(&self) -> Duration {
            self.offset.get()
        }
    }

    /// 内存库 + 假钟的测试上下文。
    pub fn ctx(clock: FakeClock) -> AppContext<FakeClock> {
        AppContext {
            workday: Mutex::new(WorkdayState::Off),
            session: Mutex::new(WorkSession::new(clock)),
            storage: Mutex::new(Storage::open_in_memory().unwrap()),
            settings: Mutex::new(ReminderSettings::default()),
            fire: Mutex::new(ReminderFire::default()),
            settings_path: temp_settings_path("ctx"),
        }
    }

    /// 测试前置：以上班时刻 1_000 打卡（计时门禁通过的前提；既有用例的时间锚不变）。
    pub fn begin_duty<C: Clock>(ctx: &AppContext<C>) {
        super::workday::clock_in_inner(ctx, 1_000).unwrap();
    }

    /// 秒数简写。
    pub fn secs(n: u64) -> Duration {
        Duration::from_secs(n)
    }

    /// 测试设置文件路径（临时目录，禁触真实用户数据）。
    pub fn temp_settings_path(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("capsule-pulse-cmd-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(format!("{tag}.json"))
    }
}
