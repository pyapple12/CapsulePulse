//! 工作日/打卡纯逻辑层（PL005）：WorkdayState 状态机（Off/OnDuty）、EventKind 事件类型、
//! 事件归约纯函数 reduce_day（events → 在岗/工作/休息 + 时间图谱区块）。
//! 设计要点：① 状态机与归约零 IO、时间戳全用 Unix 秒入参注入——跨零点/回填等边界
//! 在 cargo test 下直测，禁真实等待（AGENTS 陷阱清单）；② OnDuty 携带库行 id（下班落库
//! 的行句柄随态流转，免二次查询——对条目"OnDuty{clock_in_at}"的实现细则扩展）；
//! ③ reduce_day 输出结构直接 derive Serialize 作为 IPC DTO 单一来源（serde 非 tauri，纯度不破）。
//! 关联：storage.rs workdays/events 两表（数据源），commands/workday.rs（接线）。

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
            _ => None,
        }
    }
}

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

/// 在岗时段信息：clock_out 转移的返回值，命令层据其落库关行（workdays.id 行句柄）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DutySpan {
    /// 上班打卡时刻（Unix 秒）。
    pub clock_in_at: i64,
    /// workdays 表行 id。
    pub id: i64,
}

/// 工作日状态机：Off（未上班）/ OnDuty（在岗中）。
/// OnDuty 携带 clock_in_at（自动下班回填的基准）与 id（下班关行的库句柄）——
/// id 属库行引用而非业务语义，随态流转避免命令层二次查询。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WorkdayState {
    /// 未上班。
    #[default]
    Off,
    /// 在岗中：clock_in_at = 上班时刻，id = workdays 行 id。
    OnDuty {
        /// 上班打卡时刻（Unix 秒）。
        clock_in_at: i64,
        /// workdays 表行 id。
        id: i64,
    },
}

impl WorkdayState {
    /// 上班转移：仅 Off 合法。
    /// # 错误
    /// 已在岗返回 [`WorkdayError::AlreadyOnDuty`]。
    pub fn clock_in(&mut self, at: i64, id: i64) -> Result<(), WorkdayError> {
        if self.is_on_duty() {
            return Err(WorkdayError::AlreadyOnDuty);
        }
        *self = WorkdayState::OnDuty {
            clock_in_at: at,
            id,
        };
        Ok(())
    }

    /// 下班转移：仅 OnDuty 合法，返回在岗信息供落库关行。
    /// # 错误
    /// 未上班返回 [`WorkdayError::NotOnDuty`]。
    pub fn clock_out(&mut self) -> Result<DutySpan, WorkdayError> {
        let WorkdayState::OnDuty { clock_in_at, id } = *self else {
            return Err(WorkdayError::NotOnDuty);
        };
        *self = WorkdayState::Off;
        Ok(DutySpan { clock_in_at, id })
    }

    /// 是否在岗中（计时门禁与启动恢复判断用）。
    pub fn is_on_duty(&self) -> bool {
        matches!(self, WorkdayState::OnDuty { .. })
    }

    /// 启动恢复：库行 clock_out 为空 = 在岗恢复；已关行或无行 = Off。
    /// # 参数
    /// latest 为 workday_latest 查询结果 (id, clock_in_at, clock_out_at)。
    pub fn from_latest(latest: Option<(i64, i64, Option<i64>)>) -> Self {
        match latest {
            Some((id, clock_in_at, None)) => WorkdayState::OnDuty { clock_in_at, id },
            Some(_) | None => WorkdayState::Off,
        }
    }
}

/// 自动下班到点判定：now 达到上班 + N 小时即到点，返回回填时刻（上班 + N，与发现时刻无关——
/// "发现可迟到、账目准时"定案）；未到点返回 None。
pub fn auto_out_due(now: i64, clock_in_at: i64, hours: u32) -> Option<i64> {
    let out_at = clock_in_at + i64::from(hours) * 3_600;
    (now >= out_at).then_some(out_at)
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
    /// 时间图谱区块（按时间序，Work/Rest 交替）。
    pub blocks: Vec<DayBlock>,
}

/// 事件归约纯函数：单日 events 流 → 在岗/工作/休息 + 图谱区块（PL005 图表与三值唯一取数逻辑）。
/// 规则：duty 窗口 = clock_in → clock_out（未下班闭于 min(now, day_end)；孤儿 clock_out =
/// 前日班跨入本日，自 day_start 起算）；segment_start → segment_end 记工作块（孤儿段末同理），
/// 在岗内空隙记休息块；全部区块裁剪到 [day_start, day_end)。输入契约 = 状态机产出的合法
/// 事件流（乱序冗余仅做容错合并，不放大为错误——本函数是视图聚合，输入仅来自本应用写入）。
/// # 参数
/// events 为按时间升序的单日事件（含跨日班的前段）；now 为当前 Unix 秒（闭未下班窗口）。
pub fn reduce_day(
    events: &[(i64, EventKind)],
    day_start: i64,
    day_end: i64,
    now: i64,
) -> DaySummary {
    let mut duty_windows: Vec<(i64, i64)> = Vec::new();
    let mut work_windows: Vec<(i64, i64)> = Vec::new();
    let mut open_duty: Option<i64> = None;
    let mut pending_work: Option<i64> = None;
    let mut open_window = false;

    // 收一个工作块：起点 ws，终点取 we（非负长度才收）
    fn take_work(pending: &mut Option<i64>, windows: &mut Vec<(i64, i64)>, we: i64) {
        if let Some(ws) = pending.take() {
            if we > ws {
                windows.push((ws, we));
            }
        }
    }

    for &(at, kind) in events {
        match kind {
            EventKind::ClockIn => {
                if open_duty.is_none() {
                    open_duty = Some(at.max(day_start));
                }
            }
            EventKind::ClockOut | EventKind::AutoClockOut => {
                let start = open_duty.take().unwrap_or(day_start);
                let end = at.min(day_end);
                if end > start {
                    duty_windows.push((start, end));
                }
                take_work(&mut pending_work, &mut work_windows, end);
            }
            EventKind::SegmentStart => {
                if pending_work.is_none() {
                    pending_work = Some(at.max(day_start));
                }
            }
            EventKind::SegmentEnd => {
                pending_work.get_or_insert(day_start);
                take_work(&mut pending_work, &mut work_windows, at.min(day_end));
            }
        }
    }
    // 未下班窗口闭于 min(now, day_end)；未闭段随之闭合
    if let Some(start) = open_duty {
        let end = now.min(day_end);
        if end > start {
            duty_windows.push((start, end));
            take_work(&mut pending_work, &mut work_windows, end);
            open_window = true;
        }
    }

    // 逐班窗口装配区块：工作区间裁剪合并入窗，空隙为休息
    let mut summary = DaySummary {
        duty_started_at: None,
        duty_ended_at: None,
        duty_secs: 0,
        work_secs: 0,
        rest_secs: 0,
        blocks: Vec::new(),
    };
    let mut merged: Vec<(i64, i64)> = Vec::new();
    for &(ds, de) in &duty_windows {
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
    if open_window {
        summary.duty_ended_at = None;
    }
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

    /// Off 态上班：进入 OnDuty 且携带 clock_in_at 与库行 id。
    #[test]
    fn clock_in_from_off_enters_on_duty() {
        let mut s = WorkdayState::Off;
        s.clock_in(1_000, 7).unwrap();
        assert_eq!(
            s,
            WorkdayState::OnDuty {
                clock_in_at: 1_000,
                id: 7
            }
        );
        assert!(s.is_on_duty());
    }

    /// 重复上班严格报错（AlreadyOnDuty），状态不被破坏。
    #[test]
    fn clock_in_while_on_duty_errors() {
        let mut s = WorkdayState::Off;
        s.clock_in(1_000, 7).unwrap();
        assert_eq!(s.clock_in(2_000, 8), Err(WorkdayError::AlreadyOnDuty));
        assert_eq!(
            s,
            WorkdayState::OnDuty {
                clock_in_at: 1_000,
                id: 7
            }
        );
    }

    /// OnDuty 下班：返回在岗信息（clock_in_at + id 供落库关行）并回到 Off。
    #[test]
    fn clock_out_returns_span_and_exits() {
        let mut s = WorkdayState::Off;
        s.clock_in(1_000, 7).unwrap();
        assert_eq!(
            s.clock_out(),
            Ok(DutySpan {
                clock_in_at: 1_000,
                id: 7
            })
        );
        assert_eq!(s, WorkdayState::Off);
        assert!(!s.is_on_duty());
    }

    /// 未上班下班严格报错（NotOnDuty）。
    #[test]
    fn clock_out_while_off_errors() {
        let mut s = WorkdayState::Off;
        assert_eq!(s.clock_out(), Err(WorkdayError::NotOnDuty));
    }

    /// 启动恢复语义：库行 clock_out 为空 = 在岗恢复；已关行/无行 = Off（W3 恢复判定源）。
    #[test]
    fn from_latest_restores_open_duty() {
        assert_eq!(
            WorkdayState::from_latest(Some((7, 1_000, None))),
            WorkdayState::OnDuty {
                clock_in_at: 1_000,
                id: 7
            }
        );
        assert_eq!(
            WorkdayState::from_latest(Some((7, 1_000, Some(2_000)))),
            WorkdayState::Off
        );
        assert_eq!(WorkdayState::from_latest(None), WorkdayState::Off);
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

    /// now < 上班 + N 不到点；恰满/超过返回回填时刻（上班 + N，与发现时刻无关）。
    #[test]
    fn auto_out_due_backfills_at_threshold() {
        let in_at = 10_000;
        assert_eq!(auto_out_due(in_at + 8 * HOUR - 1, in_at, 8), None);
        assert_eq!(
            auto_out_due(in_at + 8 * HOUR, in_at, 8),
            Some(in_at + 8 * HOUR)
        );
        assert_eq!(
            auto_out_due(in_at + 9 * HOUR, in_at, 8),
            Some(in_at + 8 * HOUR),
            "迟到发现仍回填至上班 + N"
        );
    }

    // —— W1：reduce_day 事件归约 ——

    /// 空日：全零、无区块、无在岗起止。
    #[test]
    fn empty_day_reduces_to_zero() {
        let sum = reduce_day(&[], DAY, DAY + 86_400, DAY + 12 * HOUR);
        assert_eq!(sum.duty_secs, 0);
        assert_eq!(sum.work_secs, 0);
        assert_eq!(sum.rest_secs, 0);
        assert!(sum.blocks.is_empty());
        assert_eq!(sum.duty_started_at, None);
        assert_eq!(sum.duty_ended_at, None);
    }

    /// 全程连续工作：duty = work，rest 为零，单块 Work。
    #[test]
    fn continuous_work_has_no_rest() {
        let events = [
            ev(DAY + 9 * HOUR, EventKind::ClockIn),
            ev(DAY + 9 * HOUR, EventKind::SegmentStart),
            ev(DAY + 17 * HOUR, EventKind::SegmentEnd),
            ev(DAY + 17 * HOUR, EventKind::ClockOut),
        ];
        let sum = reduce_day(&events, DAY, DAY + 86_400, DAY + 20 * HOUR);
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
        let events = [
            ev(DAY + 9 * HOUR, EventKind::ClockIn),
            ev(DAY + 9 * HOUR, EventKind::SegmentStart),
            ev(DAY + 12 * HOUR, EventKind::SegmentEnd),
            ev(DAY + 17 * HOUR, EventKind::ClockOut),
        ];
        let sum = reduce_day(&events, DAY, DAY + 86_400, DAY + 20 * HOUR);
        assert_eq!(sum.duty_secs, 8 * HOUR);
        assert_eq!(sum.work_secs, 3 * HOUR);
        assert_eq!(sum.rest_secs, 5 * HOUR);
        assert_eq!(sum.blocks.len(), 2);
        assert_eq!(sum.blocks[0].kind, BlockKind::Work);
        assert_eq!(sum.blocks[1].kind, BlockKind::Rest);
        assert_eq!(sum.blocks[1].start, DAY + 12 * HOUR);
        assert_eq!(sum.blocks[1].end, DAY + 17 * HOUR);
    }

    /// 多段多间隙：三值与四区块（Work/Rest 交替）按序产出。
    #[test]
    fn multiple_segments_alternate_blocks() {
        let events = [
            ev(DAY + 9 * HOUR, EventKind::ClockIn),
            ev(DAY + 9 * HOUR, EventKind::SegmentStart),
            ev(DAY + 10 * HOUR, EventKind::SegmentEnd),
            ev(DAY + 11 * HOUR, EventKind::SegmentStart),
            ev(DAY + 12 * HOUR, EventKind::SegmentEnd),
            ev(DAY + 17 * HOUR, EventKind::ClockOut),
        ];
        let sum = reduce_day(&events, DAY, DAY + 86_400, DAY + 20 * HOUR);
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

    /// 未下班（在岗中）：duty 窗口闭于 min(now, day_end)，duty_ended_at 为 None。
    #[test]
    fn open_duty_closes_at_now() {
        let events = [
            ev(DAY + 9 * HOUR, EventKind::ClockIn),
            ev(DAY + 9 * HOUR, EventKind::SegmentStart),
        ];
        // now 在日内：闭于 now
        let sum = reduce_day(&events, DAY, DAY + 86_400, DAY + 10 * HOUR);
        assert_eq!(sum.duty_secs, HOUR);
        assert_eq!(sum.work_secs, HOUR);
        assert_eq!(sum.duty_ended_at, None);
        // now 已跨日：闭于 day_end（不越界）
        let sum = reduce_day(&events, DAY, DAY + 86_400, DAY + 40 * HOUR);
        assert_eq!(sum.duty_secs, 15 * HOUR);
        assert_eq!(sum.duty_ended_at, None);
    }

    /// 跨零点按日切分：23:00 上班次日 07:00 下班，两日各取各段、总账不重不漏。
    #[test]
    fn cross_midnight_splits_by_day() {
        let events = [
            ev(DAY + 23 * HOUR, EventKind::ClockIn),
            ev(DAY + 23 * HOUR, EventKind::SegmentStart),
            ev(DAY + 27 * HOUR, EventKind::SegmentEnd),
            ev(DAY + 31 * HOUR, EventKind::ClockOut),
        ];
        // 第 1 日：班内 1h 且全为工作
        let day1 = reduce_day(&events, DAY, DAY + 86_400, DAY + 40 * HOUR);
        assert_eq!(day1.duty_secs, HOUR);
        assert_eq!(day1.work_secs, HOUR);
        assert_eq!(day1.rest_secs, 0);
        assert_eq!(day1.duty_started_at, Some(DAY + 23 * HOUR));
        assert_eq!(day1.duty_ended_at, Some(DAY + 86_400));
        // 第 2 日：clock_out 孤儿 → 在岗自日零点起算；segment_end 孤儿 → 工作自日零点起算
        let day2 = reduce_day(&events, DAY + 86_400, DAY + 2 * 86_400, DAY + 40 * HOUR);
        assert_eq!(day2.duty_secs, 7 * HOUR);
        assert_eq!(day2.work_secs, 3 * HOUR);
        assert_eq!(day2.rest_secs, 4 * HOUR);
        assert_eq!(day2.duty_started_at, Some(DAY + 86_400));
        assert_eq!(day2.duty_ended_at, Some(DAY + 31 * HOUR));
        // 两日总账：在岗 8h / 工作 4h / 休息 4h（不重不漏）
        let total = day1.duty_secs + day2.duty_secs;
        assert_eq!(total, 8 * HOUR);
        assert_eq!(day1.work_secs + day2.work_secs, 4 * HOUR);
    }

    /// 自动下班回填：clock_out 与段末都记在回填时刻，图谱工作块恰好闭于回填点。
    #[test]
    fn auto_out_backfill_clips_to_out_at() {
        let out_at = DAY + 17 * HOUR;
        let events = [
            ev(DAY + 9 * HOUR, EventKind::ClockIn),
            ev(DAY + 9 * HOUR, EventKind::SegmentStart),
            // 真实段末晚于回填点（次日才发现），但事件按回填口径记在 out_at
            ev(out_at, EventKind::SegmentEnd),
            ev(out_at, EventKind::AutoClockOut),
        ];
        let sum = reduce_day(&events, DAY, DAY + 86_400, DAY + 40 * HOUR);
        assert_eq!(sum.duty_secs, 8 * HOUR);
        assert_eq!(sum.work_secs, 8 * HOUR);
        assert_eq!(sum.rest_secs, 0);
        assert_eq!(sum.blocks.last().unwrap().end, out_at);
        assert_eq!(sum.duty_ended_at, Some(out_at));
    }

    /// 零长在岗窗口（误触上下班同一秒）不产出区块也不计三值。
    #[test]
    fn zero_length_duty_is_dropped() {
        let events = [
            ev(DAY + 9 * HOUR, EventKind::ClockIn),
            ev(DAY + 9 * HOUR, EventKind::ClockOut),
        ];
        let sum = reduce_day(&events, DAY, DAY + 86_400, DAY + 20 * HOUR);
        assert_eq!(sum.duty_secs, 0);
        assert!(sum.blocks.is_empty());
        assert_eq!(sum.duty_started_at, None);
    }

    /// 在岗但全程未计时：整段皆为休息（三值语义：休息 = 在岗 − 工作）。
    #[test]
    fn duty_without_segments_is_all_rest() {
        let events = [
            ev(DAY + 9 * HOUR, EventKind::ClockIn),
            ev(DAY + 17 * HOUR, EventKind::ClockOut),
        ];
        let sum = reduce_day(&events, DAY, DAY + 86_400, DAY + 20 * HOUR);
        assert_eq!(sum.duty_secs, 8 * HOUR);
        assert_eq!(sum.work_secs, 0);
        assert_eq!(sum.rest_secs, 8 * HOUR);
        assert_eq!(sum.blocks.len(), 1);
        assert_eq!(sum.blocks[0].kind, BlockKind::Rest);
    }

    /// 一日多班（下班后再上班）：两段 duty 窗口都归约，起止取首尾、开班则 ended 为 None。
    #[test]
    fn multiple_duty_windows_per_day() {
        let events = [
            ev(DAY + 9 * HOUR, EventKind::ClockIn),
            ev(DAY + 10 * HOUR, EventKind::ClockOut),
            ev(DAY + 11 * HOUR, EventKind::ClockIn),
        ];
        let sum = reduce_day(&events, DAY, DAY + 86_400, DAY + 12 * HOUR);
        assert_eq!(sum.duty_secs, 2 * HOUR);
        assert_eq!(sum.rest_secs, 2 * HOUR);
        assert_eq!(sum.duty_started_at, Some(DAY + 9 * HOUR));
        assert_eq!(sum.duty_ended_at, None, "尚有未关窗口（在岗中）");
        assert_eq!(sum.blocks.len(), 2);
    }
}
