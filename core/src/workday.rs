//! 工作日/打卡纯逻辑层（PL005）：WorkdayState 状态机（Off/OnDuty）、EventKind 事件类型、
//! 事件归约纯函数 reduce_day（events → 在岗/工作/休息 + 时间图谱区块）。
//! 设计要点：① 状态机与归约零 IO、时间戳全用 Unix 秒入参注入——跨零点/回填等边界
//! 在 cargo test 下直测，禁真实等待（AGENTS 陷阱清单）；② OnDuty **只携带 `clock_in_at`**
//! （FIX008.19 照实：**PL027 起不再携带库行 id**——存储改 `events` 单表后不存在"班行"可关，
//! 见 [`WorkdayState`]）；
//! ③ reduce_day 输出结构直接 derive Serialize 作为 IPC DTO 单一来源（serde 非 tauri，纯度不破）。
//! 关联：storage.rs 的 **`events` 单表（唯一事实源；旧 `workdays`/`sessions` 两表 PL027 已退役）** + `runtime`/`days` 缓存层，commands/workday.rs（接线）。

use serde::Serialize;
use thiserror::Error;

/// 事件类型（events.kind TEXT 列的编解码单一来源；as_str/parse 由存储层调用）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventKind {
    /// 上班打卡。
    ClockIn,
    /// 手动下班打卡。
    ClockOut,
    /// 自动下班（满 N 小时回填记账）。
    AutoClockOut,
    /// 计时段开始（开始/继续按钮）。
    SegmentStart,
    /// 计时段结束（暂停按钮/下班收段）。
    SegmentEnd,
    /// 加班（PL025.9）：把本班下班基准拉满到 `clock_in + 12h`；只作留痕，不参与归约。
    Overtime,
    /// 重置标记（PL027.5）：重置按钮 = 收段落库 + 大计时器归零 + 回休息态；只作留痕，
    /// 不参与归约——"大计时器累计"的基准 = 最后一条本事件或本次打卡，使累计可由流水推出。
    SessionReset,
    /// 崩溃补下班（PL027.4）：重启恢复判定"已断"时按最后心跳补记的下班；只作留痕，
    /// 不参与归约（与 `ClockOut`/`AutoClockOut` 同性质，仅用于区分"这次下班是补记的"）。
    CrashClockOut,
}

impl EventKind {
    /// 存储序列化文本。
    pub fn as_str(&self) -> &'static str {
        match self {
            EventKind::ClockIn => "clock_in",
            EventKind::ClockOut => "clock_out",
            EventKind::AutoClockOut => "auto_clock_out",
            EventKind::SegmentStart => "segment_start",
            EventKind::SegmentEnd => "segment_end",
            EventKind::Overtime => "overtime",
            EventKind::SessionReset => "session_reset",
            EventKind::CrashClockOut => "crash_clock_out",
        }
    }

    /// 自存储文本解析；未知值返回 None（存储层据此严格报错）。
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "clock_in" => Some(EventKind::ClockIn),
            "clock_out" => Some(EventKind::ClockOut),
            "auto_clock_out" => Some(EventKind::AutoClockOut),
            "segment_start" => Some(EventKind::SegmentStart),
            "segment_end" => Some(EventKind::SegmentEnd),
            "overtime" => Some(EventKind::Overtime),
            "session_reset" => Some(EventKind::SessionReset),
            "crash_clock_out" => Some(EventKind::CrashClockOut),
            _ => None,
        }
    }
}

/// 加班上限（小时）：与自动下班小时的可配上限（设置板 4–12）同值，即"拉满到硬上限"（PL025.9）。
pub const OVERTIME_CAP_HOURS: u32 = 12;

/// 自动下班小时数的**可配下限**（小时，FIX006.16）：与 [`OVERTIME_CAP_HOURS`] 一起构成设置校验
/// 区间（`settings::ReminderSettings::validate` 与设置板步进器**同源**）；它是**产品下限**
/// （不是技术硬顶），前端经 `ui/types.ts` 镜像（跨进程不能 import）。
pub const AUTO_OUT_MIN_HOURS: u32 = 4;

/// 工作日状态机错误：非法转移严格报错（AGENTS 错误策略主线），不静默忽略。
#[derive(Debug, Error, PartialEq, Eq)]
pub enum WorkdayError {
    /// 已在岗中再次上班（重复打卡）。
    #[error("已在岗中（不可重复上班）")]
    AlreadyOnDuty,
    /// 未上班却执行下班相关操作。
    #[error("未上班（需先上班打卡）")]
    NotOnDuty,
}

/// 工作日状态机：Off（未上班）/ OnDuty（在岗中）。
/// OnDuty 只携带 clock_in_at（自动下班回填基准 + 与流水交叉校验）——**PL027 起不再携带库行 id**：
/// 存储改为 events 单表后不存在"班行"可关，下班只追加事件，状态无需行句柄。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WorkdayState {
    /// 未上班。
    #[default]
    Off,
    /// 在岗中：clock_in_at = 上班时刻。
    OnDuty {
        /// 上班打卡时刻（Unix 秒）。
        clock_in_at: i64,
    },
}

impl WorkdayState {
    /// 上班转移：仅 Off 合法。
    /// # 错误
    /// 已在岗返回 [`WorkdayError::AlreadyOnDuty`]。
    pub fn clock_in(&mut self, at: i64) -> Result<(), WorkdayError> {
        if self.is_on_duty() {
            return Err(WorkdayError::AlreadyOnDuty);
        }
        *self = WorkdayState::OnDuty { clock_in_at: at };
        Ok(())
    }

    /// 下班转移：仅 OnDuty 合法（下班只追加事件，无行句柄可读）。
    /// # 错误
    /// 未上班返回 [`WorkdayError::NotOnDuty`]。
    pub fn clock_out(&mut self) -> Result<(), WorkdayError> {
        if !self.is_on_duty() {
            return Err(WorkdayError::NotOnDuty);
        }
        *self = WorkdayState::Off;
        Ok(())
    }

    /// 是否在岗中（计时门禁与启动恢复判断用）。
    pub fn is_on_duty(&self) -> bool {
        matches!(self, WorkdayState::OnDuty { .. })
    }

    /// 在岗打卡时刻（未在岗 None）——自动下班基准重建 / 心跳取数用。
    pub fn clock_in_at(&self) -> Option<i64> {
        match self {
            WorkdayState::OnDuty { clock_in_at } => Some(*clock_in_at),
            WorkdayState::Off => None,
        }
    }

    /// 由流水重建在岗态（PL027.4 启动恢复"继续"分支）：入参 = 未配对打卡时刻。
    pub fn restore_on_duty(clock_in_at: i64) -> Self {
        WorkdayState::OnDuty { clock_in_at }
    }
}

/// 自动下班到点时刻：上班 + N 小时（"账目准时"基准的单一算法；PL025.8 打卡武装与加班拉满共用）。
pub fn auto_out_at(clock_in_at: i64, hours: u32) -> i64 {
    clock_in_at + i64::from(hours) * 3_600
}

/// 每次加班的步进（小时，PATCH005.3）：基准之上每点一次「加班」+4h，硬顶 [`OVERTIME_CAP_HOURS`]。
pub const OVERTIME_STEP_HOURS: u32 = 4;

/// 今日上限（小时，PATCH005.3 纯函数）：`基准 + 4h × 加班步数`，封顶 12h。
/// 基准 = 当天第一次打卡时的自动下班小时数——因"整日锁定"当天内不可能变，故直接取设置值
/// （不需要快照事件）。
pub fn day_budget_hours(basis_hours: u32, overtime_steps: u32) -> u32 {
    (basis_hours + OVERTIME_STEP_HOURS * overtime_steps).min(OVERTIME_CAP_HOURS)
}

/// 今日已累计在岗（秒，PATCH005.3 纯函数）：Σ 当日在岗窗口——班间空档不在窗口内、天然不计；
/// 班内休息在窗口内，按"在岗"计入（用户定案：预算按在岗算，不是工作时长）。
pub fn day_used_secs(windows: &[(i64, i64)]) -> i64 {
    windows
        .iter()
        .map(|(start, end)| (end - start).max(0))
        .sum()
}

/// 本班剩余预算（秒，PATCH005.3 纯函数）：今日上限 − 本班打卡前的今日已累计，不为负。
pub fn shift_remaining_secs(budget_hours: u32, used_before_shift_secs: i64) -> i64 {
    (i64::from(budget_hours) * 3_600 - used_before_shift_secs).max(0)
}

/// 本班到点时刻（PATCH005.3 纯函数）：本班打卡 + 本班剩余——自动下班武装与前端"轴末"同一口径。
pub fn shift_deadline(clock_in_at: i64, remaining_secs: i64) -> i64 {
    clock_in_at + remaining_secs
}

/// 加班裁决（PATCH005.3 纯函数）：到顶（上限已 12h，+4 无意义）或跨日（now 的日历日 ≠ 打卡日）
/// → 拒（确认框只给「下班」）；否则允许。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OvertimeDecision {
    /// 允许加班（+4h，封顶 12h）。
    Allow,
    /// 已达 12h 顶：加班无意义。
    AtCap,
    /// 已跨日（零点后）：本数据日不再给加班。
    AcrossMidnight,
}

/// 见 [`OvertimeDecision`]：入参为两个**日历日零点**（now 所属 / 本班打卡所属）与今日上限小时。
pub fn overtime_decision(
    now_day_start: i64,
    shift_day_start: i64,
    budget_hours: u32,
) -> OvertimeDecision {
    if now_day_start != shift_day_start {
        OvertimeDecision::AcrossMidnight
    } else if budget_hours >= OVERTIME_CAP_HOURS {
        OvertimeDecision::AtCap
    } else {
        OvertimeDecision::Allow
    }
}

/// 兜底线程的一步决策（PL025.8 纯逻辑；线程只按它行动，故可脱离线程直测）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoOutStep {
    /// 无在岗基准（已下班/未武装）：线程退出。
    Stop,
    /// 基准已换代（加班/下班/重新打卡）：旧线程让位退出，由新线程接手。
    Yield,
    /// 未到点：睡 `secs` 秒后再看（上限 60s——基准变更最多 1 分钟内被感知，到点前精确唤醒）。
    Wait(u64),
    /// 到点：以 `at`（= 基准本身，账目准时）执行自动下班。
    Fire(i64),
}

/// 兜底线程决策纯函数：按基准与当前时刻决定下一步。
/// # 参数
/// due_at / current_generation 为锁内读到的当前基准与代次；generation 为线程启动时代次；
/// now 为当前 Unix 秒。
pub fn auto_out_step(
    due_at: Option<i64>,
    generation: u64,
    current_generation: u64,
    now: i64,
) -> AutoOutStep {
    if generation != current_generation {
        return AutoOutStep::Yield;
    }
    let Some(due) = due_at else {
        return AutoOutStep::Stop;
    };
    if now >= due {
        return AutoOutStep::Fire(due);
    }
    AutoOutStep::Wait(((due - now) as u64).min(60))
}

/// 自动下班待决窗口（秒）：到点前 60s 内弹确认框（PL025.8）。
pub const AUTO_OUT_PENDING_SECS: i64 = 60;

/// 崩溃恢复宽限（秒，PL027.4）：停机 ≤ 此值视为"没断"（无缝续），> 此值视为"断了"（按最后心跳补下班）。
pub const CRASH_GRACE_SECS: i64 = 600;

/// 启动恢复判定入参（运行态缓存的只读投影；纯逻辑层不依赖存储类型）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecoveryInput {
    /// 最后心跳时刻（"最后已知活着"的下界，误差 ≤ 心跳间隔）。
    pub last_beat_at: i64,
    /// 在岗打卡时刻（段起点与自动下班基准的锚）。
    pub duty_clock_in_at: i64,
    /// 崩溃时是否在工作态（决定收口时是否需要补一条段末事件）。
    pub working: bool,
    /// 工作段起点（休息态 None）。
    pub segment_start_at: Option<i64>,
    /// 断开前的累计秒数。
    pub accumulated_secs: i64,
    /// 自动下班基准（未武装 None；含加班重算后的值）。
    pub auto_out_at: Option<i64>,
}

/// 启动恢复判定结果（PL027.4）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryDecision {
    /// 无需处理（无未闭合打卡 / 无缓存）：交给常规启动路径。
    Nothing,
    /// 停机 ≤ 宽限：**无缝续**——恢复在岗与会话（停机时长计入当前段），不写任何流水。
    Continue {
        /// 在岗打卡时刻。
        duty_clock_in_at: i64,
        /// 当前是否工作态。
        working: bool,
        /// 工作段起点（休息态 None）。
        segment_start_at: Option<i64>,
        /// 断开前累计秒数。
        accumulated_secs: i64,
    },
    /// 停机 > 宽限：**按最后心跳补下班**。
    CloseAt {
        /// 记账时刻 = min(最后心跳, 自动下班点)。
        at: i64,
        /// 是否以"自动下班"名义记账（已越过自动下班点）。
        auto: bool,
        /// 是否在补下班前补一条段末事件（崩溃时在工作态）。
        close_segment: bool,
    },
}

/// 启动恢复判定（PL027.4 纯函数，时间可注入）：三分支见 [`RecoveryDecision`]。
/// 口径：停机 = `now − last_beat_at`；收口时刻 = `min(last_beat_at, auto_out_at)`
/// （越过自动下班点即用该点，避免虚记加班——发现可迟到、账目准时）。
pub fn decide_recovery(input: Option<&RecoveryInput>, now: i64) -> RecoveryDecision {
    let Some(input) = input else {
        return RecoveryDecision::Nothing;
    };
    if now - input.last_beat_at <= CRASH_GRACE_SECS {
        return RecoveryDecision::Continue {
            duty_clock_in_at: input.duty_clock_in_at,
            working: input.working,
            segment_start_at: input.segment_start_at,
            accumulated_secs: input.accumulated_secs,
        };
    }
    let cap = input.auto_out_at.unwrap_or(input.last_beat_at);
    let at = input.last_beat_at.min(cap);
    RecoveryDecision::CloseAt {
        at,
        auto: input.auto_out_at.is_some() && at == cap && cap < input.last_beat_at,
        close_segment: input.working,
    }
}

/// 脏数据归一化（PL027.9）：检出"前一条打卡未闭合又出现新打卡"（历史/异常路径遗留的重叠班），
/// 返回**需要追加的纠正事件**——把前一条班视为在**新打卡前一秒**结束：若此刻仍有未闭合工作段，
/// 先补一条 `segment_end`，再补一条 `clock_out`（同一时刻，顺序即此）。
/// 用"前一秒"而非"同刻"是**幂等必需**：同刻事件按 id 排序，追加的纠正会排在触发它的打卡之后，
/// 重扫时依旧看到嵌套 → 永远不收敛；前移一秒后重扫即见"先关后开"，零新增。
/// 纯函数：入参为按时间升序的全部流水，出参为按顺序追加的纠正事件（可能为空）。
pub fn nested_clock_in_fixes(events: &[(i64, EventKind)]) -> Vec<(i64, EventKind)> {
    let mut fixes = Vec::new();
    let mut duty_open = false;
    let mut work_open = false;
    for &(at, kind) in events {
        match kind {
            EventKind::ClockIn => {
                if duty_open {
                    let close_at = at - 1;
                    if work_open {
                        fixes.push((close_at, EventKind::SegmentEnd));
                        work_open = false;
                    }
                    fixes.push((close_at, EventKind::ClockOut));
                }
                duty_open = true;
            }
            EventKind::ClockOut | EventKind::AutoClockOut | EventKind::CrashClockOut => {
                duty_open = false;
                work_open = false;
            }
            EventKind::SegmentStart => work_open = true,
            EventKind::SegmentEnd => work_open = false,
            _ => {}
        }
    }
    fixes
}

/// 待决判定纯函数：`now` 落在 `[due − 60, due)` 内即待决（返回到点时刻）；其余 None。
pub fn auto_out_pending(due_at: Option<i64>, now: i64) -> Option<i64> {
    let due = due_at?;
    (now >= due - AUTO_OUT_PENDING_SECS && now < due).then_some(due)
}

/// 由流水推"某个数据日的在岗窗口"（PL027.6 单一事实源）：`clock_in` 开窗，其后最近的
/// 下班事件（`clock_out` / `auto_clock_out` / `crash_clock_out`）关窗；**前班未关又出现新打卡
/// 时视为前班在该刻结束**（脏数据兜底，归一化见 PL027.9）；末班未关则闭于 `now`（在岗中）。
/// 只收 `clock_in ∈ [day_start, day_end)` 的窗口（数据日 = 打卡日，跨零点不切分）。
/// # 参数
/// events 为本数据日及其后紧邻区间的流水（升序）；now 为当前 Unix 秒。
/// # 返回
/// (窗口列表（升序、半开）、末窗是否仍未关（在岗中且属于本数据日）)
pub fn duty_windows_for_day(
    events: &[(i64, EventKind)],
    day_start: i64,
    day_end: i64,
    now: i64,
) -> (Vec<(i64, i64)>, bool) {
    let mut windows: Vec<(i64, i64)> = Vec::new();
    let mut open: Option<i64> = None;
    for &(at, kind) in events {
        match kind {
            EventKind::ClockIn => {
                if let Some(start) = open.replace(at) {
                    if at > start {
                        windows.push((start, at));
                    }
                }
            }
            EventKind::ClockOut | EventKind::AutoClockOut | EventKind::CrashClockOut => {
                if let Some(start) = open.take() {
                    if at > start {
                        windows.push((start, at));
                    }
                }
            }
            _ => {}
        }
    }
    let mut open_tail = false;
    // 在岗末窗闭于 `now`；**同秒也保底 1 秒**（PATCH004.1）：打卡当秒若按零长丢窗，当日摘要
    // 就只剩上一班，"当前班起点 = 末个班间空档之后第一块（无空档则当天首块）"的推法会退回
    // 当天首块 → 表盘底轨蓝条短暂锚到上一班起点（被上一班色块盖住后恰好从块末露出），
    // 下一秒窗口非零才自纠。保底 1 秒让新班当秒即产块，锚点从第一帧就是打卡刻。
    if let Some(start) = open {
        windows.push((start, now.max(start + 1)));
        open_tail = start >= day_start && start < day_end;
    }
    let day_windows = windows
        .into_iter()
        .filter(|(start, _)| *start >= day_start && *start < day_end)
        .collect();
    (day_windows, open_tail)
}

/// 图谱区块类型：工作（计时段）/ 休息（在岗内空隙）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum BlockKind {
    /// 工作块。
    Work,
    /// 休息块。
    Rest,
}

/// 图谱区块：[start, end) Unix 秒区间（serde 结构单一来源，TS 侧镜像于 ui/types.ts）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct DayBlock {
    /// 区块起点（Unix 秒）。
    pub start: i64,
    /// 区块终点（Unix 秒，不含）。
    pub end: i64,
    /// 工作 / 休息。
    pub kind: BlockKind,
}

/// 单日归约汇总：三值（在岗/工作/休息秒数）+ 在岗起止 + 时间图谱区块。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DaySummary {
    /// 在岗起点（本日切片内，Unix 秒；无在岗为 None）。
    pub duty_started_at: Option<i64>,
    /// 在岗终点（Unix 秒；仍在岗中为 None）。
    pub duty_ended_at: Option<i64>,
    /// 在岗总秒数。
    pub duty_secs: i64,
    /// 工作总秒数（在岗内计时段之和）。
    pub work_secs: i64,
    /// 休息总秒数（在岗 − 工作）。
    pub rest_secs: i64,
    /// 时间图谱区块（按时间序；**同一在岗窗口内 Work/Rest 交替**——一日多班时班间空档不入列，故可出现 `Work→Work` 相邻）。
    pub blocks: Vec<DayBlock>,
    /// 今日上限小时数（PATCH005.3：基准 + 4h×加班步数、封顶 12h）——**仅今日有意义**，过去日为 0。
    pub budget_hours: u32,
    /// 今日已累计在岗秒数（PATCH005.3）——前端据此算"今日剩余"；过去日为 0。
    pub used_secs: i64,
    /// 本班剩余预算秒数（PATCH005.3：上限 − 本班前已累计）——表盘底轨跨度与"轴末"同一口径；
    /// 未在岗（暂下班）与过去日为 0（那时不画蓝条、轴末取末段末尾）。
    pub remaining_secs: i64,
    /// **当前班起点**（FIX006.4：最后一个班间空档之后的第一块起点 = 末个在岗窗口起点，Unix 秒）；
    /// 未在岗（末窗已闭合）为 None。表盘底轨与电池共用同一事实源（原前端两处各推一份）。
    pub shift_start: Option<i64>,
    /// **今日可加班**（FIX006.4：在岗 && 未到 12h 硬顶 && 未跨日）；其余情形 false。
    /// 自动下班确认框的「加班」钮可见性据此（原前端自算"到顶 + 跨日"）。
    pub can_overtime: bool,
    /// **今日已到 12h 硬顶**（FIX006.4：`used_secs >= OVERTIME_CAP_HOURS × 3600`）；其余情形 false。
    /// 上班钮置灰据此（原前端自算）。
    pub cap_reached: bool,
}

/// 事件归约纯函数（PL025.6 窗口口径）：单日数据日的在岗窗口 + 事件流 → 三值 + 图谱区块
/// （PL005 图表与三值唯一取数逻辑）。规则：**在岗窗口由调用方给定**——数据日 = 上班打卡日，
/// 窗口由 `duty_windows_for_day` **从流水推出**（该数据日 `clock_in` 开窗、下班事件关窗；未关的末窗闭于 now；可多段 =
/// 同日多班；跨零点尾巴随窗口天然带入，**不再按自然日切分**）；`segment_start → segment_end`
/// 记工作块，窗口内空隙记休息块；**孤儿段末丢弃**（无起点的 segment_end 无窗口可归，不再从
/// 窗口起点起算）；clock_in / clock_out / auto_clock_out / overtime 只作留痕，不参与窗口推导。
/// 输入契约 = 状态机产出的合法事件流（乱序冗余仅做容错合并，不放大为错误——本函数是视图聚合）。
/// # 参数
/// windows 为该数据日的在岗区间（升序，半开 `[start, end)`；零长窗口忽略）；
/// events 为按时间升序的事件（含跨日班的前段）；open_tail 为末窗仍在岗（duty_ended_at 置 None）。
pub fn reduce_day(
    windows: &[(i64, i64)],
    events: &[(i64, EventKind)],
    open_tail: bool,
) -> DaySummary {
    let mut work_windows: Vec<(i64, i64)> = Vec::new();
    let mut pending_work: Option<i64> = None;

    // 收一个工作块：起点 ws，终点取 we（非负长度才收；pending 为空即丢弃 = 孤儿段末）
    fn take_work(pending: &mut Option<i64>, windows: &mut Vec<(i64, i64)>, we: i64) {
        if let Some(ws) = pending.take() {
            if we > ws {
                windows.push((ws, we));
            }
        }
    }

    for &(at, kind) in events {
        match kind {
            EventKind::SegmentStart => {
                if pending_work.is_none() {
                    pending_work = Some(at);
                }
            }
            EventKind::SegmentEnd => take_work(&mut pending_work, &mut work_windows, at),
            // 窗口由 windows 给定：打卡/自动下班/加班/重置标记/崩溃补下班只作留痕，不参与窗口推导
            EventKind::ClockIn
            | EventKind::ClockOut
            | EventKind::AutoClockOut
            | EventKind::Overtime
            | EventKind::SessionReset
            | EventKind::CrashClockOut => {}
        }
    }
    // 未收段（崩溃残留）闭于末窗末尾——按"一直在工作"处理，与旧口径未闭段闭合一致
    if let Some(&(_, last_end)) = windows.last() {
        take_work(&mut pending_work, &mut work_windows, last_end);
    }

    // 逐窗口装配区块：工作区间裁剪并入窗，空隙为休息
    let mut summary = DaySummary {
        duty_started_at: None,
        duty_ended_at: None,
        duty_secs: 0,
        work_secs: 0,
        rest_secs: 0,
        blocks: Vec::new(),
        // PATCH005.3：预算三字段由命令层按"今日"填充（归约层不感知预算）
        budget_hours: 0,
        used_secs: 0,
        remaining_secs: 0,
        // FIX006.4：shift_start 由本函数按 open_tail 填；可加班 / 到顶由命令层按预算与"今日"填
        shift_start: None,
        can_overtime: false,
        cap_reached: false,
    };
    let mut merged: Vec<(i64, i64)> = Vec::new();
    for &(ds, de) in windows {
        if de <= ds {
            continue; // 零长窗口（误触上下班同一秒）不产出区块也不计三值
        }
        summary.duty_secs += de - ds;
        if summary.duty_started_at.is_none() {
            summary.duty_started_at = Some(ds);
        }
        summary.duty_ended_at = Some(de);

        merged.clear();
        for &(ws, we) in &work_windows {
            let s = ws.max(ds);
            let e = we.min(de);
            if e > s {
                // 插入排序式合并：work_windows 近似有序，重叠/相邻并入上一块
                match merged.last_mut() {
                    Some(last) if s <= last.1 => last.1 = last.1.max(e),
                    _ => merged.push((s, e)),
                }
            }
        }
        let mut cursor = ds;
        for &(ws, we) in &merged {
            if ws > cursor {
                summary.blocks.push(DayBlock {
                    start: cursor,
                    end: ws,
                    kind: BlockKind::Rest,
                });
            }
            summary.blocks.push(DayBlock {
                start: ws,
                end: we,
                kind: BlockKind::Work,
            });
            cursor = we;
        }
        if de > cursor {
            summary.blocks.push(DayBlock {
                start: cursor,
                end: de,
                kind: BlockKind::Rest,
            });
        }
    }
    let work_secs: i64 = summary
        .blocks
        .iter()
        .filter(|b| b.kind == BlockKind::Work)
        .map(|b| b.end - b.start)
        .sum();
    summary.work_secs = work_secs;
    summary.rest_secs = summary.duty_secs - work_secs;
    if open_tail {
        summary.duty_ended_at = None;
    }
    // FIX006.4：当前班起点 = 末个在岗窗口起点（= 最后一个班间空档之后的第一块起点）；未在岗为 None
    summary.shift_start = if open_tail {
        windows.last().map(|&(start, _)| start)
    } else {
        None
    };
    summary
}
#[cfg(test)]
mod tests {
    use super::*;

    /// 时间简写：基准日零点 + 秒偏移（纯整数，无时区耦合）。
    const DAY: i64 = 864_000;
    const HOUR: i64 = 3_600;

    /// 事件简写。
    fn ev(at: i64, kind: EventKind) -> (i64, EventKind) {
        (at, kind)
    }

    // —— W2：WorkdayState 状态机 ——

    /// Off 态上班：进入 OnDuty 且携带 clock_in_at（PL027 起不再携带库行 id）。
    #[test]
    fn clock_in_from_off_enters_on_duty() {
        let mut s = WorkdayState::Off;
        s.clock_in(1_000).unwrap();
        assert_eq!(s, WorkdayState::OnDuty { clock_in_at: 1_000 });
        assert!(s.is_on_duty());
        assert_eq!(s.clock_in_at(), Some(1_000));
    }

    /// 重复上班严格报错（AlreadyOnDuty），状态不被破坏。
    #[test]
    fn clock_in_while_on_duty_errors() {
        let mut s = WorkdayState::Off;
        s.clock_in(1_000).unwrap();
        assert_eq!(s.clock_in(2_000), Err(WorkdayError::AlreadyOnDuty));
        assert_eq!(s, WorkdayState::OnDuty { clock_in_at: 1_000 });
    }

    /// OnDuty 下班：转移回 Off（在岗信息由命令层转移前读取，此处只管状态）。
    #[test]
    fn clock_out_exits_duty() {
        let mut s = WorkdayState::Off;
        s.clock_in(1_000).unwrap();
        s.clock_out().unwrap();
        assert_eq!(s, WorkdayState::Off);
        assert!(!s.is_on_duty());
        assert_eq!(s.clock_in_at(), None);
    }

    /// 未上班下班严格报错（NotOnDuty）。
    #[test]
    fn clock_out_while_off_errors() {
        let mut s = WorkdayState::Off;
        assert_eq!(s.clock_out(), Err(WorkdayError::NotOnDuty));
    }

    /// 启动恢复语义（PL027.4）：由未配对打卡时刻重建在岗态。
    #[test]
    fn restore_on_duty_builds_on_duty_state() {
        let restored = WorkdayState::restore_on_duty(1_000);
        assert_eq!(restored, WorkdayState::OnDuty { clock_in_at: 1_000 });
        assert!(restored.is_on_duty());
        assert_eq!(WorkdayState::Off.clock_in_at(), None);
    }

    // —— PL027.6：由流水推在岗窗口 ——

    /// 单班：打卡开窗、下班关窗；窗口只收本数据日的打卡。
    #[test]
    fn duty_windows_single_shift() {
        let events = [
            ev(DAY + 9 * HOUR, EventKind::ClockIn),
            ev(DAY + 17 * HOUR, EventKind::ClockOut),
        ];
        let (w, open) = duty_windows_for_day(&events, DAY, DAY + 86_400, DAY + 20 * HOUR);
        assert_eq!(w, vec![(DAY + 9 * HOUR, DAY + 17 * HOUR)]);
        assert!(!open);
    }

    /// 跨零点班：打卡在当日、下班在次日 → 窗口随打卡日归（不按自然日切分）；次日无窗口。
    #[test]
    fn duty_windows_cross_midnight_belongs_to_clock_in_day() {
        let events = [
            ev(DAY + 20 * HOUR, EventKind::ClockIn),
            ev(DAY + 26 * HOUR, EventKind::AutoClockOut),
        ];
        let now = DAY + 30 * HOUR;
        let (today, open_today) = duty_windows_for_day(&events, DAY, DAY + 86_400, now);
        assert_eq!(today, vec![(DAY + 20 * HOUR, DAY + 26 * HOUR)]);
        assert!(!open_today);
        let (next, open_next) = duty_windows_for_day(&events, DAY + 86_400, DAY + 2 * 86_400, now);
        assert!(next.is_empty(), "次日无打卡 → 无窗口");
        assert!(!open_next);
    }

    /// 未关班闭于 now 且标记在岗（在岗中的今日视图）。
    #[test]
    fn duty_windows_open_tail_closes_at_now() {
        let events = [ev(DAY + 9 * HOUR, EventKind::ClockIn)];
        let now = DAY + 12 * HOUR;
        let (w, open) = duty_windows_for_day(&events, DAY, DAY + 86_400, now);
        assert_eq!(w, vec![(DAY + 9 * HOUR, now)]);
        assert!(open, "末班未关 = 在岗中");
    }

    /// 在岗末窗保底 1 秒（PATCH004.1）：打卡当秒 `now == start` 不得丢窗——否则当日摘要里
    /// 只剩上一班，"当前班起点 = 末个班间空档之后"的推法会退回到当天首块（表盘底轨锚错）。
    #[test]
    fn duty_windows_open_tail_keeps_min_one_second() {
        let events = [ev(DAY + 9 * HOUR, EventKind::ClockIn)];
        let (w, open) = duty_windows_for_day(&events, DAY, DAY + 86_400, DAY + 9 * HOUR);
        assert_eq!(w, vec![(DAY + 9 * HOUR, DAY + 9 * HOUR + 1)]);
        assert!(open, "同秒也是在岗中");
    }

    // —— PATCH005.3：今日预算与加班步进（纯函数层）——

    /// 今日预算：基准 + 每次加班 4h、硬顶 12h（用户例：6 → 10 → 12；第二次 +4 实得 +2）。
    #[test]
    fn day_budget_steps_by_four_and_caps_at_twelve() {
        assert_eq!(day_budget_hours(6, 0), 6);
        assert_eq!(day_budget_hours(6, 1), 10);
        assert_eq!(day_budget_hours(6, 2), 12, "第二次加班撞顶：+4 实得 +2");
        assert_eq!(day_budget_hours(8, 1), 12);
        assert_eq!(day_budget_hours(12, 1), 12, "基准即 12 时不再增长");
    }

    /// 今日已累计在岗 = Σ 当日在岗窗口（班间空档不在窗口里，天然不计）。
    #[test]
    fn day_used_secs_sums_duty_windows() {
        assert_eq!(day_used_secs(&[]), 0);
        assert_eq!(day_used_secs(&[(DAY, DAY + 4 * HOUR)]), 4 * HOUR);
        // 同日两班：09–12（3h）+ 13–14（1h）= 4h；中间 12–13 空档不算
        assert_eq!(
            day_used_secs(&[
                (DAY + 9 * HOUR, DAY + 12 * HOUR),
                (DAY + 13 * HOUR, DAY + 14 * HOUR)
            ]),
            4 * HOUR
        );
    }

    /// 本班剩余 = 今日上限 − 本班打卡前的已累计（用户例：基准 6h、第一班做 4h → 第二班只剩 2h）。
    #[test]
    fn shift_remaining_is_budget_minus_used_before_shift() {
        assert_eq!(shift_remaining_secs(6, 0), 6 * HOUR);
        assert_eq!(shift_remaining_secs(6, 4 * HOUR), 2 * HOUR);
        assert_eq!(shift_remaining_secs(6, 6 * HOUR), 0);
        assert_eq!(shift_remaining_secs(6, 7 * HOUR), 0, "超额不取负");
    }

    /// 本班到点时刻 = 本班打卡 + 剩余（前端"轴末 / 下班 HH:MM"同一口径）；零头 10 分钟照武装。
    #[test]
    fn shift_deadline_is_clock_in_plus_remaining() {
        let clock_in = DAY + 13 * HOUR;
        assert_eq!(shift_deadline(clock_in, 2 * HOUR), DAY + 15 * HOUR);
        assert_eq!(shift_deadline(clock_in, 600), clock_in + 600, "零头照武装");
    }

    /// 加班裁决：到顶（上限已 12h）或跨日（now 日历日 ≠ 打卡日）→ 拒（只给下班）；否则允许。
    #[test]
    fn overtime_decision_blocks_at_cap_and_across_midnight() {
        assert_eq!(overtime_decision(DAY, DAY, 6), OvertimeDecision::Allow);
        assert_eq!(overtime_decision(DAY, DAY, 10), OvertimeDecision::Allow);
        assert_eq!(overtime_decision(DAY, DAY, 12), OvertimeDecision::AtCap);
        assert_eq!(
            overtime_decision(DAY + 86_400, DAY, 6),
            OvertimeDecision::AcrossMidnight,
            "22:00 开、次日到点：跨日不给加班"
        );
    }

    /// 脏数据兜底：前班未关又出现新打卡 → 前班在该刻结束（窗口不重叠）。
    #[test]
    fn duty_windows_normalizes_nested_clock_in() {
        let events = [
            ev(DAY + 9 * HOUR, EventKind::ClockIn),
            ev(DAY + 10 * HOUR, EventKind::ClockIn),
            ev(DAY + 11 * HOUR, EventKind::ClockOut),
        ];
        let (w, open) = duty_windows_for_day(&events, DAY, DAY + 86_400, DAY + 12 * HOUR);
        assert_eq!(
            w,
            vec![
                (DAY + 9 * HOUR, DAY + 10 * HOUR),
                (DAY + 10 * HOUR, DAY + 11 * HOUR)
            ],
            "前班在新打卡刻结束，窗口不重叠"
        );
        assert!(!open);
    }

    /// 孤儿下班事件（无打卡）不产生窗口；多班按时间升序返回。
    #[test]
    fn duty_windows_orphan_close_and_multi_shift_order() {
        let events = [
            ev(DAY + 8 * HOUR, EventKind::ClockOut),
            ev(DAY + 11 * HOUR, EventKind::ClockIn),
            ev(DAY + 12 * HOUR, EventKind::ClockOut),
            ev(DAY + 14 * HOUR, EventKind::ClockIn),
            ev(DAY + 15 * HOUR, EventKind::CrashClockOut),
        ];
        let (w, open) = duty_windows_for_day(&events, DAY, DAY + 86_400, DAY + 16 * HOUR);
        assert_eq!(
            w,
            vec![
                (DAY + 11 * HOUR, DAY + 12 * HOUR),
                (DAY + 14 * HOUR, DAY + 15 * HOUR)
            ],
            "孤儿下班不产窗口；两班升序"
        );
        assert!(!open);
    }

    // —— EventKind 序列化 ——

    /// 五类事件 as_str ↔ parse 往返一致（存储 TEXT 列的编解码契约）。
    #[test]
    fn event_kind_str_roundtrip() {
        let all = [
            EventKind::ClockIn,
            EventKind::ClockOut,
            EventKind::AutoClockOut,
            EventKind::SegmentStart,
            EventKind::SegmentEnd,
        ];
        for kind in all {
            assert_eq!(EventKind::parse(kind.as_str()), Some(kind));
        }
        assert_ne!(EventKind::ClockIn.as_str(), EventKind::ClockOut.as_str());
    }

    /// 未知 kind 字符串解析为 None（存储层据此报 UnknownEventKind，不静默吞）。
    #[test]
    fn parse_unknown_is_none() {
        assert_eq!(EventKind::parse("nonsense"), None);
        assert_eq!(EventKind::parse(""), None);
    }

    // —— 自动下班判定 ——

    /// 到点时刻 = 上班 + N 小时（账目准时基准；打卡武装与加班拉满共用同一算术）。
    #[test]
    fn auto_out_at_is_clock_in_plus_hours() {
        let in_at = 10_000;
        assert_eq!(auto_out_at(in_at, 8), in_at + 8 * HOUR);
        assert_eq!(
            auto_out_at(in_at, OVERTIME_CAP_HOURS),
            in_at + 12 * HOUR,
            "加班拉满即按上限小时数取点"
        );
    }

    // —— W1：reduce_day 事件归约（PL025.6 窗口口径：在岗窗口由调用方给定） ——

    /// 空日：无窗口无事件 → 全零、无区块、无在岗起止。
    #[test]
    fn empty_day_reduces_to_zero() {
        let sum = reduce_day(&[], &[], false);
        assert_eq!(sum.duty_secs, 0);
        assert_eq!(sum.work_secs, 0);
        assert_eq!(sum.rest_secs, 0);
        assert!(sum.blocks.is_empty());
        assert_eq!(sum.duty_started_at, None);
        assert_eq!(sum.duty_ended_at, None);
    }

    /// 全程连续工作：duty = work、rest 为零、单块 Work（打卡/下班事件只留痕，不参与窗口推导）。
    #[test]
    fn continuous_work_has_no_rest() {
        let windows = [(DAY + 9 * HOUR, DAY + 17 * HOUR)];
        let events = [
            ev(DAY + 9 * HOUR, EventKind::ClockIn),
            ev(DAY + 9 * HOUR, EventKind::SegmentStart),
            ev(DAY + 17 * HOUR, EventKind::SegmentEnd),
            ev(DAY + 17 * HOUR, EventKind::ClockOut),
        ];
        let sum = reduce_day(&windows, &events, false);
        assert_eq!(sum.duty_secs, 8 * HOUR);
        assert_eq!(sum.work_secs, 8 * HOUR);
        assert_eq!(sum.rest_secs, 0);
        assert_eq!(sum.blocks.len(), 1);
        assert_eq!(sum.blocks[0].kind, BlockKind::Work);
        assert_eq!(sum.blocks[0].start, DAY + 9 * HOUR);
        assert_eq!(sum.blocks[0].end, DAY + 17 * HOUR);
        assert_eq!(sum.duty_started_at, Some(DAY + 9 * HOUR));
        assert_eq!(sum.duty_ended_at, Some(DAY + 17 * HOUR));
    }

    /// 段间空隙 = 休息块：工作 3h + 休息 5h，区块按时间序 Work→Rest。
    #[test]
    fn duty_gap_becomes_rest_block() {
        let windows = [(DAY + 9 * HOUR, DAY + 17 * HOUR)];
        let events = [
            ev(DAY + 9 * HOUR, EventKind::SegmentStart),
            ev(DAY + 12 * HOUR, EventKind::SegmentEnd),
        ];
        let sum = reduce_day(&windows, &events, false);
        assert_eq!(sum.duty_secs, 8 * HOUR);
        assert_eq!(sum.work_secs, 3 * HOUR);
        assert_eq!(sum.rest_secs, 5 * HOUR);
        assert_eq!(sum.blocks.len(), 2);
        assert_eq!(sum.blocks[0].kind, BlockKind::Work);
        assert_eq!(sum.blocks[1].kind, BlockKind::Rest);
        assert_eq!(sum.blocks[1].start, DAY + 12 * HOUR);
        assert_eq!(sum.blocks[1].end, DAY + 17 * HOUR);
    }

    /// 多段多间隙：三值与四区块（单窗口内 Work/Rest 交替；班间空档不产出区块）按序产出。
    #[test]
    fn multiple_segments_alternate_blocks() {
        let windows = [(DAY + 9 * HOUR, DAY + 17 * HOUR)];
        let events = [
            ev(DAY + 9 * HOUR, EventKind::SegmentStart),
            ev(DAY + 10 * HOUR, EventKind::SegmentEnd),
            ev(DAY + 11 * HOUR, EventKind::SegmentStart),
            ev(DAY + 12 * HOUR, EventKind::SegmentEnd),
        ];
        let sum = reduce_day(&windows, &events, false);
        assert_eq!(sum.duty_secs, 8 * HOUR);
        assert_eq!(sum.work_secs, 2 * HOUR);
        assert_eq!(sum.rest_secs, 6 * HOUR);
        let kinds: Vec<BlockKind> = sum.blocks.iter().map(|b| b.kind).collect();
        assert_eq!(
            kinds,
            [
                BlockKind::Work,
                BlockKind::Rest,
                BlockKind::Work,
                BlockKind::Rest
            ]
        );
    }

    /// 未下班（在岗中）：窗口闭于 now（**不按自然日截断**——跨零点尾巴随窗口带入）。
    #[test]
    fn open_duty_closes_at_now() {
        let windows = [(DAY + 9 * HOUR, DAY + 10 * HOUR)];
        let events = [ev(DAY + 9 * HOUR, EventKind::SegmentStart)];
        let sum = reduce_day(&windows, &events, true);
        assert_eq!(sum.duty_secs, HOUR);
        assert_eq!(sum.work_secs, HOUR);
        assert_eq!(sum.duty_ended_at, None, "末窗仍在岗");
        // 跨零点不切分：窗口可越过自然日边界（时钟走了 31h）
        let long = [(DAY + 9 * HOUR, DAY + 40 * HOUR)];
        let sum = reduce_day(&long, &events, true);
        assert_eq!(sum.duty_secs, 31 * HOUR, "窗口越过日界也不裁剪");
        assert_eq!(sum.duty_ended_at, None);
    }

    /// 跨零点班归打卡日（PL025.6 定案）：同窗口全量归本日；次日无该日打卡行 → 无窗口即无数据。
    #[test]
    fn cross_midnight_stays_on_clock_in_day() {
        let windows = [(DAY + 23 * HOUR, DAY + 31 * HOUR)];
        let events = [
            ev(DAY + 23 * HOUR, EventKind::SegmentStart),
            ev(DAY + 27 * HOUR, EventKind::SegmentEnd),
        ];
        let day = reduce_day(&windows, &events, false);
        assert_eq!(day.duty_secs, 8 * HOUR);
        assert_eq!(day.work_secs, 4 * HOUR);
        assert_eq!(day.rest_secs, 4 * HOUR);
        assert_eq!(day.duty_started_at, Some(DAY + 23 * HOUR));
        assert_eq!(day.duty_ended_at, Some(DAY + 31 * HOUR), "终点越过日界保留");
        let next = reduce_day(&[], &events, false);
        assert_eq!(next.duty_secs, 0, "跨零点尾巴不在次日出现");
        assert!(next.blocks.is_empty());
    }

    /// 自动下班回填：段末与下班都记在回填时刻，图谱工作块恰好闭于回填点。
    #[test]
    fn auto_out_backfill_clips_to_out_at() {
        let out_at = DAY + 17 * HOUR;
        let windows = [(DAY + 9 * HOUR, out_at)];
        let events = [
            ev(DAY + 9 * HOUR, EventKind::SegmentStart),
            ev(out_at, EventKind::SegmentEnd),
            ev(out_at, EventKind::AutoClockOut),
        ];
        let sum = reduce_day(&windows, &events, false);
        assert_eq!(sum.duty_secs, 8 * HOUR);
        assert_eq!(sum.work_secs, 8 * HOUR);
        assert_eq!(sum.rest_secs, 0);
        assert_eq!(sum.blocks.last().unwrap().end, out_at);
        assert_eq!(sum.duty_ended_at, Some(out_at));
    }

    /// 零长在岗窗口（误触上下班同一秒）不产出区块也不计三值。
    #[test]
    fn zero_length_duty_is_dropped() {
        let windows = [(DAY + 9 * HOUR, DAY + 9 * HOUR)];
        let sum = reduce_day(&windows, &[], false);
        assert_eq!(sum.duty_secs, 0);
        assert!(sum.blocks.is_empty());
        assert_eq!(sum.duty_started_at, None);
    }

    /// 在岗但全程未计时：整段皆为休息（三值语义：休息 = 在岗 − 工作）。
    #[test]
    fn duty_without_segments_is_all_rest() {
        let windows = [(DAY + 9 * HOUR, DAY + 17 * HOUR)];
        let sum = reduce_day(&windows, &[], false);
        assert_eq!(sum.duty_secs, 8 * HOUR);
        assert_eq!(sum.work_secs, 0);
        assert_eq!(sum.rest_secs, 8 * HOUR);
        assert_eq!(sum.blocks.len(), 1);
        assert_eq!(sum.blocks[0].kind, BlockKind::Rest);
    }

    /// 一日多班（下班后再上班）：两窗口都归约，起止取首尾、末班未关则 ended 为 None。
    #[test]
    fn multiple_duty_windows_per_day() {
        let windows = [
            (DAY + 9 * HOUR, DAY + 10 * HOUR),
            (DAY + 11 * HOUR, DAY + 12 * HOUR),
        ];
        let events = [
            ev(DAY + 9 * HOUR, EventKind::SegmentStart),
            ev(DAY + 10 * HOUR, EventKind::SegmentEnd),
            ev(DAY + 11 * HOUR, EventKind::SegmentStart),
        ];
        let sum = reduce_day(&windows, &events, true);
        assert_eq!(sum.duty_secs, 2 * HOUR);
        assert_eq!(sum.work_secs, 2 * HOUR);
        assert_eq!(sum.rest_secs, 0);
        assert_eq!(sum.duty_started_at, Some(DAY + 9 * HOUR));
        assert_eq!(sum.duty_ended_at, None, "末窗未关（在岗中）");
        assert_eq!(sum.blocks.len(), 2, "两班各一块");
    }

    /// PL025.6 新增：孤儿段末（无起点的 segment_end）丢弃——不再从窗口起点起算（防过计）。
    #[test]
    fn orphan_segment_end_is_dropped() {
        let windows = [(DAY + 9 * HOUR, DAY + 17 * HOUR)];
        let events = [ev(DAY + 12 * HOUR, EventKind::SegmentEnd)];
        let sum = reduce_day(&windows, &events, false);
        assert_eq!(sum.duty_secs, 8 * HOUR);
        assert_eq!(sum.work_secs, 0, "孤儿段末不产生工作块");
        assert_eq!(sum.rest_secs, 8 * HOUR);
    }

    /// PL025.6 新增：崩溃残留的未收段闭于末窗末尾（按"一直在工作"处理），跨窗口裁剪后全覆盖。
    #[test]
    fn unclosed_segment_closes_at_last_window_end() {
        let windows = [
            (DAY + 9 * HOUR, DAY + 12 * HOUR),
            (DAY + 14 * HOUR, DAY + 18 * HOUR),
        ];
        let events = [ev(DAY + 9 * HOUR, EventKind::SegmentStart)];
        let sum = reduce_day(&windows, &events, false);
        assert_eq!(sum.duty_secs, 7 * HOUR);
        assert_eq!(sum.work_secs, 7 * HOUR, "未收段跨窗口裁剪后全覆盖");
        assert_eq!(sum.rest_secs, 0);
    }

    // —— PL025.8/.9：兜底决策与加班事件 ——

    /// 加班事件类型编解码往返（events.kind TEXT 零迁移）。
    #[test]
    fn overtime_kind_roundtrips() {
        assert_eq!(EventKind::Overtime.as_str(), "overtime");
        assert_eq!(EventKind::parse("overtime"), Some(EventKind::Overtime));
    }

    /// 加班事件不参与归约：与不加该事件的三值、区块完全一致。
    #[test]
    fn overtime_event_is_ignored_by_reduce() {
        let windows = [(DAY + 9 * HOUR, DAY + 17 * HOUR)];
        let base = [
            ev(DAY + 9 * HOUR, EventKind::SegmentStart),
            ev(DAY + 12 * HOUR, EventKind::SegmentEnd),
        ];
        let with_overtime = [
            ev(DAY + 9 * HOUR, EventKind::SegmentStart),
            ev(DAY + 12 * HOUR, EventKind::Overtime),
            ev(DAY + 12 * HOUR, EventKind::SegmentEnd),
        ];
        let plain = reduce_day(&windows, &base, false);
        let overtime = reduce_day(&windows, &with_overtime, false);
        assert_eq!(plain.duty_secs, overtime.duty_secs);
        assert_eq!(plain.work_secs, overtime.work_secs);
        assert_eq!(plain.rest_secs, overtime.rest_secs);
        assert_eq!(plain.blocks, overtime.blocks);
    }

    // —— PL027.2：重置标记与崩溃补下班 ——

    /// PL027.2：两类新事件编解码往返（events.kind TEXT 零迁移）。
    #[test]
    fn session_reset_and_crash_clock_out_roundtrip() {
        assert_eq!(EventKind::SessionReset.as_str(), "session_reset");
        assert_eq!(
            EventKind::parse("session_reset"),
            Some(EventKind::SessionReset)
        );
        assert_eq!(EventKind::CrashClockOut.as_str(), "crash_clock_out");
        assert_eq!(
            EventKind::parse("crash_clock_out"),
            Some(EventKind::CrashClockOut)
        );
    }

    /// PL027.2：两类新事件不参与归约——三值与区块与不含它们时完全一致。
    #[test]
    fn reset_and_crash_events_are_ignored_by_reduce() {
        let windows = [(DAY + 9 * HOUR, DAY + 17 * HOUR)];
        let base = [
            ev(DAY + 9 * HOUR, EventKind::SegmentStart),
            ev(DAY + 12 * HOUR, EventKind::SegmentEnd),
        ];
        let extra = [
            ev(DAY + 9 * HOUR, EventKind::SegmentStart),
            ev(DAY + 11 * HOUR, EventKind::SessionReset),
            ev(DAY + 12 * HOUR, EventKind::SegmentEnd),
            ev(DAY + 13 * HOUR, EventKind::CrashClockOut),
        ];
        let plain = reduce_day(&windows, &base, false);
        let with_extra = reduce_day(&windows, &extra, false);
        assert_eq!(plain.duty_secs, with_extra.duty_secs);
        assert_eq!(plain.work_secs, with_extra.work_secs);
        assert_eq!(plain.rest_secs, with_extra.rest_secs);
        assert_eq!(plain.blocks, with_extra.blocks);
    }

    /// PL027.9：脏数据归一化——嵌套打卡被纠正为"前班在新打卡刻结束"（段末在前、下班在后）。
    #[test]
    fn nested_clock_in_fixes_close_previous_duty() {
        let events = [
            ev(DAY, EventKind::ClockIn),
            ev(DAY, EventKind::SegmentStart),
            ev(DAY + HOUR, EventKind::ClockIn),
            ev(DAY + 2 * HOUR, EventKind::ClockOut),
        ];
        assert_eq!(
            nested_clock_in_fixes(&events),
            vec![
                (DAY + HOUR - 1, EventKind::SegmentEnd),
                (DAY + HOUR - 1, EventKind::ClockOut)
            ],
            "补段末 + 补下班（新打卡前一秒，保证重扫收敛）"
        );

        let clean = [
            ev(DAY, EventKind::ClockIn),
            ev(DAY, EventKind::SegmentStart),
            ev(DAY + HOUR, EventKind::SegmentEnd),
            ev(DAY + HOUR, EventKind::ClockOut),
            ev(DAY + 2 * HOUR, EventKind::ClockIn),
        ];
        assert!(nested_clock_in_fixes(&clean).is_empty(), "干净流水零纠正");

        let duty_only = [
            ev(DAY, EventKind::ClockIn),
            ev(DAY, EventKind::SegmentStart),
            ev(DAY + HOUR, EventKind::SegmentEnd),
            ev(DAY + 2 * HOUR, EventKind::ClockIn),
        ];
        assert_eq!(
            nested_clock_in_fixes(&duty_only),
            vec![(DAY + 2 * HOUR - 1, EventKind::ClockOut)],
            "段已收 → 只补下班"
        );
    }

    /// 兜底决策：无基准 → 停；未到点 → 睡（上限 60s）；到点 → 以基准时刻执行；换代 → 让位。
    #[test]
    fn auto_out_step_matrix() {
        let due = DAY + 8 * HOUR;
        assert_eq!(
            auto_out_step(None, 1, 1, DAY),
            AutoOutStep::Stop,
            "无在岗基准即停"
        );
        assert_eq!(
            auto_out_step(Some(due), 1, 1, due - 3_600),
            AutoOutStep::Wait(60),
            "远期未到点：睡满上限 60s"
        );
        assert_eq!(
            auto_out_step(Some(due), 1, 1, due - 10),
            AutoOutStep::Wait(10),
            "临近到点：按剩余秒数精确唤醒"
        );
        assert_eq!(
            auto_out_step(Some(due), 1, 1, due),
            AutoOutStep::Fire(due),
            "到点：以基准时刻（非发现时刻）执行"
        );
        assert_eq!(
            auto_out_step(Some(due), 1, 1, due + 5_000),
            AutoOutStep::Fire(due),
            "迟到发现仍回填基准时刻"
        );
        assert_eq!(
            auto_out_step(Some(due), 1, 2, due - 10),
            AutoOutStep::Yield,
            "基准已换代（加班/下班/重打卡）：旧线程让位"
        );
    }

    /// 待决窗口：59s 内待决、恰 60s 待决、61s 未到、已过点不待决、无基准不待决。
    #[test]
    fn auto_out_pending_boundaries() {
        let due = DAY + 8 * HOUR;
        assert_eq!(auto_out_pending(Some(due), due - 61), None);
        assert_eq!(auto_out_pending(Some(due), due - 60), Some(due));
        assert_eq!(auto_out_pending(Some(due), due - 1), Some(due));
        assert_eq!(
            auto_out_pending(Some(due), due),
            None,
            "到点即执行，不再待决"
        );
        assert_eq!(auto_out_pending(Some(due), due + 1), None);
        assert_eq!(auto_out_pending(None, due - 30), None);
    }
}
