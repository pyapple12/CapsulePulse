//! 计时状态机：工作会话的三态生命周期与时长计算（纯逻辑，无外部依赖，cargo test 直测）。
//! 设计要点：时间源经 [`Clock`] 注入——业务代码禁止直取系统时间，测试用手拨假钟
//! 验证跨段累计与边界而零真实等待（AGENTS.md 陷阱清单：禁止 sleep 真实等待）。
//! 关联：PL001 阶段 C（z.plan.md 附录）；阶段 D 命令层将以 `Mutex<WorkSession>` 持有本结构。

use std::time::Duration;
use std::time::Instant;

use thiserror::Error;

/// 单调时钟抽象：`now()` 返回自任意起点的单调时长，仅供差值计算。
/// 契约：返回值必须单调不减——[`RealClock`] 由 `Instant` 保证；测试假钟须守同一契约，
/// 违约（时间倒流）会使 `total()`/`pause()` 处 Duration 减法 panic，属程序错误而非运行时错误。
pub trait Clock {
    /// 当前单调时刻（起点任意）。
    fn now(&self) -> Duration;
}

/// 真实时钟：基于 `std::time::Instant`（单调，不受系统时间手动调整影响）。
/// epoch 取构造时刻，`now()` 返回自构造起的时长。
pub struct RealClock {
    epoch: Instant,
}

impl Default for RealClock {
    fn default() -> Self {
        Self::new()
    }
}

impl RealClock {
    /// 创建真实时钟，以当前时刻为计时起点。
    pub fn new() -> Self {
        Self {
            epoch: Instant::now(),
        }
    }
}

impl Clock for RealClock {
    fn now(&self) -> Duration {
        self.epoch.elapsed()
    }
}

/// 会话生命周期三态（计划书 §2.1）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    /// 未开始：无任何累计。
    Idle,
    /// 进行中：`start` = 本段开始的单调时刻。
    Running {
        /// 本段开始时刻。
        start: Duration,
    },
    /// 暂停：`elapsed` = 已累计时长（含此前所有段落）。
    Paused {
        /// 累计时长。
        elapsed: Duration,
    },
}

/// 状态机操作错误：非法状态下的操作严格报错，不静默忽略（AGENTS.md 错误策略主线）。
#[derive(Debug, Error, PartialEq, Eq)]
pub enum SessionError {
    /// 非 Idle 态调用 `start`（已开始或已暂停——重开须先 `reset`）。
    #[error("会话已存在（仅 Idle 态可 start，重开请先 reset）")]
    NotIdle,
    /// 非 Running 态调用 `pause`。
    #[error("会话未在运行（仅 Running 态可 pause）")]
    NotRunning,
    /// 非 Paused 态调用 `resume`。
    #[error("会话未暂停（仅 Paused 态可 resume）")]
    NotPaused,
}

/// 计时切换热键的动作（三态循环：Idle→start / Running→pause / Paused→resume）。
/// 托盘菜单与全局热键共用的纯逻辑推导，不依赖任何外部状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimerAction {
    /// 开始新会话。
    Start,
    /// 暂停进行中的会话。
    Pause,
    /// 继续暂停的会话。
    Resume,
}

impl SessionState {
    /// 本状态下计时切换热键应执行的动作（托盘与全局热键共用）。
    pub fn toggle_action(&self) -> TimerAction {
        match self {
            SessionState::Idle => TimerAction::Start,
            SessionState::Running { .. } => TimerAction::Pause,
            SessionState::Paused { .. } => TimerAction::Resume,
        }
    }
}

/// 工作会话：三态状态机 + 可注入时钟。
/// 内部把"本段起点"与"已完成段累计"分离——`pause()` 因此能返回本段时长（PL002 落库取数源），
/// 而对外的 `total()`（= 累计 + 本段）与三态枚举形状保持 PL001 语义不变。
/// 泛型默认 [`RealClock`]，业务侧直接 `WorkSession::default()`；测试注入假钟。
pub struct WorkSession<C: Clock = RealClock> {
    clock: C,
    state: SessionState,
    accumulated: Duration,
}

impl Default for WorkSession {
    fn default() -> Self {
        Self::new(RealClock::default())
    }
}

impl<C: Clock> WorkSession<C> {
    /// 以给定时钟创建 Idle 会话（不变量：`start()` 仅可自 Idle 进入，此时 accumulated 恒为零）。
    pub fn new(clock: C) -> Self {
        Self {
            clock,
            state: SessionState::Idle,
            accumulated: Duration::ZERO,
        }
    }

    /// 当前状态只读视图（供命令层判断与前端展示映射）。
    pub fn state(&self) -> &SessionState {
        &self.state
    }

    /// 开始新会话：仅 Idle 合法，记录本段起点。
    /// # 错误
    /// 非 Idle 态返回 [`SessionError::NotIdle`]。
    pub fn start(&mut self) -> Result<(), SessionError> {
        if !matches!(self.state, SessionState::Idle) {
            return Err(SessionError::NotIdle);
        }
        self.state = SessionState::Running {
            start: self.clock.now(),
        };
        Ok(())
    }

    /// 暂停：仅 Running 合法；本段时长并入累计并返回（PL002 落库取数源）。
    /// # 错误
    /// 非 Running 态返回 [`SessionError::NotRunning`]。
    pub fn pause(&mut self) -> Result<Duration, SessionError> {
        let SessionState::Running { start } = self.state else {
            return Err(SessionError::NotRunning);
        };
        let segment = self.clock.now() - start;
        self.accumulated += segment;
        self.state = SessionState::Paused {
            elapsed: self.accumulated,
        };
        Ok(segment)
    }

    /// 继续：仅 Paused 合法；本段起点重新记为当前时刻（累计保留在 `accumulated`）。
    /// # 错误
    /// 非 Paused 态返回 [`SessionError::NotPaused`]。
    pub fn resume(&mut self) -> Result<(), SessionError> {
        let SessionState::Paused { .. } = self.state else {
            return Err(SessionError::NotPaused);
        };
        self.state = SessionState::Running {
            start: self.clock.now(),
        };
        Ok(())
    }

    /// 重置回 Idle 并清零累计（未上班/退出等既有用途）；任意态合法、幂等。
    pub fn reset(&mut self) {
        self.state = SessionState::Idle;
        self.accumulated = Duration::ZERO;
    }

    /// 重置为"归零休息"（PATCH002：重置按钮的落点）：任意态 → `Paused { elapsed: 0 }`，
    /// 累计清零、幂等——大计时器归零且**不自动开跑**（休息态，等用户按「工作」再计）。
    /// # 契约
    /// 调用方负责先把未落库的运行段收段落库（见 commands/session 的 `restart_session`）。
    pub fn reset_to_paused(&mut self) {
        self.state = SessionState::Paused {
            elapsed: Duration::ZERO,
        };
        self.accumulated = Duration::ZERO;
    }

    /// 撤销最近一次 pause（命令层落库/留痕失败回滚专用）：恢复 Running{start} 并把
    /// pause 时并入累计的本段从 `accumulated` 扣回——数据未落库，计时语义回到暂停前连续态。
    /// # 契约
    /// 仅可在 pause() 成功后、其他状态变更前调用一次；`paused_from` 须为该次 pause 前的
    /// Running 快照（start 原点）。非 Running 快照为契约外输入，静默保持原状（防御）。
    pub fn undo_pause(&mut self, paused_from: SessionState, segment: Duration) {
        let SessionState::Running { start } = paused_from else {
            return;
        };
        // pause 刚把 segment 并入 accumulated，扣回即回到 pause 前；单调时钟契约下不发生下溢
        self.accumulated -= segment;
        self.state = SessionState::Running { start };
    }

    /// 恢复状态快照（命令层留痕失败回滚专用）：仅回写状态字段、不动累计——
    /// resume 失败回 Paused 语义正确（resume 本就不改累计）。
    pub fn restore_state(&mut self, state: SessionState) {
        self.state = state;
    }

    /// 按给定基线恢复为运行态（PL027.5 崩溃恢复专用）：`start` = 本段起点的**单调**时刻、
    /// `accumulated` = 本段之前的已完成段累计。调用方负责把墙钟起点换算成单调时刻
    /// （见命令层恢复入口）；任意态合法（恢复即重建，不做状态校验）。
    pub fn restore_running(&mut self, start: Duration, accumulated: Duration) {
        self.accumulated = accumulated;
        self.state = SessionState::Running { start };
    }

    /// 按"本段已走时长 + 此前累计"恢复运行态（PL027.4 崩溃恢复专用）：内部以当前单调时刻
    /// 回推本段起点，故调用方只需给墙钟算出的 `elapsed`（= now − 段起点）；任意态合法。
    /// PATCH006.4：单调钟起点 = 本进程启动（[`RealClock`] 契约），而 `elapsed` 来自**墙钟**——
    /// 段起于进程之前时 `elapsed` 会超过"本进程已运行时长"，直接相减即 Duration 下溢 panic
    /// （实机表现：停机 ≤10 分钟重启后主线程崩、应用起不来）。故把超出部分**折进基线**：
    /// 本段起点落在单调零，读数一分不减（累计 = 此前累计 + 超出部分，total() 不变量仍成立）。
    pub fn restore_running_elapsed(&mut self, elapsed: Duration, accumulated: Duration) {
        let now = self.clock.now();
        let (start, carry) = match elapsed.checked_sub(now) {
            Some(extra) => (Duration::ZERO, extra),
            None => (now - elapsed, Duration::ZERO),
        };
        self.accumulated = accumulated + carry;
        self.state = SessionState::Running { start };
    }

    /// 按给定累计恢复为暂停态（PL027.5 崩溃恢复专用，休息中崩溃）：读数停在基线上，
    /// 不自动开跑；任意态合法。
    pub fn restore_paused(&mut self, accumulated: Duration) {
        self.accumulated = accumulated;
        self.state = SessionState::Paused {
            elapsed: accumulated,
        };
    }

    /// 当前段时长：Running = 本段实时；Paused/Idle 为零（"暂停即重置"提醒语义的取数源）。
    pub fn segment_secs(&self) -> Duration {
        match self.state {
            SessionState::Running { start } => self.clock.now() - start,
            SessionState::Idle | SessionState::Paused { .. } => Duration::ZERO,
        }
    }

    /// 累计工作时长：Idle 为零；Running = 已完成段累计 + 本段实时；Paused = 已完成段累计。
    pub fn total(&self) -> Duration {
        match self.state {
            SessionState::Idle => Duration::ZERO,
            SessionState::Running { start } => self.accumulated + (self.clock.now() - start),
            SessionState::Paused { elapsed } => elapsed,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::rc::Rc;
    use std::time::Duration;

    use super::*;

    /// 手拨假钟：测试直接推进 offset 模拟时间流逝（零真实等待）。
    /// 内部用 `Rc<Cell<Duration>>` 使 clone 与 WorkSession 持有的实例共享同一时间轴。
    #[derive(Clone)]
    struct FakeClock {
        offset: Rc<Cell<Duration>>,
    }

    impl FakeClock {
        /// 创建从零起的假钟。
        fn new() -> Self {
            Self {
                offset: Rc::new(Cell::new(Duration::ZERO)),
            }
        }

        /// 快进（只进不退，守 Clock 单调契约）。
        fn advance(&self, d: Duration) {
            self.offset.set(self.offset.get() + d);
        }
    }

    impl Clock for FakeClock {
        fn now(&self) -> Duration {
            self.offset.get()
        }
    }

    /// 秒数简写。
    fn secs(n: u64) -> Duration {
        Duration::from_secs(n)
    }

    /// 边界：Idle 态 total 为零。
    #[test]
    fn idle_total_is_zero() {
        let s = WorkSession::new(FakeClock::new());
        assert_eq!(s.total(), Duration::ZERO);
        assert_eq!(s.state(), &SessionState::Idle);
    }

    /// S1：多轮 start-pause-resume 循环累计正确；暂停期间 total 冻结；每次 pause 返回本段时长。
    #[test]
    fn multi_round_accumulation() {
        let clock = FakeClock::new();
        let mut s = WorkSession::new(clock.clone());
        // 第 1 段 100s
        s.start().unwrap();
        clock.advance(secs(100));
        assert_eq!(s.pause().unwrap(), secs(100));
        assert_eq!(s.total(), secs(100));
        // 暂停期间推进 50s 不计入
        clock.advance(secs(50));
        assert_eq!(s.total(), secs(100));
        // 第 2 段 30s，resume 继承累计
        s.resume().unwrap();
        clock.advance(secs(30));
        assert_eq!(s.pause().unwrap(), secs(30));
        assert_eq!(s.total(), secs(130));
        // 第 3 段 45s：Running 态 total() 现算 = 历史 + 本段实时
        s.resume().unwrap();
        clock.advance(secs(45));
        assert_eq!(s.total(), secs(175));
        assert_eq!(s.pause().unwrap(), secs(45));
        assert_eq!(s.total(), secs(175));
    }

    /// S1：Running 态 total() 随假钟实时增长。
    #[test]
    fn running_total_is_computed_live() {
        let clock = FakeClock::new();
        let mut s = WorkSession::new(clock.clone());
        s.start().unwrap();
        clock.advance(secs(10));
        assert_eq!(s.total(), secs(10));
        clock.advance(secs(5));
        assert_eq!(s.total(), secs(15));
    }

    /// S2：非法态操作严格报错——Idle 下 pause/resume、Running/Paused 下重复 start。
    #[test]
    fn invalid_state_operations_error() {
        let clock = FakeClock::new();
        let mut s = WorkSession::new(clock);
        // Idle 下 pause / resume
        assert_eq!(s.pause(), Err(SessionError::NotRunning));
        assert_eq!(s.resume(), Err(SessionError::NotPaused));
        // Running 下重复 start
        s.start().unwrap();
        assert_eq!(s.start(), Err(SessionError::NotIdle));
        assert_eq!(s.pause(), Ok(Duration::ZERO));
        // Paused 下 start（重开须先 reset）与 resume 正常路径
        assert_eq!(s.start(), Err(SessionError::NotIdle));
        assert_eq!(s.resume(), Ok(()));
    }

    /// 边界：零时长会话（start 后立即 pause）合法且 total 为零。
    #[test]
    fn zero_duration_session() {
        let clock = FakeClock::new();
        let mut s = WorkSession::new(clock.clone());
        s.start().unwrap();
        s.pause().unwrap();
        assert_eq!(s.total(), Duration::ZERO);
        assert!(
            matches!(s.state(), SessionState::Paused { elapsed } if *elapsed == Duration::ZERO)
        );
    }

    /// S3：假钟大幅任意拨动（零步进/超大步进混合）循环 100 轮无 panic，total 恒等于累计。
    #[test]
    fn fake_clock_wild_advance_no_panic() {
        let clock = FakeClock::new();
        let mut s = WorkSession::new(clock.clone());
        let steps = [0u64, 1, 3_600, 86_400, 0, 604_800];
        let mut expect = Duration::ZERO;
        s.start().unwrap();
        for round in 0..100 {
            let d = secs(steps[round % steps.len()]);
            clock.advance(d);
            expect += d;
            s.pause().unwrap();
            assert_eq!(s.total(), expect);
            s.resume().unwrap();
        }
        s.pause().unwrap();
        assert_eq!(s.total(), expect);
    }

    /// PL003.5：segment_secs——仅 Running 有本段时长，Paused/Idle 为零（暂停即重置取数源）。
    #[test]
    fn segment_secs_semantics() {
        let clock = FakeClock::new();
        let mut s = WorkSession::new(clock.clone());
        assert_eq!(s.segment_secs(), Duration::ZERO);
        s.start().unwrap();
        clock.advance(secs(10));
        assert_eq!(s.segment_secs(), secs(10));
        s.pause().unwrap();
        assert_eq!(s.segment_secs(), Duration::ZERO);
        s.resume().unwrap();
        clock.advance(secs(5));
        assert_eq!(s.segment_secs(), secs(5));
        assert_eq!(s.total(), secs(15));
    }

    /// reset：任意态回 Idle 清零（UI"重开"路径），重开后先前累计不带入。
    #[test]
    fn reset_returns_to_idle_and_clears() {
        let clock = FakeClock::new();
        let mut s = WorkSession::new(clock.clone());
        s.start().unwrap();
        clock.advance(secs(20));
        s.reset();
        assert_eq!(s.state(), &SessionState::Idle);
        assert_eq!(s.total(), Duration::ZERO);
        s.start().unwrap();
        clock.advance(secs(5));
        assert_eq!(s.total(), secs(5));
        // Idle 下重复 reset 幂等无害
        s.pause().unwrap();
        s.reset();
        s.reset();
        assert_eq!(s.state(), &SessionState::Idle);
    }

    /// PATCH002：reset_to_paused —— 任意态 → Paused{0}，累计清零、幂等（重置按钮的落点）。
    #[test]
    fn reset_to_paused_lands_zeroed_pause_from_any_state() {
        let clock = FakeClock::new();
        let mut s = WorkSession::new(clock.clone());

        // Idle → Paused{0}
        s.reset_to_paused();
        assert_eq!(
            s.state(),
            &SessionState::Paused {
                elapsed: Duration::ZERO
            }
        );
        assert_eq!(s.total(), Duration::ZERO);

        // Running 中累计后重置 → 归零且落在 Paused（不再自动开跑）
        s.resume().unwrap();
        clock.advance(secs(30));
        s.reset_to_paused();
        assert_eq!(
            s.state(),
            &SessionState::Paused {
                elapsed: Duration::ZERO
            }
        );
        assert_eq!(s.total(), Duration::ZERO);

        // 幂等：重复调用无副作用
        s.reset_to_paused();
        assert_eq!(
            s.state(),
            &SessionState::Paused {
                elapsed: Duration::ZERO
            }
        );
    }

    /// T5：计时切换热键的动作映射——Idle→start / Running→pause / Paused→resume 三态循环。
    #[test]
    fn toggle_action_maps_three_states() {
        assert_eq!(SessionState::Idle.toggle_action(), TimerAction::Start);
        assert_eq!(
            SessionState::Running {
                start: Duration::ZERO
            }
            .toggle_action(),
            TimerAction::Pause
        );
        assert_eq!(
            SessionState::Paused {
                elapsed: Duration::ZERO
            }
            .toggle_action(),
            TimerAction::Resume
        );
    }

    /// FIX002.1：undo_pause 恢复 Running 并扣回本段——回滚后计时连续无双计。
    #[test]
    fn undo_pause_restores_running_without_double_count() {
        let clock = FakeClock::new();
        let mut s = WorkSession::new(clock.clone());
        s.start().unwrap();
        clock.advance(secs(100));
        let segment = s.pause().unwrap();
        assert_eq!(s.state(), &SessionState::Paused { elapsed: secs(100) });
        s.undo_pause(
            SessionState::Running {
                start: Duration::ZERO,
            },
            segment,
        );
        assert_eq!(
            s.state(),
            &SessionState::Running {
                start: Duration::ZERO
            }
        );
        clock.advance(secs(50));
        assert_eq!(s.total(), secs(150), "回滚后连续计时：100 + 50 无双计");
        assert_eq!(s.pause().unwrap(), secs(150));
    }

    /// FIX002.1：restore_state 仅回写状态、不动累计——resume 失败回 Paused 语义完整。
    #[test]
    fn restore_state_recovers_paused_without_touching_accumulated() {
        let clock = FakeClock::new();
        let mut s = WorkSession::new(clock.clone());
        s.start().unwrap();
        clock.advance(secs(100));
        s.pause().unwrap();
        let snapshot = *s.state();
        s.resume().unwrap();
        s.restore_state(snapshot);
        assert_eq!(s.state(), &SessionState::Paused { elapsed: secs(100) });
        assert_eq!(s.total(), secs(100));
    }

    // —— PL027.5：崩溃恢复的会话入口 ——

    /// PL027.5：按给定起点与累计恢复运行态——读数 = 累计 + 从原段起点续算（停机折入本段）。
    #[test]
    fn restore_running_continues_from_given_baseline() {
        let clock = FakeClock::new();
        let mut s = WorkSession::new(clock.clone());
        clock.advance(secs(100)); // 墙钟意义上"本段已走了 100s"
        s.restore_running(Duration::ZERO, secs(300));
        assert_eq!(
            s.state(),
            &SessionState::Running {
                start: Duration::ZERO
            }
        );
        assert_eq!(s.total(), secs(400), "此前累计 300 + 本段 100");
        clock.advance(secs(20));
        assert_eq!(s.total(), secs(420), "恢复后继续走");
        assert_eq!(s.pause().unwrap(), secs(120), "本段时长 = 起点到暂停");
        assert_eq!(s.total(), secs(420));
    }

    /// PATCH006.4：单调钟起点 = **本进程启动**（[`RealClock`] 契约），而恢复段的 `elapsed` 由
    /// **墙钟**算出——段起于进程之前时 `elapsed` 会大于"本进程已运行时长"，直接相减即 Duration
    /// 下溢（实机表现：停机 ≤10 分钟重启 → 主线程 panic、应用起不来）。恢复须把超出部分折进基线。
    #[test]
    fn restore_running_elapsed_survives_elapsed_beyond_monotonic_origin() {
        /// 可控单调钟：起点 0（模拟"进程刚启动"的单调轴），可手拨。
        #[derive(Clone)]
        struct StepClock {
            at: std::sync::Arc<std::sync::Mutex<Duration>>,
        }
        impl Clock for StepClock {
            fn now(&self) -> Duration {
                *self.at.lock().unwrap()
            }
        }
        let clock = StepClock {
            at: std::sync::Arc::new(std::sync::Mutex::new(Duration::ZERO)),
        };
        let mut s = WorkSession::new(clock.clone());
        // 进程刚起来（单调 = 0），恢复"本段已走 27 分钟、此前累计 5 分钟"的班
        s.restore_running_elapsed(secs(27 * 60), secs(5 * 60));
        assert_eq!(s.total(), secs(32 * 60), "读数 = 此前累计 + 本段已走");
        *clock.at.lock().unwrap() = secs(5);
        assert_eq!(s.total(), secs(32 * 60 + 5), "恢复后继续走字");
    }

    /// PL027.5：按给定累计恢复暂停态（休息中崩溃）——读数停在基线上、不自动开跑。
    #[test]
    fn restore_paused_holds_given_baseline() {
        let clock = FakeClock::new();
        let mut s = WorkSession::new(clock.clone());
        s.restore_paused(secs(500));
        assert_eq!(s.state(), &SessionState::Paused { elapsed: secs(500) });
        assert_eq!(s.total(), secs(500));
        clock.advance(secs(30));
        assert_eq!(s.total(), secs(500), "休息态读数不动");
        s.resume().unwrap();
        clock.advance(secs(10));
        assert_eq!(s.total(), secs(510), "继续后在基线之上累加");
    }
}
