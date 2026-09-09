//! SQLite 存储层：会话记录写入与今日/本周/累计聚合（计划书 §2.2 定案 schema）。
//! 全程参数化查询（禁拼接）；聚合边界经 period 纯函数（泛型时区，测试固定时区）；
//! 测试一律内存库/临时目录，零真实用户数据写入（AGENTS 红线）。

use std::path::Path;

use chrono::{DateTime, TimeZone};
use rusqlite::Connection;
use thiserror::Error;

use crate::period;

/// 存储层错误：底层错误透传，不静默兜底（AGENTS 错误策略主线）。
#[derive(Debug, Error)]
pub enum StorageError {
    /// SQLite 错误（打开/建表/读写）。
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
    /// 数据目录创建失败。
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// workdays 行不存在（关行时 id 无效）。
    #[error("工作日记录不存在：id={0}")]
    WorkdayMissing(i64),
    /// events.kind 出现未知文本（外部改坏/版本残留），读取即报错。
    #[error("事件类型非法：{0}")]
    UnknownEventKind(String),
}

/// 会话存储：单表 sessions，一条记录 = 一次暂停/重开时的一个工作段。
pub struct Storage {
    conn: Connection,
}

impl Storage {
    /// 打开（必要时创建）文件库：库文件可自建，父目录须已存在（见 [`Self::open_default`]）。
    pub fn open(path: &Path) -> Result<Self, StorageError> {
        Self::init(Connection::open(path)?)
    }

    /// 内存库（测试专用）。
    pub fn open_in_memory() -> Result<Self, StorageError> {
        Self::init(Connection::open_in_memory()?)
    }

    /// 以默认路径打开：<运行时根>/data/pulse.db（双落址见 crate::paths；目录不存在则自建）。
    pub fn open_default() -> Result<Self, StorageError> {
        Self::open(&crate::paths::default_db_path()?)
    }

    /// 建表（幂等）：sessions 为计划书 §2.2 定案 schema；workdays/events 为 PL005 追加
    /// （CREATE TABLE IF NOT EXISTS 对旧库零迁移成本，老文件首启即补表）。
    fn init(conn: Connection) -> Result<Self, StorageError> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS sessions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                started_at INTEGER NOT NULL,
                seconds INTEGER NOT NULL
            )",
            [],
        )?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS workdays (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                clock_in_at INTEGER NOT NULL,
                clock_out_at INTEGER
            )",
            [],
        )?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                at INTEGER NOT NULL,
                kind TEXT NOT NULL
            )",
            [],
        )?;
        Ok(Self { conn })
    }

    /// 写入一个工作段（参数化绑定）。
    pub fn add_session(&self, started_at: i64, seconds: i64) -> Result<(), StorageError> {
        self.conn.execute(
            "INSERT INTO sessions (started_at, seconds) VALUES (?1, ?2)",
            [started_at, seconds],
        )?;
        Ok(())
    }

    /// 今日累计秒数（started_at ≥ 本地今日零点）。
    pub fn today_total<Tz: TimeZone>(&self, now: &DateTime<Tz>) -> Result<i64, StorageError> {
        self.total_since(period::day_start_secs(now))
    }

    /// 本周累计秒数（started_at ≥ 本地周一零点，周一起）。
    pub fn week_total<Tz: TimeZone>(&self, now: &DateTime<Tz>) -> Result<i64, StorageError> {
        self.total_since(period::week_start_secs(now))
    }

    /// 全部累计秒数。
    pub fn all_total(&self) -> Result<i64, StorageError> {
        self.total_since(0)
    }

    /// 记录条数（测试断言零秒段跳过行为用）。
    #[cfg(test)]
    pub(crate) fn session_count(&self) -> Result<i64, StorageError> {
        Ok(self
            .conn
            .query_row("SELECT COUNT(*) FROM sessions", [], |row| row.get(0))?)
    }

    /// 汇总（since 为下界，含边界；COALESCE 使空表为零）。
    fn total_since(&self, since: i64) -> Result<i64, StorageError> {
        Ok(self.conn.query_row(
            "SELECT COALESCE(SUM(seconds), 0) FROM sessions WHERE started_at >= ?1",
            [since],
            |row| row.get(0),
        )?)
    }

    /// 开一个工作日（上班打卡）：写入 clock_in_at 并返回行 id（clock_out 保持 NULL = 在岗中）。
    pub fn workday_open(&self, clock_in_at: i64) -> Result<i64, StorageError> {
        self.conn.execute(
            "INSERT INTO workdays (clock_in_at, clock_out_at) VALUES (?1, NULL)",
            [clock_in_at],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// 关工作日（下班打卡）：按 id 写 clock_out_at。
    /// # 错误
    /// id 不存在返回 [`StorageError::WorkdayMissing`]（严格报错，不静默成功）。
    pub fn workday_close(&self, id: i64, clock_out_at: i64) -> Result<(), StorageError> {
        let affected = self.conn.execute(
            "UPDATE workdays SET clock_out_at = ?1 WHERE id = ?2",
            [clock_out_at, id],
        )?;
        if affected == 0 {
            return Err(StorageError::WorkdayMissing(id));
        }
        Ok(())
    }

    /// 最新一个工作日 (id, clock_in_at, clock_out_at)：启动恢复判定源（clock_out NULL = 在岗中）。
    pub fn workday_latest(&self) -> Result<Option<(i64, i64, Option<i64>)>, StorageError> {
        let row = self.conn.query_row(
            "SELECT id, clock_in_at, clock_out_at FROM workdays
             ORDER BY id DESC LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        );
        match row {
            Ok(latest) => Ok(Some(latest)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(err) => Err(err.into()),
        }
    }

    /// 写一条打卡/段事件（kind 经 as_str 编码为 TEXT）。
    pub fn insert_event(
        &self,
        at: i64,
        kind: crate::workday::EventKind,
    ) -> Result<(), StorageError> {
        self.conn.execute(
            "INSERT INTO events (at, kind) VALUES (?1, ?2)",
            rusqlite::params![at, kind.as_str()],
        )?;
        Ok(())
    }

    /// 查询半开区间 [start, end) 内事件，按时间升序（同刻按写入序）返回类型化行。
    /// # 错误
    /// 行内 kind 无法解析为已知事件类型时返回 [`StorageError::UnknownEventKind`]（保留原文）。
    pub fn events_between(
        &self,
        start: i64,
        end: i64,
    ) -> Result<Vec<(i64, crate::workday::EventKind)>, StorageError> {
        let mut stmt = self
            .conn
            .prepare("SELECT at, kind FROM events WHERE at >= ?1 AND at < ?2 ORDER BY at, id")?;
        let rows = stmt.query_map([start, end], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?;
        rows.map(|row| {
            let (at, kind) = row?;
            let parsed = crate::workday::EventKind::parse(&kind)
                .ok_or_else(|| StorageError::UnknownEventKind(kind))?;
            Ok((at, parsed))
        })
        .collect()
    }

    /// 绕过类型编码直插事件原文（测试专用：构造未知 kind 的坏行）。
    #[cfg(test)]
    pub(crate) fn insert_raw_event(&self, at: i64, kind: &str) -> Result<(), StorageError> {
        self.conn.execute(
            "INSERT INTO events (at, kind) VALUES (?1, ?2)",
            rusqlite::params![at, kind],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use chrono::{FixedOffset, TimeZone};

    use super::*;
    use crate::workday::EventKind;

    /// 固定 +8 时区（近似 CST）。
    fn tz() -> FixedOffset {
        FixedOffset::east_opt(8 * 3600).unwrap()
    }

    /// 锚点时刻（2026-09-09 周三为测试"当前时间"）。
    fn now() -> chrono::DateTime<FixedOffset> {
        tz().with_ymd_and_hms(2026, 9, 9, 12, 0, 0).unwrap()
    }

    fn ts(y: i32, m: u32, d: u32, h: u32) -> i64 {
        tz().with_ymd_and_hms(y, m, d, h, 0, 0).unwrap().timestamp()
    }

    /// 空表三聚合全为零（COALESCE 兜底）。
    #[test]
    fn empty_storage_totals_are_zero() {
        let s = Storage::open_in_memory().unwrap();
        assert_eq!(s.today_total(&now()).unwrap(), 0);
        assert_eq!(s.week_total(&now()).unwrap(), 0);
        assert_eq!(s.all_total().unwrap(), 0);
    }

    /// T2：多段求和 + 边界排除——今日/本周/累计各自只计入区间内记录。
    #[test]
    fn aggregates_respect_period_boundaries() {
        let s = Storage::open_in_memory().unwrap();
        // 上周（排除于周/今日，计入累计）
        s.add_session(ts(2026, 8, 31, 10), 100).unwrap();
        // 本周一（计入周，排除今日）
        s.add_session(ts(2026, 9, 7, 9), 200).unwrap();
        // 昨天（计入周，排除今日）
        s.add_session(ts(2026, 9, 8, 9), 400).unwrap();
        // 今天上午（周/今日都计入）
        s.add_session(ts(2026, 9, 9, 8), 300).unwrap();

        assert_eq!(s.today_total(&now()).unwrap(), 300);
        assert_eq!(s.week_total(&now()).unwrap(), 900);
        assert_eq!(s.all_total().unwrap(), 1000);
    }

    /// 恰在边界零点的记录计入当日/当周（>= 语义）。
    #[test]
    fn record_at_exact_boundary_is_included() {
        let s = Storage::open_in_memory().unwrap();
        let boundary = period::day_start_secs(&now());
        s.add_session(boundary, 60).unwrap();
        assert_eq!(s.today_total(&now()).unwrap(), 60);
    }

    /// 记录条数辅助（落库跳过零秒段的行为断言用）。
    #[test]
    fn session_count_tracks_inserts() {
        let s = Storage::open_in_memory().unwrap();
        assert_eq!(s.session_count().unwrap(), 0);
        s.add_session(1_000, 30).unwrap();
        s.add_session(2_000, 45).unwrap();
        assert_eq!(s.session_count().unwrap(), 2);
    }

    /// W3：workday 开行/关行/latest 往返——clock_out 为空 = 在岗中（启动恢复语义）。
    #[test]
    fn workday_open_close_latest_roundtrip() {
        let s = Storage::open_in_memory().unwrap();
        assert_eq!(s.workday_latest().unwrap(), None);
        let id = s.workday_open(1_000).unwrap();
        assert_eq!(id, 1);
        assert_eq!(s.workday_latest().unwrap(), Some((1, 1_000, None)));
        s.workday_close(id, 2_000).unwrap();
        assert_eq!(s.workday_latest().unwrap(), Some((1, 1_000, Some(2_000))));
    }

    /// W3：latest 取最新行——次日再上班，恢复语义只看最后一班。
    #[test]
    fn workday_latest_picks_newest_row() {
        let s = Storage::open_in_memory().unwrap();
        let first = s.workday_open(1_000).unwrap();
        s.workday_close(first, 2_000).unwrap();
        let second = s.workday_open(9_000).unwrap();
        assert_eq!(second, 2);
        assert_eq!(s.workday_latest().unwrap(), Some((2, 9_000, None)));
    }

    /// 关行时 id 不存在严格报错（不静默成功）。
    #[test]
    fn workday_close_missing_id_errors() {
        let s = Storage::open_in_memory().unwrap();
        assert!(matches!(
            s.workday_close(99, 2_000),
            Err(StorageError::WorkdayMissing(99))
        ));
    }

    /// W4 数据源：事件写入按 kind 编码，查询按时间序返回类型化结果。
    #[test]
    fn events_roundtrip_typed_and_ordered() {
        let s = Storage::open_in_memory().unwrap();
        // 刻意乱序写入，验证查询按 at 升序
        s.insert_event(3_000, EventKind::SegmentEnd).unwrap();
        s.insert_event(1_000, EventKind::ClockIn).unwrap();
        s.insert_event(2_000, EventKind::SegmentStart).unwrap();
        let rows = s.events_between(0, 9_000).unwrap();
        assert_eq!(
            rows,
            vec![
                (1_000, EventKind::ClockIn),
                (2_000, EventKind::SegmentStart),
                (3_000, EventKind::SegmentEnd),
            ]
        );
    }

    /// events_between 半开区间 [start, end)：恰在两端点的归属确定，不重不漏。
    #[test]
    fn events_between_half_open_bounds() {
        let s = Storage::open_in_memory().unwrap();
        for at in [100, 200, 300] {
            s.insert_event(at, EventKind::SegmentStart).unwrap();
        }
        assert_eq!(s.events_between(100, 300).unwrap().len(), 2, "右端点不含");
        assert_eq!(s.events_between(100, 301).unwrap().len(), 3, "左端点含");
        assert_eq!(s.events_between(301, 400).unwrap().len(), 0);
    }

    /// 库内未知 kind 字符串（外部改坏/旧版本残留）读取即严格报错，不静默跳过。
    #[test]
    fn events_unknown_kind_is_strict_error() {
        let s = Storage::open_in_memory().unwrap();
        s.insert_raw_event(1_000, "nonsense").unwrap();
        match s.events_between(0, 2_000) {
            Err(StorageError::UnknownEventKind(k)) => assert_eq!(k, "nonsense"),
            other => panic!("期望 UnknownEventKind，实际 {other:?}"),
        }
    }

    /// 文件库往返：临时目录建库→三表写入→重开→数据仍在（schema 幂等 + 目录自建语义）。
    #[test]
    fn file_storage_persists_across_reopen() {
        let dir = std::env::temp_dir().join(format!("capsule-pulse-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = dir.join("probe.db");
        let _ = std::fs::remove_file(&db);

        {
            let s = Storage::open(&db).unwrap();
            s.add_session(1_000, 120).unwrap();
            let id = s.workday_open(1_000).unwrap();
            s.workday_close(id, 2_000).unwrap();
            s.insert_event(1_000, EventKind::ClockIn).unwrap();
        }
        let s = Storage::open(&db).unwrap();
        assert_eq!(s.all_total().unwrap(), 120);
        assert_eq!(s.workday_latest().unwrap(), Some((1, 1_000, Some(2_000))));
        assert_eq!(s.events_between(0, 9_000).unwrap().len(), 1);

        let _ = std::fs::remove_file(&db);
        let _ = std::fs::remove_dir(&dir);
    }
}
