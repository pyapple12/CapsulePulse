//! Tauri 命令层：前端 invoke 的唯一入口——按职责拆分（会话/统计/提醒设置），本层只做转发与副作用。
//! `AppContext`、`CommandError` 与共享助手集中在本模块；`#[tauri::command]` 包装在各子模块，
//! 经 `pub use` 再导出给 lib.rs。命令核心逻辑抽为接收 `&AppContext` 的自由函数，
//! T3 可在 cargo test 下无窗口直测（注入内存库 + 假钟，见 test_support）。
//! 落库时间戳 started_at = wall_now − 段秒，SystemTime 只在本层出现，session.rs 纯度不破。
//! 注意：自定义命令不经 ACL 白名单（系列实证），但通知/事件等能力仍需 capabilities 授权。

pub mod reminder;
pub mod session;
pub mod stats;
pub mod window;
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

/// 自动下班基准与代次（PL025.8）：当前在岗班的到点时刻（`clock_in + N`；加班改为 `clock_in + 12h`）。
/// 代次每次写入 +1——兜底线程据它判断"基准是否已换代"（换代即让位退出，由新线程接手）；
/// 待决提示标志使到点前 60s 只提示一次（基准变更时清零）。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct AutoOutWatch {
    /// 到点时刻（Unix 秒）；`None` = 无在岗班 / 已下班。
    pub due_at: Option<i64>,
    /// 代次（每次基准写入 +1）。
    pub generation: u64,
    /// 待决提示是否已发（到点前 60s 只提示一次）。
    pub pending_notified: bool,
}

/// 墙钟抽象（PL027.6）：命令层取"当下 Unix 秒"的统一入口——生产用 [`SystemWallClock`]，
/// 测试注入与假单调钟同步的假墙钟（"时间源可注入"纪律的墙钟面；段时长由事件时间戳推出后，
/// 墙钟可控是既有用例保住强度的前提）。
pub trait WallClock: Send + Sync {
    /// 当前 Unix 秒。
    fn now_secs(&self) -> Result<i64, CommandError>;
}

/// 系统墙钟（生产实现）：`SystemTime` 直取。
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemWallClock;

impl WallClock for SystemWallClock {
    fn now_secs(&self) -> Result<i64, CommandError> {
        wall_now_secs()
    }
}

/// 取当下 Unix 秒（经注入墙钟）：命令实现统一走它，勿直取系统时钟。
pub(crate) fn now_secs<C: Clock>(ctx: &AppContext<C>) -> Result<i64, CommandError> {
    ctx.wall.now_secs()
}

/// 应用级共享上下文：工作日状态 + 会话状态机 + 存储 + 提醒设置与触发状态。
/// lib.rs 经 `.manage()` 注册，各命令经 State 访问。泛型默认 [`RealClock`]；测试注入假钟。
/// rusqlite Connection 非 Sync，故 Storage 亦入 Mutex；
/// 锁序恒 workday → session → storage/settings/fire → auto_out 单向（禁反向嵌套，防死锁）。
/// 线程现状（FIX002.8）：写命令同步（主线程串行），四个读命令（status/stats/day/week）已 async 化——
/// 全面 async 化后本锁序纪律即实际承压面，新增命令必须遵守。
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
    /// 自动下班基准与兜底线程代次（PL025.8；锁序末位取用）。
    pub auto_out: Mutex<AutoOutWatch>,
    /// 设置文件路径（set_settings 持久化目标）。
    pub settings_path: PathBuf,
    /// 墙钟（PL027.6 注入点）：生产 = 系统时钟，测试 = 假墙钟。
    pub wall: Box<dyn WallClock>,
    /// 一次性启动恢复提示（PL027.8）：仅"判下班"分支有值；前端取一次即清空。
    pub recovery: Mutex<Option<workday::RecoveryOutcome>>,
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
    /// 在岗期间禁止修改自动下班小时数（PL025.5：本班基准不可中途改，账目口径唯一）。
    #[error("在岗期间不可修改自动下班小时数（本班按 {0} 小时计）")]
    AutoOutLockedOnDuty(u32),
    /// 已到加班上限（PL025.9：拉满 12h 后只能下班）。
    #[error("已到加班上限（{0} 小时），请下班")]
    AlreadyAtOvertimeCap(u32),
    /// 锁中毒（此前持锁线程 panic）——携带锁名严格报错，不静默续行（y.problems#5②：文案如实反映是哪把锁）。
    #[error("{0}锁已中毒（此前持锁线程异常终止）")]
    Poisoned(&'static str),
    /// 系统时钟早于 Unix 纪元（时间戳不可用）。
    #[error("系统时钟异常（早于 Unix 纪元）")]
    Clock,
    /// 单日明细的日期偏移越界（合法范围 ±366 日）。
    #[error("日期偏移非法：{0}（应为 -366–366）")]
    InvalidOffset(i64),
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

/// 锁结果统一收敛：Mutex 中毒（此前持锁线程 panic）严格报错（Poisoned 带锁名），不静默续行。
fn poison<T>(lock_name: &'static str, lock: LockResult<T>) -> Result<T, CommandError> {
    lock.map_err(|_| CommandError::Poisoned(lock_name))
}

/// 取会话锁：中毒严格报错（Poisoned 带锁名），不 unwrap 不吞错。
fn lock<C: Clock>(ctx: &AppContext<C>) -> Result<MutexGuard<'_, WorkSession<C>>, CommandError> {
    poison("会话", ctx.session.lock())
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

    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use super::*;
    use crate::settings::ReminderSettings;
    use crate::workday::WorkdayState;

    /// 手拨假钟（PL027.6：偏移改原子共享）：`Arc<AtomicU64>` 毫秒偏移——与假墙钟共享同一
    /// 时间轴，且满足 `Box<dyn WallClock>` 的 Send + Sync 约束；clone 后仍同轴。
    #[derive(Clone)]
    pub struct FakeClock {
        millis: Arc<AtomicU64>,
    }

    impl FakeClock {
        /// 新建零点假钟（时间源注入测试用）。
        pub fn new() -> Self {
            Self {
                millis: Arc::new(AtomicU64::new(0)),
            }
        }

        /// 前拨假钟推进时间轴。
        pub fn advance(&self, d: Duration) {
            self.millis
                .fetch_add(d.as_millis() as u64, Ordering::SeqCst);
        }

        /// 共享毫秒句柄（假墙钟与假单调钟同轴，镜像生产"两者同刻"）。
        pub fn shared_millis(&self) -> Arc<AtomicU64> {
            self.millis.clone()
        }
    }

    impl Clock for FakeClock {
        fn now(&self) -> Duration {
            Duration::from_millis(self.millis.load(Ordering::SeqCst))
        }
    }

    /// 假墙钟（PL027.6）：`base + 假钟偏移`——与假单调钟共享同一原子时间轴，
    /// 故"假钟前拨 N 秒"事件时间戳同步前拨 N 秒；`base` 取 1_000 与既有夹具打卡锚对齐。
    #[derive(Clone)]
    pub struct FakeWall {
        base: i64,
        millis: Arc<AtomicU64>,
    }

    impl WallClock for FakeWall {
        fn now_secs(&self) -> Result<i64, CommandError> {
            Ok(self.base + (self.millis.load(Ordering::SeqCst) / 1_000) as i64)
        }
    }

    /// 内存库 + 假钟的测试上下文。
    pub fn ctx(clock: FakeClock) -> AppContext<FakeClock> {
        AppContext {
            workday: Mutex::new(WorkdayState::Off),
            session: Mutex::new(WorkSession::new(clock.clone())),
            storage: Mutex::new(Storage::open_in_memory().unwrap()),
            settings: Mutex::new(ReminderSettings::default()),
            fire: Mutex::new(ReminderFire::default()),
            auto_out: Mutex::new(AutoOutWatch::default()),
            settings_path: temp_settings_path("ctx"),
            wall: Box::new(FakeWall {
                base: 1_000,
                millis: clock.shared_millis(),
            }),
            recovery: Mutex::new(None),
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
