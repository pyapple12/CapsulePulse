//! 依赖探针：rusqlite bundled 工具链直连冒烟（内存库建表/写入/聚合），不依赖本 crate 业务代码。
//! 属 PL002.1 验证资产；收口时若无复用价值随任务注记决定去留。

use rusqlite::Connection;

#[test]
fn rusqlite_bundled_in_memory_smoke() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute("CREATE TABLE t (a INTEGER)", []).unwrap();
    conn.execute("INSERT INTO t (a) VALUES (1)", []).unwrap();
    conn.execute("INSERT INTO t (a) VALUES (2)", []).unwrap();
    let total: i64 = conn
        .query_row("SELECT SUM(a) FROM t", [], |row| row.get(0))
        .unwrap();
    assert_eq!(total, 3);
}
