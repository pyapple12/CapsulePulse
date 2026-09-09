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

    /// 建表（幂等）：schema 为计划书 §2.2 定案。
    fn init(conn: Connection) -> Result<Self, StorageError> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS sessions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                started_at INTEGER NOT NULL,
                seconds INTEGER NOT NULL
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
}

#[cfg(test)]
mod tests {
    use chrono::{FixedOffset, TimeZone};

    use super::*;

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

    /// 文件库往返：临时目录建库→写入→重开→数据仍在（目录自建语义）。
    #[test]
    fn file_storage_persists_across_reopen() {
        let dir = std::env::temp_dir().join(format!("capsule-pulse-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = dir.join("probe.db");
        let _ = std::fs::remove_file(&db);

        {
            let s = Storage::open(&db).unwrap();
            s.add_session(1_000, 120).unwrap();
        }
        let s = Storage::open(&db).unwrap();
        assert_eq!(s.all_total().unwrap(), 120);

        let _ = std::fs::remove_file(&db);
        let _ = std::fs::remove_dir(&dir);
    }
}
