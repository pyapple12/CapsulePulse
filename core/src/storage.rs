//! SQLite 存储层：会话记录写入与今日/本周/累计聚合（计划书 §2.2 定案 schema）。
//! 全程参数化查询（禁拼接）；聚合边界经 period 纯函数（泛型时区，测试固定时区）；
//! 测试一律内存库/临时目录，零真实用户数据写入（AGENTS 红线）。

use std::path::Path;

use rusqlite::Connection;
use thiserror::Error;

/// 存储层错误：底层错误透传，不静默兜底（AGENTS 错误策略主线）。
#[derive(Debug, Error)]
pub enum StorageError {
    /// SQLite 错误（打开/建表/读写）。
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
    /// 数据目录创建失败。
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// events.kind 出现未知文本（外部改坏/版本残留），读取即报错。
    #[error("事件类型非法：{0}")]
    UnknownEventKind(String),
    /// runtime.mode 出现未知文本（外部改坏/版本残留），读取即报错。
    #[error("运行态模式非法：{0}")]
    UnknownRuntimeMode(String),
    /// 迁移校验未通过：有 workdays 行缺少同刻 clock_in 事件（迁移中止、旧表原样）。
    #[error("迁移校验未通过：{0} 条 workdays 行缺同刻 clock_in 事件（已中止，旧表原样）")]
    MigrationValidationFailed(i64),
    /// 迁移备份路径解析失败（库文件无父目录——不应发生）。
    #[error("迁移备份路径解析失败（库文件无父目录）")]
    MigrationPath,
}

/// 运行态模式（PL027 心跳持久化编码；与命令层 `RunMode` 同义：工作 = Running、休息 = Paused）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeMode {
    /// 工作（计时运行中）。
    Work,
    /// 休息（计时暂停）。
    Rest,
}

impl RuntimeMode {
    /// 存储序列化文本。
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Work => "work",
            Self::Rest => "rest",
        }
    }

    /// 自存储文本解析；未知值返回 None（读取方据此严格报错）。
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "work" => Some(Self::Work),
            "rest" => Some(Self::Rest),
            _ => None,
        }
    }
}

/// 运行态缓存（PL027）：单行 `id=1`。**除 `last_beat_at`（最后心跳）外，其余字段均可由
/// events 流水重建**——与流水不一致时以流水为准、缓存可丢可重建；心跳用于崩溃恢复判定
/// "停机多久"（流水里推不出来，这是本行存在的唯一硬理由）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeState {
    /// 是否在岗（最后一条打卡未配对）。
    pub on_duty: bool,
    /// 当前运行态。
    pub mode: RuntimeMode,
    /// 当前工作段起点（休息态为 None）。
    pub segment_start_at: Option<i64>,
    /// 当前班打卡时刻（未在岗为 None）。
    pub duty_clock_in_at: Option<i64>,
    /// 自动下班基准（打卡 + N，含加班重算；未在岗为 None）。
    pub auto_out_at: Option<i64>,
    /// 本次累计（自最后一条 `session_reset` 或本次打卡起的工作秒数）。
    pub accumulated_secs: i64,
    /// 最后一次心跳时刻（Unix 秒）。
    pub last_beat_at: i64,
}

/// 已封口数据日的三值汇总（PL027 加速层）：`date` = 本地数据日（打卡日，`YYYY-MM-DD`）。
/// 全部字段可由 events 流水重算，故整表可丢、可随时重建（只是省掉"逐日现算"）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DayRollup {
    /// 数据日（打卡日，`YYYY-MM-DD` 本地）。
    pub date: String,
    /// 在岗秒数。
    pub duty_secs: i64,
    /// 工作秒数。
    pub work_secs: i64,
    /// 休息秒数。
    pub rest_secs: i64,
    /// 首个窗口起点（无数据日 None）。
    pub first_in_at: Option<i64>,
    /// 末个窗口终点（未下班 None）。
    pub last_out_at: Option<i64>,
    /// 滚动落库时刻（Unix 秒）。
    pub sealed_at: i64,
}

/// 已封口日汇总合计（PL027 加速层聚合；供全历史累计/总日均）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DaysTotals {
    /// 在岗总秒数。
    pub duty_secs: i64,
    /// 工作总秒数。
    pub work_secs: i64,
    /// 休息总秒数。
    pub rest_secs: i64,
    /// 有数据天数。
    pub days: u32,
}

/// 迁移报告（PL027.1b，供命令层写对账日志）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MigrationReport {
    /// 迁移前 `sessions` 秒数总和（对账基准；老库无该表 → None）。
    pub sessions_secs_before: Option<i64>,
    /// 迁移时校验过的 `workdays` 行数。
    pub workdays_rows: i64,
    /// 本次是否真的执行了迁移（false = 无需迁移 / 已迁过）。
    pub migrated: bool,
}

/// 会话存储（PL027 起 = **events 单表唯一事实源** + `runtime` 运行态缓存 + `days` 按天汇总）：
/// 打卡/下班/段起止只追加事件；今日/本周/累计与三值由流水归约，`days` 只做加速。
pub struct Storage {
    conn: Connection,
}

impl Storage {
    /// 打开（必要时创建）文件库：库文件可自建，父目录须已存在（见 [`Self::open_default`]）。
    pub fn open(path: &Path) -> Result<Self, StorageError> {
        Self::init(Connection::open(path)?)
    }

    /// 内存库（测试专用）。
    #[cfg(test)]
    pub fn open_in_memory() -> Result<Self, StorageError> {
        Self::init(Connection::open_in_memory()?)
    }

    /// 以默认路径打开：<运行时根>/data/pulse.db（双落址见 crate::paths；目录不存在则自建）。
    pub fn open_default() -> Result<Self, StorageError> {
        Self::open(&crate::paths::default_db_path()?)
    }

    /// 建表（幂等）：`events` 为唯一事实源（PL027 前为 PL005 追加）；PL027 追加
    /// `runtime`（运行态缓存单行）/`days`（按天汇总加速层）/`schema_meta`（模型版本）
    /// 与 `idx_events_at`（按时间取流水的索引）。**旧两表 `workdays`/`sessions` 不再创建**
    /// （唯一事实源即流水；老库由 [`Self::migrate_events_only`] 一次性迁移处置）。
    fn init(conn: Connection) -> Result<Self, StorageError> {
        // PATCH005.14：写库**快速失败**——默认 busy 等待约 5s（探针实测 5522ms）会把"打卡失败"
        // 的反馈拖成数秒；正常路径 app 是唯一写者（各命令经 Mutex 串行、心跳同锁），归零不误伤。
        conn.busy_timeout(std::time::Duration::from_millis(0))?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                at INTEGER NOT NULL,
                kind TEXT NOT NULL
            )",
            [],
        )?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS runtime (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                on_duty INTEGER NOT NULL,
                mode TEXT NOT NULL,
                segment_start_at INTEGER,
                duty_clock_in_at INTEGER,
                auto_out_at INTEGER,
                accumulated_secs INTEGER NOT NULL,
                last_beat_at INTEGER NOT NULL
            )",
            [],
        )?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS days (
                date TEXT PRIMARY KEY,
                duty_secs INTEGER NOT NULL,
                work_secs INTEGER NOT NULL,
                rest_secs INTEGER NOT NULL,
                first_in_at INTEGER,
                last_out_at INTEGER,
                sealed_at INTEGER NOT NULL
            )",
            [],
        )?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS schema_meta (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            )",
            [],
        )?;
        conn.execute("CREATE INDEX IF NOT EXISTS idx_events_at ON events(at)", [])?;
        Ok(Self { conn })
    }

    /// 追加若干事件（单事务，PL027：打卡与下班都只写流水）：任一笔失败整体回滚；
    /// 按入参顺序写入（同刻事件定序，归约依赖它）。
    pub fn insert_events(
        &self,
        events: &[(i64, crate::workday::EventKind)],
    ) -> Result<(), StorageError> {
        let tx = self.conn.unchecked_transaction()?;
        for (at, kind) in events {
            tx.execute(
                "INSERT INTO events (at, kind) VALUES (?1, ?2)",
                rusqlite::params![at, kind.as_str()],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// 未配对打卡时刻（PL027.6 单一事实源）：最后一条 `clock_in` 之后没有任何下班事件
    /// （`clock_out` / `auto_clock_out` / `crash_clock_out`）→ 返回该打卡时刻，否则 None。
    /// 打卡门禁与启动恢复共用（与库同源，根治"内存态与库不同步时开出重叠班"）。
    pub fn open_duty_clock_in(&self) -> Result<Option<i64>, StorageError> {
        let last_in: Option<i64> = self.conn.query_row(
            "SELECT MAX(at) FROM events WHERE kind = 'clock_in'",
            [],
            |row| row.get(0),
        )?;
        let Some(clock_in) = last_in else {
            return Ok(None);
        };
        let closes: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM events
             WHERE kind IN ('clock_out', 'auto_clock_out', 'crash_clock_out') AND at >= ?1",
            [clock_in],
            |row| row.get(0),
        )?;
        Ok((closes == 0).then_some(clock_in))
    }

    /// 全历史最早记录时刻（events.at 最小；空库 None）——总日均聚合的窗口起点
    /// （PL027：单一事实源，不再看 workdays）。
    pub fn first_record_at(&self) -> Result<Option<i64>, StorageError> {
        Ok(self
            .conn
            .query_row("SELECT MIN(at) FROM events", [], |row| row.get(0))?)
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

    // —— PL027.1：运行态缓存 / 按天汇总 / 模型版本 ——

    /// 读运行态缓存（PL027）：无行（未在岗 / 旧库首启）返回 None。
    /// # 错误
    /// `mode` 列出现未知文本时返回 [`StorageError::UnknownRuntimeMode`]。
    pub fn runtime_get(&self) -> Result<Option<RuntimeState>, StorageError> {
        let row = self.conn.query_row(
            "SELECT on_duty, mode, segment_start_at, duty_clock_in_at, auto_out_at,
                    accumulated_secs, last_beat_at
             FROM runtime WHERE id = 1",
            [],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<i64>>(2)?,
                    row.get::<_, Option<i64>>(3)?,
                    row.get::<_, Option<i64>>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, i64>(6)?,
                ))
            },
        );
        let raw = match row {
            Ok(raw) => raw,
            Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
            Err(err) => return Err(err.into()),
        };
        let mode = RuntimeMode::parse(&raw.1)
            .ok_or_else(|| StorageError::UnknownRuntimeMode(raw.1.clone()))?;
        Ok(Some(RuntimeState {
            on_duty: raw.0 != 0,
            mode,
            segment_start_at: raw.2,
            duty_clock_in_at: raw.3,
            auto_out_at: raw.4,
            accumulated_secs: raw.5,
            last_beat_at: raw.6,
        }))
    }

    /// 写运行态缓存（PL027 心跳）：单行 upsert，幂等——后写覆盖前写（并发由调用方锁序保证）。
    pub fn runtime_upsert(&self, state: RuntimeState) -> Result<(), StorageError> {
        self.conn.execute(
            "INSERT INTO runtime (id, on_duty, mode, segment_start_at, duty_clock_in_at,
                                  auto_out_at, accumulated_secs, last_beat_at)
             VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(id) DO UPDATE SET
                on_duty = excluded.on_duty,
                mode = excluded.mode,
                segment_start_at = excluded.segment_start_at,
                duty_clock_in_at = excluded.duty_clock_in_at,
                auto_out_at = excluded.auto_out_at,
                accumulated_secs = excluded.accumulated_secs,
                last_beat_at = excluded.last_beat_at",
            rusqlite::params![
                i64::from(state.on_duty),
                state.mode.as_str(),
                state.segment_start_at,
                state.duty_clock_in_at,
                state.auto_out_at,
                state.accumulated_secs,
                state.last_beat_at,
            ],
        )?;
        Ok(())
    }

    /// 清运行态缓存（下班 / 恢复收口后）：无行时亦成功（幂等）。
    pub fn runtime_clear(&self) -> Result<(), StorageError> {
        self.conn.execute("DELETE FROM runtime", [])?;
        Ok(())
    }

    /// 读某个数据日的汇总行（PL027）：未滚过返回 None（调用方回落到流水现算）。
    pub fn day_get(&self, date: &str) -> Result<Option<DayRollup>, StorageError> {
        let row = self.conn.query_row(
            "SELECT date, duty_secs, work_secs, rest_secs, first_in_at, last_out_at, sealed_at
             FROM days WHERE date = ?1",
            [date],
            |row| {
                Ok(DayRollup {
                    date: row.get(0)?,
                    duty_secs: row.get(1)?,
                    work_secs: row.get(2)?,
                    rest_secs: row.get(3)?,
                    first_in_at: row.get(4)?,
                    last_out_at: row.get(5)?,
                    sealed_at: row.get(6)?,
                })
            },
        );
        match row {
            Ok(rollup) => Ok(Some(rollup)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(err) => Err(err.into()),
        }
    }

    /// 写某个数据日的汇总行（PL027 惰性滚动，幂等：同日后写覆盖）。
    pub fn day_upsert(&self, row: &DayRollup) -> Result<(), StorageError> {
        self.conn.execute(
            "INSERT INTO days (date, duty_secs, work_secs, rest_secs, first_in_at, last_out_at,
                               sealed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(date) DO UPDATE SET
                duty_secs = excluded.duty_secs,
                work_secs = excluded.work_secs,
                rest_secs = excluded.rest_secs,
                first_in_at = excluded.first_in_at,
                last_out_at = excluded.last_out_at,
                sealed_at = excluded.sealed_at",
            rusqlite::params![
                row.date,
                row.duty_secs,
                row.work_secs,
                row.rest_secs,
                row.first_in_at,
                row.last_out_at,
                row.sealed_at,
            ],
        )?;
        Ok(())
    }

    /// 已封口日汇总合计（PL027 加速层；空表全零）。
    pub fn days_totals(&self) -> Result<DaysTotals, StorageError> {
        Ok(self.conn.query_row(
            "SELECT COALESCE(SUM(duty_secs), 0), COALESCE(SUM(work_secs), 0),
                    COALESCE(SUM(rest_secs), 0), COUNT(*)
             FROM days",
            [],
            |row| {
                Ok(DaysTotals {
                    duty_secs: row.get(0)?,
                    work_secs: row.get(1)?,
                    rest_secs: row.get(2)?,
                    days: row.get::<_, i64>(3)? as u32,
                })
            },
        )?)
    }

    /// 读模型元信息（PL027 `schema_meta`）：键不存在返回 None。
    pub fn schema_get(&self, key: &str) -> Result<Option<String>, StorageError> {
        let row = self.conn.query_row(
            "SELECT value FROM schema_meta WHERE key = ?1",
            [key],
            |row| row.get(0),
        );
        match row {
            Ok(value) => Ok(Some(value)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(err) => Err(err.into()),
        }
    }

    /// 写模型元信息（PL027 幂等 upsert）。
    pub fn schema_set(&self, key: &str, value: &str) -> Result<(), StorageError> {
        self.conn.execute(
            "INSERT INTO schema_meta (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [key, value],
        )?;
        Ok(())
    }

    /// 测试辅助：运行态行数（单行 upsert 幂等断言用）。
    #[cfg(test)]
    pub(crate) fn runtime_row_count(&self) -> Result<i64, StorageError> {
        Ok(self
            .conn
            .query_row("SELECT COUNT(*) FROM runtime", [], |row| row.get(0))?)
    }

    /// 测试辅助：直插未知 mode 的坏行（错误路径断言用）。
    #[cfg(test)]
    pub(crate) fn insert_raw_runtime_mode(&self, mode: &str) -> Result<(), StorageError> {
        self.conn.execute(
            "INSERT INTO runtime (id, on_duty, mode, segment_start_at, duty_clock_in_at,
                                  auto_out_at, accumulated_secs, last_beat_at)
             VALUES (1, 0, ?1, NULL, NULL, NULL, 0, 0)",
            rusqlite::params![mode],
        )?;
        Ok(())
    }

    /// 表存在性（迁移探测旧表 / 建库契约断言共用）。
    pub(crate) fn table_exists(&self, name: &str) -> Result<bool, StorageError> {
        let n: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
            [name],
            |row| row.get(0),
        )?;
        Ok(n > 0)
    }

    // —— PL027.1b：把旧模型（workdays + sessions）一次性迁到 events 单表 ——

    /// 一次性迁移（PL027.1b）：把旧模型迁到 events 单表——**幂等**（`model_version = 2` 即跳过）、
    /// **先备份后动手**（`VACUUM INTO` 一致快照，路径由调用方给）、**校验不过即中止**（旧表原样）。
    /// 步骤：① 已迁过 → 直接返回；② 备份；③ 校验"每条 `workdays` 行都有同刻 `clock_in` 事件"；
    /// ④ `DROP` 旧两表（内容 100% 可由流水推出：班 = 打卡配对、段 = 段事件配对）。
    /// `model_version` 由命令层在**回填汇总并写对账日志之后**再置 2（避免"半迁移"被当已完成）。
    /// # 错误
    /// 校验不通过返回 [`StorageError::MigrationValidationFailed`]（此时旧表未被删）。
    pub fn migrate_events_only(&self, backup_path: &Path) -> Result<MigrationReport, StorageError> {
        if self.schema_get("model_version")?.as_deref() == Some("2") {
            return Ok(MigrationReport {
                sessions_secs_before: None,
                workdays_rows: 0,
                migrated: false,
            });
        }
        let has_workdays = self.table_exists("workdays")?;
        let has_sessions = self.table_exists("sessions")?;
        if !has_workdays && !has_sessions {
            return Ok(MigrationReport {
                sessions_secs_before: None,
                workdays_rows: 0,
                migrated: false,
            });
        }
        let sessions_secs_before = if has_sessions {
            Some(self.conn.query_row(
                "SELECT COALESCE(SUM(seconds), 0) FROM sessions",
                [],
                |row| row.get(0),
            )?)
        } else {
            None
        };
        // ② 一致快照备份（VACUUM INTO：库打开状态下安全导出）
        self.conn
            .execute("VACUUM INTO ?1", [backup_path.to_string_lossy().as_ref()])?;
        // ③ 校验：旧班行必须能对应同刻 clock_in 事件
        let bad: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM workdays w
             WHERE NOT EXISTS (
                SELECT 1 FROM events e WHERE e.kind = 'clock_in' AND e.at = w.clock_in_at
             )",
            [],
            |row| row.get(0),
        )?;
        if bad > 0 {
            return Err(StorageError::MigrationValidationFailed(bad));
        }
        let workdays_rows: i64 =
            self.conn
                .query_row("SELECT COUNT(*) FROM workdays", [], |row| row.get(0))?;
        // ④ 删旧两表（单事务）
        let tx = self.conn.unchecked_transaction()?;
        tx.execute("DROP TABLE IF EXISTS workdays", [])?;
        tx.execute("DROP TABLE IF EXISTS sessions", [])?;
        tx.commit()?;
        Ok(MigrationReport {
            sessions_secs_before,
            workdays_rows,
            migrated: true,
        })
    }

    /// 测试辅助：按旧模型建 `workdays`/`sessions` 两表（迁移用例造老库用）。
    #[cfg(test)]
    pub(crate) fn create_legacy_tables_for_test(&self) -> Result<(), StorageError> {
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS workdays (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                clock_in_at INTEGER NOT NULL,
                clock_out_at INTEGER
            )",
            [],
        )?;
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS sessions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                started_at INTEGER NOT NULL,
                seconds INTEGER NOT NULL
            )",
            [],
        )?;
        Ok(())
    }

    /// 测试辅助：按旧模型写一行 workdays / sessions（迁移用例造老库用）。
    #[cfg(test)]
    pub(crate) fn insert_legacy_rows_for_test(
        &self,
        clock_in_at: i64,
        clock_out_at: Option<i64>,
        session: Option<(i64, i64)>,
    ) -> Result<(), StorageError> {
        self.conn.execute(
            "INSERT INTO workdays (clock_in_at, clock_out_at) VALUES (?1, ?2)",
            rusqlite::params![clock_in_at, clock_out_at],
        )?;
        if let Some((started_at, seconds)) = session {
            self.conn.execute(
                "INSERT INTO sessions (started_at, seconds) VALUES (?1, ?2)",
                rusqlite::params![started_at, seconds],
            )?;
        }
        Ok(())
    }

    // —— PL027 过渡夹具（仅测试）：旧"班行/sessions 行"形态桥接到流水 ——

    /// 测试辅助：旧的"开班 + 事件"夹具形态——只写流水（返回占位 id 0）。
    #[cfg(test)]
    pub fn workday_open_with_events(
        &self,
        at: i64,
        kinds: &[crate::workday::EventKind],
    ) -> Result<i64, StorageError> {
        let events: Vec<(i64, crate::workday::EventKind)> =
            kinds.iter().map(|k| (at, *k)).collect();
        self.insert_events(&events)?;
        Ok(0)
    }

    /// 测试辅助：旧的"关班 + 事件"夹具形态——只写流水。
    #[cfg(test)]
    pub fn workday_close_with_event(
        &self,
        _id: i64,
        at: i64,
        kind: crate::workday::EventKind,
    ) -> Result<(), StorageError> {
        self.insert_events(&[(at, kind)])
    }

    /// 测试辅助：旧"落段 + 段末事件"夹具形态——只写段末事件（段时长由流水推）。
    #[cfg(test)]
    pub fn record_session_with_event(
        &self,
        _started_at: i64,
        _seconds: i64,
        at: i64,
        kind: crate::workday::EventKind,
    ) -> Result<(), StorageError> {
        self.insert_events(&[(at, kind)])
    }

    /// 测试辅助：在岗行读取夹具——由流水推"未配对打卡"（id 占位 0）。
    #[cfg(test)]
    pub fn workday_latest(&self) -> Result<Option<(i64, i64, Option<i64>)>, StorageError> {
        Ok(self.open_duty_clock_in()?.map(|at| (0, at, None)))
    }

    /// 测试辅助：由流水配对（`segment_start` → `segment_end`）求"已落库段数"（零长段不计）。
    #[cfg(test)]
    pub fn session_count(&self) -> Result<i64, StorageError> {
        Ok(self.session_pairs()?.len() as i64)
    }

    /// 测试辅助：由流水配对求和"累计秒"（= 旧 sessions 求和语义）。
    #[cfg(test)]
    pub fn all_total(&self) -> Result<i64, StorageError> {
        Ok(self.session_pairs()?.iter().map(|(s, e)| e - s).sum())
    }

    /// 测试辅助：配对全部 `segment_start`/`segment_end`（跳过零长与孤儿），返回 (起, 止)。
    #[cfg(test)]
    fn session_pairs(&self) -> Result<Vec<(i64, i64)>, StorageError> {
        let events = self
            .conn
            .prepare("SELECT at, kind FROM events ORDER BY at, id")?
            .query_map([], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        let mut pairs = Vec::new();
        let mut open: Option<i64> = None;
        for (at, kind) in events {
            match kind.as_str() {
                "segment_start" => open = open.or(Some(at)),
                "segment_end" => {
                    if let Some(start) = open.take() {
                        if at > start {
                            pairs.push((start, at));
                        }
                    }
                }
                _ => {}
            }
        }
        Ok(pairs)
    }

    /// 测试辅助：索引存在性（建库契约断言用）。
    #[cfg(test)]
    pub(crate) fn index_exists(&self, name: &str) -> Result<bool, StorageError> {
        let n: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = ?1",
            [name],
            |row| row.get(0),
        )?;
        Ok(n > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workday::EventKind;

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

    /// PL027.6：多事件单事务写入——按入参顺序落库（同刻定序），查询按时间升序返回。
    #[test]
    fn insert_events_writes_in_order() {
        let s = Storage::open_in_memory().unwrap();
        s.insert_events(&[
            (2_000, EventKind::AutoClockOut),
            (1_000, EventKind::ClockIn),
            (1_000, EventKind::SegmentStart),
        ])
        .unwrap();
        let rows = s.events_between(0, 9_000).unwrap();
        assert_eq!(
            rows,
            vec![
                (1_000, EventKind::ClockIn),
                (1_000, EventKind::SegmentStart),
                (2_000, EventKind::AutoClockOut),
            ],
            "同刻按写入序（id 升序）"
        );
    }

    /// PL027.6：未配对打卡判定——打卡后在岗、任一形式的下班后离岗；孤儿下班不产生在岗。
    #[test]
    fn open_duty_clock_in_tracks_unpaired_clock_in() {
        let s = Storage::open_in_memory().unwrap();
        assert_eq!(s.open_duty_clock_in().unwrap(), None, "空库不在岗");
        s.insert_event(500, EventKind::ClockOut).unwrap();
        assert_eq!(s.open_duty_clock_in().unwrap(), None, "孤儿下班不产生在岗");
        s.insert_events(&[
            (1_000, EventKind::ClockIn),
            (1_000, EventKind::SegmentStart),
        ])
        .unwrap();
        assert_eq!(s.open_duty_clock_in().unwrap(), Some(1_000), "打卡后在岗");
        s.insert_event(2_000, EventKind::CrashClockOut).unwrap();
        assert_eq!(s.open_duty_clock_in().unwrap(), None, "崩溃补下班也算下班");
        s.insert_event(3_000, EventKind::ClockIn).unwrap();
        assert_eq!(
            s.open_duty_clock_in().unwrap(),
            Some(3_000),
            "最新未配对打卡"
        );
    }

    /// PL027.6：全历史起点只看流水（不再依赖 workdays）。
    #[test]
    fn first_record_at_uses_events_only() {
        let s = Storage::open_in_memory().unwrap();
        assert_eq!(s.first_record_at().unwrap(), None);
        s.insert_event(4_000, EventKind::ClockIn).unwrap();
        s.insert_event(1_000, EventKind::SegmentStart).unwrap();
        assert_eq!(s.first_record_at().unwrap(), Some(1_000));
    }

    // —— PL027.1b：一次性迁移（旧 workdays + sessions → events 单表）——

    /// PL027.1b：迁移全链——先备份（一致快照）、校验通过才删旧两表、返回对账基准。
    #[test]
    fn migrate_events_only_backs_up_validates_and_drops() {
        let dir = std::env::temp_dir().join(format!("capsule-pulse-mig-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = dir.join("mig.db");
        let backup = dir.join("legacy-model-v1-test.db");
        let _ = std::fs::remove_file(&db);
        let _ = std::fs::remove_file(&backup);

        let s = Storage::open(&db).unwrap();
        s.create_legacy_tables_for_test().unwrap();
        // 老库：一班（09:00–10:00）+ 一行 sessions(3600) + 对应 clock_in 事件
        s.insert_legacy_rows_for_test(1_000, Some(2_000), Some((1_000, 3_600)))
            .unwrap();
        s.insert_events(&[
            (1_000, EventKind::ClockIn),
            (1_000, EventKind::SegmentStart),
        ])
        .unwrap();

        let report = s.migrate_events_only(&backup).unwrap();
        assert!(report.migrated);
        assert_eq!(report.workdays_rows, 1);
        assert_eq!(report.sessions_secs_before, Some(3_600));
        assert!(backup.exists(), "迁移前已生成一致快照备份");
        assert!(!s.table_exists("workdays").unwrap(), "旧两表已删");
        assert!(!s.table_exists("sessions").unwrap());
        assert_eq!(s.events_between(0, 9_000).unwrap().len(), 2, "流水原样");

        // 已迁过：幂等（不再备份/不再动表）
        let again = s.migrate_events_only(&backup).unwrap();
        assert!(!again.migrated, "置版本后即跳过");
        let _ = std::fs::remove_file(&db);
        let _ = std::fs::remove_file(&backup);
        let _ = std::fs::remove_dir(&dir);
    }

    /// PL027.1b：校验不过**中止迁移且旧表原样**（不删、不置版本）。
    #[test]
    fn migrate_events_only_aborts_on_validation_failure() {
        let dir =
            std::env::temp_dir().join(format!("capsule-pulse-mig-bad-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = dir.join("migbad.db");
        let backup = dir.join("legacy-model-v1-bad.db");
        let _ = std::fs::remove_file(&db);
        let _ = std::fs::remove_file(&backup);

        let s = Storage::open(&db).unwrap();
        s.create_legacy_tables_for_test().unwrap();
        // 老库有一行班，但**没有**同刻 clock_in 事件（外部改坏形态）
        s.insert_legacy_rows_for_test(1_000, Some(2_000), None)
            .unwrap();
        match s.migrate_events_only(&backup) {
            Err(StorageError::MigrationValidationFailed(1)) => {}
            other => panic!("期望 MigrationValidationFailed(1)，实际 {other:?}"),
        }
        assert!(s.table_exists("workdays").unwrap(), "中止后旧表原样");
        assert!(s.table_exists("sessions").unwrap());
        assert_eq!(s.schema_get("model_version").unwrap(), None, "不置版本");
        let _ = std::fs::remove_file(&db);
        let _ = std::fs::remove_file(&backup);
        let _ = std::fs::remove_dir(&dir);
    }

    /// 文件库往返：临时目录建库→流水/运行态/汇总/模型版本写入→重开→数据仍在
    /// （schema 幂等 + 目录自建语义）。
    #[test]
    fn file_storage_persists_across_reopen() {
        let dir = std::env::temp_dir().join(format!("capsule-pulse-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = dir.join("probe.db");
        let _ = std::fs::remove_file(&db);

        let runtime = RuntimeState {
            on_duty: true,
            mode: RuntimeMode::Work,
            segment_start_at: Some(1_000),
            duty_clock_in_at: Some(1_000),
            auto_out_at: Some(29_800),
            accumulated_secs: 0,
            last_beat_at: 1_060,
        };
        let rollup = DayRollup {
            date: "2026-10-11".into(),
            duty_secs: 9 * 3600,
            work_secs: 8 * 3600,
            rest_secs: 3600,
            first_in_at: Some(1_000),
            last_out_at: Some(2_000),
            sealed_at: 3_000,
        };
        {
            let s = Storage::open(&db).unwrap();
            s.insert_events(&[
                (1_000, EventKind::ClockIn),
                (1_000, EventKind::SegmentStart),
            ])
            .unwrap();
            s.runtime_upsert(runtime).unwrap();
            s.day_upsert(&rollup).unwrap();
            s.schema_set("model_version", "2").unwrap();
        }
        let s = Storage::open(&db).unwrap();
        assert_eq!(s.events_between(0, 9_000).unwrap().len(), 2);
        assert_eq!(s.runtime_get().unwrap(), Some(runtime));
        assert_eq!(s.day_get("2026-10-11").unwrap(), Some(rollup));
        assert_eq!(s.schema_get("model_version").unwrap(), Some("2".into()));
        assert_eq!(s.days_totals().unwrap().days, 1);

        let _ = std::fs::remove_file(&db);
        let _ = std::fs::remove_dir(&dir);
    }

    // —— PL027.1：运行态缓存 / 按天汇总 / 模型版本 ——

    /// PL027.1：运行态缓存单行 upsert 幂等 + 读取往返 + 清空。
    #[test]
    fn runtime_upsert_get_clear_roundtrip() {
        let s = Storage::open_in_memory().unwrap();
        assert_eq!(s.runtime_get().unwrap(), None);
        let first = RuntimeState {
            on_duty: true,
            mode: RuntimeMode::Work,
            segment_start_at: Some(1_000),
            duty_clock_in_at: Some(1_000),
            auto_out_at: Some(29_800),
            accumulated_secs: 0,
            last_beat_at: 1_060,
        };
        s.runtime_upsert(first).unwrap();
        assert_eq!(s.runtime_get().unwrap(), Some(first));
        let second = RuntimeState {
            mode: RuntimeMode::Rest,
            segment_start_at: None,
            accumulated_secs: 900,
            last_beat_at: 1_960,
            ..first
        };
        s.runtime_upsert(second).unwrap();
        assert_eq!(
            s.runtime_get().unwrap(),
            Some(second),
            "单行 upsert：后值覆盖"
        );
        assert_eq!(s.runtime_row_count().unwrap(), 1, "始终只有一行");
        s.runtime_clear().unwrap();
        assert_eq!(s.runtime_get().unwrap(), None);
    }

    /// PL027.1：运行态 mode 列出现未知文本读取即严格报错（与 events.kind 同纪律）。
    #[test]
    fn runtime_unknown_mode_is_strict_error() {
        let s = Storage::open_in_memory().unwrap();
        s.insert_raw_runtime_mode("nonsense").unwrap();
        match s.runtime_get() {
            Err(StorageError::UnknownRuntimeMode(m)) => assert_eq!(m, "nonsense"),
            other => panic!("期望 UnknownRuntimeMode，实际 {other:?}"),
        }
    }

    /// PL027.1：按天汇总 upsert 幂等 + 单日读回 + 全历史合计（含天数）。
    #[test]
    fn day_rollup_roundtrip_and_totals() {
        let s = Storage::open_in_memory().unwrap();
        assert_eq!(s.day_get("2026-10-11").unwrap(), None);
        let d1 = DayRollup {
            date: "2026-10-11".into(),
            duty_secs: 9 * 3600,
            work_secs: 8 * 3600,
            rest_secs: 3600,
            first_in_at: Some(1_000),
            last_out_at: Some(2_000),
            sealed_at: 3_000,
        };
        let d2 = DayRollup {
            date: "2026-10-12".into(),
            duty_secs: 3600,
            work_secs: 3000,
            rest_secs: 600,
            first_in_at: Some(4_000),
            last_out_at: Some(5_000),
            sealed_at: 6_000,
        };
        s.day_upsert(&d1).unwrap();
        s.day_upsert(&d2).unwrap();
        assert_eq!(s.day_get("2026-10-11").unwrap(), Some(d1.clone()));
        let updated = DayRollup {
            duty_secs: 10 * 3600,
            ..d1.clone()
        };
        s.day_upsert(&updated).unwrap();
        assert_eq!(
            s.day_get("2026-10-11").unwrap(),
            Some(updated),
            "同一数据日后写覆盖"
        );
        let totals = s.days_totals().unwrap();
        assert_eq!(totals.duty_secs, 10 * 3600 + 3600);
        assert_eq!(totals.work_secs, 8 * 3600 + 3000);
        assert_eq!(totals.rest_secs, 3600 + 600);
        assert_eq!(totals.days, 2);
    }

    /// PL027.1：模型版本读写往返——空库为 None，写入后可覆盖。
    #[test]
    fn schema_meta_roundtrip() {
        let s = Storage::open_in_memory().unwrap();
        assert_eq!(s.schema_get("model_version").unwrap(), None);
        s.schema_set("model_version", "2").unwrap();
        assert_eq!(s.schema_get("model_version").unwrap(), Some("2".into()));
        s.schema_set("model_version", "3").unwrap();
        assert_eq!(s.schema_get("model_version").unwrap(), Some("3".into()));
    }

    /// PL027.1：新表与 events.at 索引随建库就位（旧库首启即补，零迁移成本）。
    #[test]
    fn pl027_schema_objects_exist() {
        let s = Storage::open_in_memory().unwrap();
        for name in ["runtime", "days", "schema_meta"] {
            assert!(s.table_exists(name).unwrap(), "缺表 {name}");
        }
        assert!(
            s.index_exists("idx_events_at").unwrap(),
            "缺 events.at 索引"
        );
    }
}
